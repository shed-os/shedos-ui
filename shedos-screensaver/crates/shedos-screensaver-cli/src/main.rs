//! `shedos-screensaver` CLI binary.
//!
//! Stage-2 surface: parsing, --help, --help-summary, --list,
//! --help-style, --version, --complete-{bash,zsh,fish}, --color
//! validation, --style validation against the const style table.
//! Actual frame loop and renderers land in stages 3+.

use clap::{ArgAction, CommandFactory, Parser, ValueEnum};
use clap_complete::Shell;
use shedos_screensaver_core::Color;
use shedos_screensaver_i18n::{t, t_str, I18n};
use std::io;
use std::process::ExitCode;

const STYLE_KEYS: &[&str] = &[
    "logo-bounce",
    "matrix",
    "plasma",
    "starfield",
    "conway",
    "tunnel",
    "waves",
    "mandala",
];

#[derive(Debug, Clone, Copy, ValueEnum)]
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
    /// Print a one-line summary and exit (used by `shedman help`).
    #[arg(long)]
    help_summary: bool,

    /// List all available styles and exit.
    #[arg(long)]
    list: bool,

    /// Print the option schema for one style and exit.
    #[arg(long, value_name = "NAME")]
    help_style: Option<String>,

    /// Emit a bash completion script and exit.
    #[arg(long)]
    complete_bash: bool,

    /// Emit a zsh completion script and exit.
    #[arg(long)]
    complete_zsh: bool,

    /// Emit a fish completion script and exit.
    #[arg(long)]
    complete_fish: bool,

    /// Style to render (one of: logo-bounce, matrix, plasma, starfield,
    /// conway, tunnel, waves, mandala). Required unless --random or
    /// [defaults].style is set in /etc/shedos/screensaver.toml.
    #[arg(long, value_name = "NAME")]
    style: Option<String>,

    /// Color override; accepts #rrggbb, r,g,b, named ANSI, or
    /// Catppuccin Mocha shorthand (blue/mauve/peach/text/...).
    #[arg(long, value_name = "SPEC")]
    color: Option<String>,

    /// Per-style typed override; repeatable. Example: --style-opt density=0.7
    #[arg(long = "style-opt", value_name = "KEY=VAL", action = ArgAction::Append)]
    style_opts: Vec<String>,

    /// Render backend.
    #[arg(long, value_enum, default_value_t = Mode::Auto)]
    mode: Mode,

    /// Frames per second; 30 default for TTY, 60 default for Wayland.
    #[arg(long, value_name = "N")]
    fps: Option<u32>,

    /// Auto-exit after this many seconds (used by tests).
    #[arg(long, value_name = "SECS")]
    duration: Option<f32>,

    /// Pick a random style at start.
    #[arg(long)]
    random: bool,

    /// Rotate styles every N seconds.
    #[arg(long, value_name = "SECS")]
    shuffle: Option<u32>,

    /// Long-running mode for hypridle: ignores keyboard, only exits on SIGUSR1.
    #[arg(long)]
    idle_daemon: bool,

    /// Audio reactivity source.
    #[arg(long, value_enum, default_value_t = AudioSource::None)]
    audio_source: AudioSource,

    /// Wallpaper for Wayland background layer; "auto" uses ~/.config/hypr/wallpaper.png; "none" disables.
    #[arg(long, value_name = "PATH|none|auto", default_value = "auto")]
    wallpaper: String,

    /// Wallpaper backdrop dimming factor (0.0..=1.0).
    #[arg(long, value_name = "F", default_value_t = 0.3)]
    wallpaper_dim: f32,

    /// Override system locale (BCP-47, e.g. en-US, fr-FR).
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

    if cli.list {
        print_list();
        return ExitCode::SUCCESS;
    }

    if let Some(style_name) = &cli.help_style {
        return print_help_style(style_name);
    }

    // Validations (stage 2 surface): style + color + style-opt syntax.
    // Renderer dispatch lands in stages 3-5.
    if let Some(s) = &cli.style {
        if !STYLE_KEYS.contains(&s.as_str()) {
            eprintln!("error: {}", t_str("error-unknown-style", &[("name", s)]));
            return ExitCode::from(2);
        }
    }

    if let Some(c) = &cli.color {
        if Color::parse(c).is_err() {
            eprintln!("error: {}", t_str("error-invalid-color", &[("spec", c)]));
            return ExitCode::from(2);
        }
    }

    for opt in &cli.style_opts {
        if !opt.contains('=') || opt.starts_with('=') || opt.ends_with('=') {
            eprintln!("error: {}", t_str("error-invalid-style-opt", &[("arg", opt)]));
            return ExitCode::from(2);
        }
    }

    // Stage 2 ends here. Future stages dispatch into TtyRenderer or
    // WaylandRenderer. For now, surface a clear "not yet wired" notice
    // ONLY when invoked without one of the read-only flags above —
    // tests T1-T10 hit only those paths.
    eprintln!(
        "shedos-screensaver: renderer backends are not wired yet (stage 2 surface). \
         CLI parsed cleanly: style={:?}, color={:?}, mode={:?}.",
        cli.style, cli.color, cli.mode
    );
    ExitCode::SUCCESS
}

