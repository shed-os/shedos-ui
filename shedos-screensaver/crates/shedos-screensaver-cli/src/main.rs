//! `shedos-screensaver` CLI binary.
//!
//! Stage 3 surface: TTY backend wired, all 8 styles invokable,
//! real frame loop with FPS pacing, --duration / --random /
//! --shuffle / --idle-daemon. Wayland + audio remain stub paths
//! (Wayland mode prints "not yet wired" and exits 0; that work
//! lands in stages 4+).

use clap::{ArgAction, CommandFactory, Parser, ValueEnum};
use clap_complete::Shell;
use crossterm::event::{self, Event, KeyCode};
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use shedos_screensaver_core::{Clock, Color, Frame, Logo, RealClock, SignalListener};
use shedos_screensaver_i18n::{t, t_str, I18n};
use shedos_screensaver_styles::{Ctx, Registry, Style, StyleOpts};
use shedos_screensaver_tty::{detect_terminal_size, stdout_is_tty, TerminalGuard, TtyRenderer};
use std::io;
use std::process::ExitCode;
use std::sync::atomic::Ordering;
use std::time::Duration;

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
enum Mode {
    Tty,
    Wayland,
    Auto,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum AudioSource {
    Desktop,
    Mic,
    None,
}

#[derive(Parser, Debug)]
#[command(
    name = "shedos-screensaver",
    bin_name = "shedos-screensaver",
    version,
    about = "Animated screensaver with TTY + Wayland backends, 8 styles, audio-reactive",
    long_about = None,
    disable_help_flag = false,
)]
struct Cli {
    #[arg(long)]
    help_summary: bool,

    #[arg(long)]
    list: bool,

    #[arg(long, value_name = "NAME")]
    help_style: Option<String>,

    #[arg(long)]
    complete_bash: bool,

    #[arg(long)]
    complete_zsh: bool,

    #[arg(long)]
    complete_fish: bool,

    #[arg(long, value_name = "NAME")]
    style: Option<String>,

    #[arg(long, value_name = "SPEC")]
    color: Option<String>,

    #[arg(long = "style-opt", value_name = "KEY=VAL", action = ArgAction::Append)]
    style_opts: Vec<String>,

    #[arg(long, value_enum, default_value_t = Mode::Auto)]
    mode: Mode,

    #[arg(long, value_name = "N")]
    fps: Option<u32>,

    #[arg(long, value_name = "SECS")]
    duration: Option<f32>,

    #[arg(long)]
    random: bool,

    #[arg(long, value_name = "SECS")]
    shuffle: Option<u32>,

    #[arg(long)]
    idle_daemon: bool,

    #[arg(long, value_enum, default_value_t = AudioSource::None)]
    audio_source: AudioSource,

    #[arg(long, value_name = "PATH|none|auto", default_value = "auto")]
    wallpaper: String,

    #[arg(long, value_name = "F", default_value_t = 0.3)]
    wallpaper_dim: f32,

    #[arg(long, value_name = "BCP47")]
    locale: Option<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    if let Err(e) = I18n::init(cli.locale.as_deref()) {
        eprintln!("warning: i18n init failed: {e}; falling back to embedded en-US");
    }

    if cli.help_summary {
        println!("{}", t("help-summary"));
        return ExitCode::SUCCESS;
    }
    if cli.complete_bash {
        emit_completion(Shell::Bash);
        return ExitCode::SUCCESS;
    }
    if cli.complete_zsh {
        emit_completion(Shell::Zsh);
        return ExitCode::SUCCESS;
    }
    if cli.complete_fish {
        emit_completion(Shell::Fish);
        return ExitCode::SUCCESS;
    }

    let registry = Registry::new();

    if cli.list {
        print_list(&registry);
        return ExitCode::SUCCESS;
    }

    if let Some(style_name) = &cli.help_style {
        return print_help_style(&registry, style_name);
    }

    // ----- validations -----
    if let Some(s) = &cli.style {
        if registry.get(s).is_none() {
            eprintln!("error: {}", t_str("error-unknown-style", &[("name", s)]));
            return ExitCode::from(2);
        }
    }
    let color_override = match &cli.color {
        Some(c) => match Color::parse(c) {
            Ok(col) => Some(col),
            Err(_) => {
                eprintln!("error: {}", t_str("error-invalid-color", &[("spec", c)]));
                return ExitCode::from(2);
            }
        },
        None => None,
    };

