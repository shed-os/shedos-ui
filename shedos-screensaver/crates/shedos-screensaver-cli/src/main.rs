//! `shedos-screensaver` CLI binary.
//!
//! Both render backends (TTY + Wayland-native overlay) are wired,
//! along with optional pipewire audio reactivity and wallpaper
//! compositing in Wayland mode.

use clap::{ArgAction, CommandFactory, Parser, ValueEnum};
use clap_complete::Shell;
use crossterm::event::{self, Event};
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use shedos_screensaver_audio::{AudioCapture, Source as AudioSrc};
use shedos_screensaver_core::{Clock, Color, Frame, Logo, RealClock, SignalListener};
use shedos_screensaver_i18n::{t, t_str, I18n};
use shedos_screensaver_styles::{Ctx, Registry, Style, StyleOpts};
use shedos_screensaver_tty::{detect_terminal_size, stdout_is_tty, TerminalGuard, TtyRenderer};
use shedos_screensaver_wayland::{FrameProducer, WaylandConfig, WaylandRenderer};
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
enum Mode {
    Tty,
    Wayland,
    Auto,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
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

    /// Wayland mode background image. `auto` uses
    /// ~/.config/hypr/wallpaper.png if present; `none` disables.
    #[arg(long, value_name = "PATH|none|auto", default_value = "auto")]
    wallpaper: String,

    #[arg(long, value_name = "F", default_value_t = 0.3)]
    wallpaper_dim: f32,

    #[arg(long, value_name = "BCP47")]
    locale: Option<String>,

    /// Wayland-mode font path; defaults to system DejaVu Sans Mono
    /// (looked up under /usr/share/fonts/TTF/, /usr/share/fonts/dejavu/,
    /// or /usr/share/fonts/truetype/dejavu/).
    #[arg(long, value_name = "PATH")]
    font_path: Option<PathBuf>,

    /// Wayland-mode cell pixel height (cell width derived from font metrics).
    #[arg(long, value_name = "PX", default_value_t = 18)]
    cell_height_px: u32,
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
        "logo-bounce".to_string()
    };

    // ----- shared state across renderer backends -----
    let signal_listener =
        SignalListener::install().unwrap_or_else(|e| panic!("signal install: {e}"));
    let exit_flag = signal_listener.flag();

    let initial_factory = registry.get(&initial_style).expect("validated above");
    let style: Box<dyn Style> = initial_factory();
    let schema = style.option_schema();
    let mut opts = StyleOpts::from_defaults(schema);
    for kv in &cli.style_opts {
        if let Err(e) = opts.set(schema, kv) {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    }

    // Always returns the real SHEDOS art: tries /etc/shedos-ascii.txt first
    // (so shedos-branding live-updates are picked up), falls back to the
    // compile-time embedded copy if the file is missing.
    let logo = Logo::load_default();

    // Audio capture (live, or unconfigured).
    let audio = match cli.audio_source {
        AudioSource::None => None,
        AudioSource::Desktop => Some(AudioCapture::start(AudioSrc::Desktop)),
        AudioSource::Mic => Some(AudioCapture::start(AudioSrc::Mic)),
    };
    if let Some(cap) = &audio {
        if !cap.available() {
            eprintln!(
                "warning: pipewire not reachable; --audio-source ignored. \
                 See /usr/share/doc/shedos-screensaver/audio-setup.md."
            );
        }
    }

    // ----- mode dispatch -----
    let resolved_mode = resolve_mode(cli.mode);
    let result = match resolved_mode {
        Mode::Wayland => run_wayland(
            &cli,
            registry,
            initial_style.clone(),
            opts,
            color_override,
            logo,
            shuffle_keys,
            audio,
            exit_flag.clone(),
        ),
        Mode::Tty | Mode::Auto => run_tty(
            &cli,
            registry,
            style,
            initial_style,
            opts,
            color_override,
            logo,
            shuffle_keys,
            audio,
            exit_flag,
        ),
    };

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(1)
        }
    }
}

fn resolve_mode(m: Mode) -> Mode {
    match m {
        Mode::Tty | Mode::Wayland => m,
        Mode::Auto => {
            if stdout_is_tty() || std::env::var_os("WAYLAND_DISPLAY").is_none() {
                Mode::Tty
            } else {
                Mode::Wayland
            }
        }
    }
}