fn print_list() {
    println!("{}", t("list-header"));
    for &key in STYLE_KEYS {
        let title_key = format!("style-{key}-title");
        let title = t(&title_key);
        let color = default_color_label(key);
        println!(
            "  {}",
            t_str(
                "list-style-line",
                &[("key", key), ("title", title.as_str()), ("color", color)],
            )
        );
    }
}

/// Per-style default color labels (mirrors the style table from the plan;
/// real Style trait impls land in stage 3 and will replace this lookup).
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

fn print_help_style(name: &str) -> ExitCode {
    if !STYLE_KEYS.contains(&name) {
        eprintln!("error: {}", t_str("error-unknown-style", &[("name", name)]));
        return ExitCode::from(2);
    }
    println!("{}", t_str("help-style-header", &[("name", name)]));
    for opt in style_options(name) {
        println!(
            "  {}",
            t_str(
                "help-style-line",
                &[
                    ("key", opt.key),
                    ("ty", opt.ty),
                    ("default", opt.default),
                    ("desc", opt.desc),
                ],
            )
        );
    }
    ExitCode::SUCCESS
}

#[derive(Debug, Clone, Copy)]
struct StyleOptionDoc {
    key: &'static str,
    ty: &'static str,
    default: &'static str,
    desc: &'static str,
}

/// Per-style option schema docs — stage 2 const table.
/// Stage 3 wires this into the real Style trait via OptionSchema.
fn style_options(name: &str) -> &'static [StyleOptionDoc] {
    match name {
        "matrix" => &[
            StyleOptionDoc { key: "density", ty: "f32 (0.0..=1.0)", default: "0.5", desc: "probability per column per frame of starting a new trail" },
            StyleOptionDoc { key: "trail_length", ty: "u32 (1..=100)", default: "20", desc: "trail length in cells" },
            StyleOptionDoc { key: "glyphs", ty: "enum", default: "katakana", desc: "katakana | ascii | hex | brand" },
        ],
        "conway" => &[
            StyleOptionDoc { key: "rule", ty: "str (B/S notation)", default: "B3/S23", desc: "Game of Life rule" },
            StyleOptionDoc { key: "reseed_interval", ty: "u32 (1..=600)", default: "30", desc: "reseed from logo every N seconds" },
        ],
        "plasma" => &[
            StyleOptionDoc { key: "freq_x", ty: "f32 (0.1..=10.0)", default: "1.0", desc: "X-axis spatial frequency" },
            StyleOptionDoc { key: "freq_y", ty: "f32 (0.1..=10.0)", default: "1.5", desc: "Y-axis spatial frequency" },
        ],
        "starfield" => &[
            StyleOptionDoc { key: "count", ty: "u32 (1..=10000)", default: "200", desc: "number of stars" },
            StyleOptionDoc { key: "warp_factor", ty: "f32 (1.0..=100.0)", default: "5.0", desc: "speed of perspective motion" },
        ],
        "logo-bounce" => &[
            StyleOptionDoc { key: "speed", ty: "f32 (0.1..=10.0)", default: "1.0", desc: "multiplier on bounce velocity" },
            StyleOptionDoc { key: "color_cycle", ty: "bool", default: "true", desc: "shift color on each wall hit" },
        ],
        "tunnel" => &[
            StyleOptionDoc { key: "rings", ty: "u32 (5..=50)", default: "20", desc: "number of concentric rings" },
            StyleOptionDoc { key: "speed", ty: "f32 (0.1..=10.0)", default: "1.0", desc: "inward zoom speed multiplier" },
        ],
        "waves" => &[
            StyleOptionDoc { key: "wavelength_x", ty: "f32 (0.1..=10.0)", default: "1.0", desc: "X-axis wavelength" },
            StyleOptionDoc { key: "wavelength_y", ty: "f32 (0.1..=10.0)", default: "1.5", desc: "Y-axis wavelength" },
            StyleOptionDoc { key: "speed", ty: "f32 (0.1..=10.0)", default: "1.0", desc: "phase advance per second" },
        ],
        "mandala" => &[
            StyleOptionDoc { key: "symmetry", ty: "u32 (2..=16)", default: "8", desc: "N-fold rotational symmetry" },
            StyleOptionDoc { key: "growth", ty: "f32 (0.1..=10.0)", default: "1.0", desc: "growth speed of kernel" },
        ],
        _ => &[],
    }
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
    fn style_keys_match_plan() {
        // The plan pins exactly 8 styles; any drift from this list is a bug.
        assert_eq!(STYLE_KEYS.len(), 8);
        let expected = [
            "logo-bounce",
            "matrix",
            "plasma",
            "starfield",
            "conway",
            "tunnel",
            "waves",
            "mandala",
        ];
        for key in expected {
            assert!(STYLE_KEYS.contains(&key), "missing style key: {key}");
        }
    }

    #[test]
    fn every_style_has_an_options_table() {
        for &k in STYLE_KEYS {
            let opts = style_options(k);
            assert!(!opts.is_empty(), "style {k} has no options table");
        }
    }

    #[test]
    fn unknown_style_options_returns_empty() {
        assert!(style_options("nope").is_empty());
    }

    #[test]
    fn cli_parses_all_documented_flags() {
        // Smoke test: every flag the CLI advertises must parse.
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
        // Sanity: clap's own validation catches contradictory definitions.
        Cli::command().debug_assert();
    }
}