    // ----- pick the style sequence -----
    let mut shuffle_keys: Vec<String> = registry.keys().map(str::to_string).collect();
    let mut rng = ChaCha8Rng::from_entropy();
    let initial_style: String = if cli.random {
        shuffle_keys.shuffle(&mut rng);
        shuffle_keys[0].clone()
    } else if let Some(s) = &cli.style {
        s.clone()
    } else {
        // No style + not random: default to logo-bounce.
        "logo-bounce".to_string()
    };

    // ----- mode dispatch -----
    let resolved_mode = resolve_mode(cli.mode);
    if resolved_mode == Mode::Wayland {
        // Wayland renderer lands in stage 4. For stage 3, fall back to
        // TTY with a warning so any hypridle-launched test still functions.
        eprintln!("note: --mode=wayland not yet wired; falling back to TTY (stage 3 surface)");
    }

    // ----- TTY frame loop -----
    let signal_listener = match SignalListener::install() {
        Ok(l) => l,
        Err(e) => {
            eprintln!("warning: could not install signal handlers: {e}");
            // Continue without; the only fallout is Ctrl-C exits abruptly.
            SignalListener::install().unwrap_or_else(|_| panic!("signal install retry failed"))
        }
    };
    let exit_flag = signal_listener.flag();

    // Validate per-style options against the chosen style's schema.
    let initial_factory = registry.get(&initial_style).expect("validated above");
    let mut style: Box<dyn Style> = initial_factory();
    let schema = style.option_schema();
    let mut opts = StyleOpts::from_defaults(schema);
    for kv in &cli.style_opts {
        if let Err(e) = opts.set(schema, kv) {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    }

    let logo = match Logo::load_default() {
        Ok(l) => l,
        Err(_) => {
            // /etc/shedos-ascii.txt missing — synthesize a tiny fallback so
            // the renderer still has something to show.
            Logo::parse(
                "███████\n█     █\n███████\n",
                std::path::PathBuf::from("fallback"),
            )
        }
    };

    let (rows, cols) = detect_terminal_size();
    let fps = cli.fps.unwrap_or(30).max(1);

    let _guard: Option<TerminalGuard> = if stdout_is_tty() && !cli.idle_daemon {
        match TerminalGuard::enter() {
            Ok(g) => Some(g),
            Err(e) => {
                eprintln!("warning: could not enter alt-screen + raw-mode: {e}; running anyway");
                None
            }
        }
    } else if stdout_is_tty() && cli.idle_daemon {
        // Idle-daemon mode still needs alt-screen + cursor-hide.
        match TerminalGuard::enter() {
            Ok(g) => Some(g),
            Err(_) => None,
        }
    } else {
        None
    };

    let stdout = io::stdout();
    let mut renderer = TtyRenderer::new(stdout.lock(), rows, cols);
    let clock = RealClock::new();
    let frame_budget = Duration::from_secs_f64(1.0 / fps as f64);
    let mut frame = Frame::new(rows, cols);
    let start = clock.elapsed();
    let mut last = start;
    let mut style_started = start;

    // Main loop.
    loop {
        if exit_flag.load(Ordering::Relaxed) {
            break;
        }
        if let Some(d) = cli.duration {
            if (clock.elapsed() - start).as_secs_f32() >= d {
                break;
            }
        }
        if let Some(s) = cli.shuffle {
            if (clock.elapsed() - style_started).as_secs() >= s as u64 {
                shuffle_keys.shuffle(&mut rng);
                let next = shuffle_keys[0].clone();
                if let Some(factory) = registry.get(&next) {
                    style = factory();
                    let new_schema = style.option_schema();
                    opts = StyleOpts::from_defaults(new_schema);
                    // Style-opts only apply to the user's chosen --style, not to
                    // shuffle picks; shuffle uses each style's own defaults.
                    style_started = clock.elapsed();
                    renderer.invalidate();
                }
            }
        }

        // Keypress exit (TTY mode only; idle-daemon ignores keypress).
        if !cli.idle_daemon && stdout_is_tty() {
            if event::poll(Duration::from_millis(0)).unwrap_or(false) {
                if let Ok(ev) = event::read() {
                    if matches!(ev, Event::Key(_) | Event::Resize(_, _)) {
                        if let Event::Key(ke) = ev {
                            if !matches!(ke.code, KeyCode::Null) {
                                break;
                            }
                        }
                    }
                }
            }
        }

        let now = clock.elapsed();
        let dt = now - last;
        last = now;
        frame.clear();

        let color = color_override.unwrap_or_else(|| style.default_color());
        let mut ctx = Ctx { t: now - style_started, dt, color, logo: &logo, opts: &opts, rng: &mut rng };
        style.draw(&mut frame, &mut ctx);
        if let Err(e) = renderer.submit(&frame) {
            eprintln!("renderer error: {e}");
            break;
        }

        // FPS pacing.
        let next_frame_at = now + frame_budget;
        let after_render = clock.elapsed();
        if next_frame_at > after_render {
            std::thread::sleep(next_frame_at - after_render);
        }
    }

    ExitCode::SUCCESS
}

fn resolve_mode(m: Mode) -> Mode {
    match m {
        Mode::Tty | Mode::Wayland => m,
        Mode::Auto => {
            // Prefer TTY when invoked from an interactive terminal, even if
            // WAYLAND_DISPLAY is set (terminal users probably want their
            // animation in the terminal). Choose Wayland only when stdout
            // isn't a TTY (the hypridle path).
            if stdout_is_tty() || std::env::var_os("WAYLAND_DISPLAY").is_none() {
                Mode::Tty
            } else {
                Mode::Wayland
            }
        }
    }
}

fn print_list(registry: &Registry) {
    println!("{}", t("list-header"));
    for key in registry.keys() {
        let style = registry.instantiate(key).expect("registry key returned None");
        let title_key = format!("style-{key}-title");
        let title = t(&title_key);
        let _ = style.default_color();
        let color_label = default_color_label(key);
        println!(
            "  {}",
            t_str(
                "list-style-line",
                &[("key", key), ("title", title.as_str()), ("color", color_label)],
            )
        );
    }
}

fn default_color_label(key: &str) -> &'static str {
    match key {
        "logo-bounce" | "tunnel" => "blue",
        "matrix" => "green",
        "plasma" | "waves" => "mauve",
        "starfield" => "text",
        "conway" | "mandala" => "peach",
        _ => "text",
    }
}