// ============= TTY mode =============

fn run_tty(
    cli: &Cli,
    registry: Registry,
    mut style: Box<dyn Style>,
    initial_style: String,
    mut opts: StyleOpts,
    color_override: Option<Color>,
    logo: Logo,
    mut shuffle_keys: Vec<String>,
    audio: Option<AudioCapture>,
    exit_flag: Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), String> {
    let (rows, cols) = detect_terminal_size();
    let fps = cli.fps.unwrap_or(30).max(1);
    let mut rng = ChaCha8Rng::from_entropy();

    let _guard: Option<TerminalGuard> = if stdout_is_tty() {
        match TerminalGuard::enter() {
            Ok(g) => Some(g),
            Err(e) => {
                eprintln!("warning: alt-screen+raw-mode: {e}; running anyway");
                None
            }
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
    let _ = initial_style;

    loop {
        if exit_flag.load(Ordering::Acquire) {
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
                    style_started = clock.elapsed();
                    renderer.invalidate();
                }
            }
        }

        // Keypress / resize exit (idle-daemon ignores keypress).
        if !cli.idle_daemon && stdout_is_tty()
            && event::poll(Duration::from_millis(0)).unwrap_or(false)
        {
            if let Ok(ev) = event::read() {
                match ev {
                    Event::Key(_) | Event::Mouse(_) => break,
                    Event::Resize(new_cols, new_rows) => {
                        renderer.resize(new_rows, new_cols);
                        frame = Frame::new(new_rows, new_cols);
                    }
                    _ => {}
                }
            }
        }

        let now = clock.elapsed();
        let dt = now - last;
        last = now;
        frame.clear();

        let color = color_override.unwrap_or_else(|| style.default_color());
        let audio_frame = audio.as_ref().filter(|c| c.available()).map(|c| c.latest());
        let mut ctx = Ctx {
            t: now - style_started,
            dt,
            color,
            logo: &logo,
            opts: &opts,
            rng: &mut rng,
            audio: audio_frame.as_ref(),
        };
        style.draw(&mut frame, &mut ctx);
        renderer
            .submit(&frame)
            .map_err(|e| format!("tty submit: {e}"))?;

        let next_frame_at = now + frame_budget;
        let after_render = clock.elapsed();
        if next_frame_at > after_render {
            std::thread::sleep(next_frame_at - after_render);
        }
    }

    Ok(())
}

// ============= Wayland mode =============

/// Drives the Wayland renderer by handing it a producer that pulls
/// from the same registry/style/options pipeline as TTY mode.
fn run_wayland(
    cli: &Cli,
    registry: Registry,
    initial_style: String,
    opts: StyleOpts,
    color_override: Option<Color>,
    logo: Logo,
    shuffle_keys: Vec<String>,
    audio: Option<AudioCapture>,
    exit_flag: Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), String> {
    let wallpaper_path = resolve_wallpaper(&cli.wallpaper);
    let cfg = WaylandConfig {
        font_path: cli.font_path.clone(),
        cell_height_px: cli.cell_height_px,
        wallpaper_path,
        wallpaper_dim: cli.wallpaper_dim,
        fps_cap: cli.fps.unwrap_or(60).max(1),
        idle_daemon: cli.idle_daemon,
    };

    // Producer state shared across frame callbacks.
    let producer = StyleProducer {
        registry,
        current_key: initial_style.clone(),
        current: registry_get_or_default(&initial_style),
        opts: Arc::new(Mutex::new(opts)),
        color_override,
        logo,
        shuffle_keys,
        shuffle_secs: cli.shuffle,
        rng: ChaCha8Rng::from_entropy(),
        audio,
        clock: RealClock::new(),
        style_started: Duration::ZERO,
        last_frame: Duration::ZERO,
        duration: cli.duration,
        start: Duration::ZERO,
        first: true,
        exit_flag: Arc::clone(&exit_flag),
    };

    // Start a watchdog thread that flips exit_flag when --duration expires.
    if let Some(d) = cli.duration {
        let f = Arc::clone(&exit_flag);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs_f32(d));
            f.store(true, Ordering::Release);
        });
    }

    WaylandRenderer::run(cfg, Box::new(producer), exit_flag).map_err(|e| format!("wayland: {e}"))
}

fn registry_get_or_default(key: &str) -> Box<dyn Style> {
    Registry::new()
        .instantiate(key)
        .unwrap_or_else(|| Registry::new().instantiate("logo-bounce").expect("logo-bounce always present"))
}

fn resolve_wallpaper(arg: &str) -> Option<PathBuf> {
    match arg {
        "none" => None,
        "auto" => {
            // ~/.config/hypr/wallpaper.png if present.
            if let Some(home) = std::env::var_os("HOME") {
                let p = PathBuf::from(home)
                    .join(".config/hypr/wallpaper.png");
                if p.exists() {
                    return Some(p);
                }
            }
            None
        }
        path => Some(PathBuf::from(path)),
    }
}

struct StyleProducer {
    registry: Registry,
    current_key: String,
    current: Box<dyn Style>,
    opts: Arc<Mutex<StyleOpts>>,
    color_override: Option<Color>,
    logo: Logo,
    shuffle_keys: Vec<String>,
    shuffle_secs: Option<u32>,
    rng: ChaCha8Rng,
    audio: Option<AudioCapture>,
    clock: RealClock,
    style_started: Duration,
    last_frame: Duration,
    duration: Option<f32>,
    start: Duration,
    first: bool,
    exit_flag: Arc<std::sync::atomic::AtomicBool>,
}

impl FrameProducer for StyleProducer {
    fn produce(&mut self, frame: &mut Frame) {
        let now = self.clock.elapsed();
        if self.first {
            self.start = now;
            self.style_started = now;
            self.last_frame = now;
            self.first = false;
        }
        if let Some(d) = self.duration {
            if (now - self.start).as_secs_f32() >= d {
                self.exit_flag.store(true, Ordering::Release);
            }
        }
        if let Some(s) = self.shuffle_secs {
            if (now - self.style_started).as_secs() >= s as u64 {
                self.shuffle_keys.shuffle(&mut self.rng);
                let next = self.shuffle_keys[0].clone();
                if let Some(f) = self.registry.get(&next) {
                    self.current = f();
                    self.current_key = next;
                    let new_schema = self.current.option_schema();
                    if let Ok(mut o) = self.opts.lock() {
                        *o = StyleOpts::from_defaults(new_schema);
                    }
                    self.style_started = now;
                }
            }
        }
        let dt = now - self.last_frame;
        self.last_frame = now;
        let color = self.color_override.unwrap_or_else(|| self.current.default_color());
        let audio_frame = self.audio.as_ref().filter(|c| c.available()).map(|c| c.latest());
        let opts = self.opts.lock().expect("opts lock");
        let mut ctx = Ctx {
            t: now - self.style_started,
            dt,
            color,
            logo: &self.logo,
            opts: &opts,
            rng: &mut self.rng,
            audio: audio_frame.as_ref(),
        };
        self.current.draw(frame, &mut ctx);
    }
}

// ============= read-only print helpers =============

fn print_list(registry: &Registry) {
    println!("{}", t("list-header"));
    for key in registry.keys() {
        let title_key = format!("style-{key}-title");
        let title = t(&title_key);
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
            "--font-path", "/tmp/fake.ttf",
            "--cell-height-px", "20",
        ])
        .unwrap();
    }

    #[test]
    fn clap_command_is_well_formed() {
        Cli::command().debug_assert();
    }

    #[test]
    fn resolve_wallpaper_none_returns_none() {
        assert_eq!(resolve_wallpaper("none"), None);
    }

    #[test]
    fn resolve_wallpaper_explicit_path_passes_through() {
        let p = resolve_wallpaper("/tmp/wp.png");
        assert_eq!(p, Some(PathBuf::from("/tmp/wp.png")));
    }

    #[test]
    fn resolve_mode_explicit_returns_as_is() {
        assert_eq!(resolve_mode(Mode::Tty), Mode::Tty);
        assert_eq!(resolve_mode(Mode::Wayland), Mode::Wayland);
    }
}