fn print_help_style(registry: &Registry, name: &str) -> ExitCode {
    let factory = match registry.get(name) {
        Some(f) => f,
        None => {
            eprintln!("error: {}", t_str("error-unknown-style", &[("name", name)]));
            return ExitCode::from(2);
        }
    };
    let style = factory();
    let schema = style.option_schema();
    println!("{}", t_str("help-style-header", &[("name", name)]));
    if schema.options.is_empty() {
        println!("  {}", t("help-style-no-options"));
        return ExitCode::SUCCESS;
    }
    for opt in schema.options {
        let default = match &opt.default {
            shedos_screensaver_styles::OptVal::Bool(b) => b.to_string(),
            shedos_screensaver_styles::OptVal::UInt(u) => u.to_string(),
            shedos_screensaver_styles::OptVal::Float(f) => format!("{f}"),
            shedos_screensaver_styles::OptVal::String(s) => s.clone(),
        };
        println!(
            "  {}",
            t_str(
                "help-style-line",
                &[
                    ("key", opt.key),
                    ("ty", opt.ty.label()),
                    ("default", default.as_str()),
                    ("desc", opt.desc),
                ],
            )
        );
    }
    ExitCode::SUCCESS
}

fn emit_completion(shell: Shell) {
    let mut cmd = Cli::command();
    let bin = cmd.get_name().to_string();
    clap_complete::generate(shell, &mut cmd, bin, &mut io::stdout());
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn registry_count_matches_plan() {
        assert_eq!(Registry::new().len(), 8);
    }

    #[test]
    fn cli_parses_all_documented_flags() {
        let _ = Cli::try_parse_from([
            "shedos-screensaver",
            "--style", "matrix",
            "--color", "#89b4fa",
            "--style-opt", "density=0.7",
            "--mode", "tty",
            "--fps", "30",
            "--duration", "0.5",
            "--audio-source", "desktop",
            "--wallpaper", "auto",
            "--wallpaper-dim", "0.3",
            "--locale", "en-US",
            "--shuffle", "60",
        ])
        .unwrap();
    }

    #[test]
    fn clap_command_is_well_formed() {
        Cli::command().debug_assert();
    }
}
