//! `shedos-screensaver` CLI binary.
//!
//! Architecture: an [`Engine`] cycles through (LogoVariant, Effect)
//! pairs. Each cycle picks a logo + an effect (both random by
//! default; `--logo=NAME` and/or `--effect=NAME` lock either axis),
//! renders the logo to a target Frame, runs the effect to completion
//! against that target, then holds the resolved art for `--hold`
//! seconds before picking a new pair. The animation IS how the
//! SHEDOS art appears.

use clap::{ArgAction, CommandFactory, Parser, ValueEnum};
use clap_complete::Shell;
use crossterm::event::{self, Event};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use shedos_screensaver_audio::{AudioCapture, Source as AudioSrc};
use shedos_screensaver_core::{Clock, Color, Frame, Logo, RealClock, SignalListener};
use shedos_screensaver_effects::{target, Effect, EffectCtx, Registry as EffectsRegistry};
use shedos_screensaver_i18n::{t, t_str, I18n};
use shedos_screensaver_logos::{self as logos, LogoVariant};
use shedos_screensaver_tty::{detect_terminal_size, stdout_is_tty, TerminalGuard, TtyRenderer};
use shedos_screensaver_wayland::{FrameProducer, ProducerFactory, WaylandConfig, WaylandRenderer};
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::Ordering;
use std::sync::Arc;
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
    about = "Animated SHEDOS screensaver with TTY + Wayland backends, 8 logo variants × 16 forming effects",
    long_about = None,
)]
struct Cli {
    /// Print a one-line summary and exit (used by `shedman help`).
    #[arg(long)]
    help_summary: bool,

    /// List all available effects.
    #[arg(long, alias = "list-effects")]
    list: bool,

    /// List all available logo variants.
    #[arg(long)]
    list_logos: bool,

    /// Print the description of a specific effect.
    #[arg(long, value_name = "NAME")]
    help_effect: Option<String>,

    /// Emit a bash completion script and exit.
    #[arg(long)]
    complete_bash: bool,

    /// Emit a zsh completion script and exit.
    #[arg(long)]
    complete_zsh: bool,

    /// Emit a fish completion script and exit.
    #[arg(long)]
    complete_fish: bool,

    /// Lock to a specific effect (otherwise: random each cycle).
    #[arg(long, value_name = "NAME")]
    effect: Option<String>,

    /// Lock to a specific logo variant (otherwise: random each cycle).
    #[arg(long, value_name = "VARIANT")]
    logo: Option<String>,

    /// Override the target color (one for the entire session).
    #[arg(long, value_name = "SPEC")]
    color: Option<String>,

    /// Render backend.
    #[arg(long, value_enum, default_value_t = Mode::Auto)]
    mode: Mode,

    /// Frames per second.
    #[arg(long, value_name = "N")]
    fps: Option<u32>,

    /// Auto-exit after this many seconds.
    #[arg(long, value_name = "SECS")]
    duration: Option<f32>,

    /// Hold the resolved art for this many seconds between effects.
    /// 0 = one-shot mode: animate once and sit on the resolved art
    /// until --duration / signal / keypress.
    #[arg(long, value_name = "SECS", default_value_t = 3.0)]
    hold: f32,

    /// Long-running mode: ignore keypress, exit only on SIGUSR1.
    #[arg(long)]
    idle_daemon: bool,

    /// Audio reactivity source.
    #[arg(long, value_enum, default_value_t = AudioSource::None)]
    audio_source: AudioSource,

    /// Wayland-mode background image (`auto` uses ~/.config/hypr/wallpaper.png).
    #[arg(long, value_name = "PATH|none|auto", default_value = "auto")]
    wallpaper: String,

    /// Wayland-mode wallpaper dim multiplier.
    #[arg(long, value_name = "F", default_value_t = 0.3)]
    wallpaper_dim: f32,

    /// Locale override (BCP-47).
    #[arg(long, value_name = "BCP47")]
    locale: Option<String>,

    /// Wayland-mode font path (defaults to system DejaVu Sans Mono).
    #[arg(long, value_name = "PATH")]
    font_path: Option<PathBuf>,

    /// Wayland-mode cell pixel height.
    #[arg(long, value_name = "PX", default_value_t = 18)]
    cell_height_px: u32,

    /// Repeatable: one or more effect names to cycle through (random
    /// order). Lets you curate a subset like
    /// `--cycle rain --cycle decrypt --cycle matrix-rain`.
    #[arg(long = "cycle", value_name = "NAME", action = ArgAction::Append)]
    cycle: Vec<String>,
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

    let effects = EffectsRegistry::new();

    if cli.list {
        print_effects_list(&effects);
        return ExitCode::SUCCESS;
    }
    if cli.list_logos {
        print_logos_list();
        return ExitCode::SUCCESS;
    }
    if let Some(name) = &cli.help_effect {
        return print_help_effect(&effects, name);
    }

    // ----- validations -----
    if let Some(e) = &cli.effect {
        if effects.get(e).is_none() {
            eprintln!("error: {}", t_str("error-unknown-effect", &[("name", e)]));
            return ExitCode::from(2);
        }
    }
    if let Some(l) = &cli.logo {
        if logos::by_name(l).is_none() {
            eprintln!("error: {}", t_str("error-unknown-logo", &[("name", l)]));
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
    for name in &cli.cycle {
        if effects.get(name).is_none() {
            eprintln!("error: {}", t_str("error-unknown-effect", &[("name", name)]));
            return ExitCode::from(2);
        }
    }

    // Signal handling for graceful exit.
    let signal_listener =
        SignalListener::install().unwrap_or_else(|e| panic!("signal install: {e}"));
    let exit_flag = signal_listener.flag();

    // Audio capture.
    let audio = match cli.audio_source {
        AudioSource::None => None,
        AudioSource::Desktop => Some(AudioCapture::start(AudioSrc::Desktop)),
        AudioSource::Mic => Some(AudioCapture::start(AudioSrc::Mic)),
    };
    if let Some(cap) = &audio {
        if !cap.available() {
            eprintln!(
                "warning: pipewire/ALSA not reachable; --audio-source ignored. \
                 See /usr/share/doc/shedos-screensaver/audio-setup.md."
            );
        }
    }

    // Mode dispatch.
    let resolved_mode = resolve_mode(cli.mode);
    let result = match resolved_mode {
        Mode::Wayland => run_wayland(&cli, color_override, audio, Arc::clone(&exit_flag)),
        Mode::Tty | Mode::Auto => run_tty(&cli, color_override, audio, Arc::clone(&exit_flag)),
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

// ============= Engine =============

/// Drives the (logo, effect) cycling. Holds all per-session state
/// so frame loops in either backend just call `produce(frame)`.
struct Engine {
    forced_logo: Option<String>,
    forced_effect: Option<String>,
    cycle: Vec<String>,
    color_override: Option<Color>,
    rng: ChaCha8Rng,
    hold: Duration,
    audio: Option<AudioCapture>,
    state: EngineState,
}

enum EngineState {
    /// First frame — pick the initial pair.
    Booting,
    /// Effect is currently animating toward `target`.
    Animating { effect: Box<dyn Effect>, target: Frame },
    /// Effect finished; show the resolved art until `holds_remaining` runs out.
    Holding { target: Frame, holds_remaining: Duration },
}

impl Engine {
    fn new(
        forced_logo: Option<String>,
        forced_effect: Option<String>,
        cycle: Vec<String>,
        color_override: Option<Color>,
        hold: Duration,
        audio: Option<AudioCapture>,
    ) -> Self {
        Self {
            forced_logo,
            forced_effect,
            cycle,
            color_override,
            rng: ChaCha8Rng::from_entropy(),
            hold,
            audio,
            state: EngineState::Booting,
        }
    }

    fn pick_logo(&mut self, rows: u16, cols: u16) -> &'static LogoVariant {
        if let Some(name) = &self.forced_logo {
            return logos::by_name(name).expect("validated at startup");
        }
        logos::pick_random_for_canvas(&mut self.rng, rows, cols)
    }

    fn pick_effect(&mut self, registry: &EffectsRegistry) -> Box<dyn Effect> {
        if let Some(name) = &self.forced_effect {
            return registry.instantiate(name).expect("validated at startup");
        }
        if !self.cycle.is_empty() {
            use rand::seq::SliceRandom;
            let name = self.cycle.choose(&mut self.rng).expect("non-empty");
            return registry.instantiate(name).expect("validated at startup");
        }
        // Pick uniformly from the full registry.
        use rand::seq::IteratorRandom;
        let name = registry.keys().choose(&mut self.rng).expect("non-empty");
        registry.instantiate(name).expect("registry consistency")
    }

    fn start_session(&mut self, registry: &EffectsRegistry, rows: u16, cols: u16) {
        let logo_variant = self.pick_logo(rows, cols);
        let logo = logo_variant.load();
        let fg = self.color_override.unwrap_or(logo_variant.default_color);
        let target = target::build_target(rows, cols, &logo, fg);

        let mut effect = self.pick_effect(registry);
        let mut ctx = EffectCtx { final_color: fg, rng: &mut self.rng };
        effect.setup(&target, &mut ctx);

        self.state = EngineState::Animating { effect, target };
    }

    /// Drive one frame. Frame size determines target dimensions.
    fn produce(&mut self, frame: &mut Frame, registry: &EffectsRegistry, dt: Duration) {
        let rows = frame.rows();
        let cols = frame.cols();
        let audio_frame = self
            .audio
            .as_ref()
            .filter(|c| c.available())
            .map(|c| c.latest());

        loop {
            match &mut self.state {
                EngineState::Booting => {
                    self.start_session(registry, rows, cols);
                }
                EngineState::Animating { effect, target } => {
                    // If frame size has changed, restart the session
                    // with new target dimensions.
                    if target.rows() != rows || target.cols() != cols {
                        self.start_session(registry, rows, cols);
                        continue;
                    }
                    frame.clear();
                    let done = effect.step(frame, dt, audio_frame.as_ref());
                    if done {
                        // Snap canvas to exact target so the held image
                        // looks pristine regardless of effect finish-state.
                        frame.clone_from(target);
                        self.state = EngineState::Holding {
                            target: target.clone(),
                            holds_remaining: self.hold,
                        };
                    }
                    return;
                }
                EngineState::Holding { target, holds_remaining } => {
                    if target.rows() != rows || target.cols() != cols {
                        self.start_session(registry, rows, cols);
                        continue;
                    }
                    frame.clone_from(target);
                    // `--hold 0` means "one-shot": animate to completion,
                    // then sit on the resolved art forever (or until
                    // SIGINT / SIGUSR1 / a keypress / `--duration`).
                    // Without this, hold=0 restarts the animation every
                    // frame so the completed art flashes for ~33 ms
                    // (one frame) and disappears, which feels broken.
                    // Cycling mode (--hold > 0) is unchanged.
                    if self.hold.is_zero() {
                        return;
                    }
                    if dt >= *holds_remaining {
                        self.start_session(registry, rows, cols);
                    } else {
                        *holds_remaining -= dt;
                    }
                    return;
                }
            }
        }
    }
}

// ============= TTY mode =============

fn run_tty(
    cli: &Cli,
    color_override: Option<Color>,
    audio: Option<AudioCapture>,
    exit_flag: Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), String> {
    let (rows, cols) = detect_terminal_size();
    let fps = cli.fps.unwrap_or(30).max(1);
    let mut engine = Engine::new(
        cli.logo.clone(),
        cli.effect.clone(),
        cli.cycle.clone(),
        color_override,
        Duration::from_secs_f32(cli.hold.max(0.0)),
        audio,
    );
    let registry = EffectsRegistry::new();

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

    loop {
        if exit_flag.load(Ordering::Acquire) {
            break;
        }
        if let Some(d) = cli.duration {
            if (clock.elapsed() - start).as_secs_f32() >= d {
                break;
            }
        }

        if !cli.idle_daemon
            && stdout_is_tty()
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

        engine.produce(&mut frame, &registry, dt);
        renderer.submit(&frame).map_err(|e| format!("tty submit: {e}"))?;

        let next_frame_at = now + frame_budget;
        let after_render = clock.elapsed();
        if next_frame_at > after_render {
            std::thread::sleep(next_frame_at - after_render);
        }
    }

    Ok(())
}

// ============= Wayland mode =============

fn run_wayland(
    cli: &Cli,
    color_override: Option<Color>,
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

    // The renderer mints one producer per output it discovers. The
    // captures below are all clonable / copy-able except `audio`,
    // which owns a cpal Stream we can't duplicate. The closure
    // `take()`s the audio Option on its first call, so the first
    // output to come up gets audio-reactive effects; subsequent
    // outputs run their effects' silence-fallback path.
    let logo = cli.logo.clone();
    let effect = cli.effect.clone();
    let cycle = cli.cycle.clone();
    let hold = Duration::from_secs_f32(cli.hold.max(0.0));
    let duration = cli.duration;
    let exit_for_factory = Arc::clone(&exit_flag);
    let mut audio_one_shot = audio;

    let factory: ProducerFactory = Box::new(move || {
        Box::new(EngineProducer {
            engine: Engine::new(
                logo.clone(),
                effect.clone(),
                cycle.clone(),
                color_override,
                hold,
                audio_one_shot.take(),
            ),
            registry: EffectsRegistry::new(),
            clock: RealClock::new(),
            last_frame: Duration::ZERO,
            first: true,
            exit_flag: Arc::clone(&exit_for_factory),
            duration,
            start: Duration::ZERO,
        }) as Box<dyn FrameProducer>
    });

    if let Some(d) = cli.duration {
        let f = Arc::clone(&exit_flag);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs_f32(d));
            f.store(true, Ordering::Release);
        });
    }

    WaylandRenderer::run(cfg, factory, exit_flag).map_err(|e| format!("wayland: {e}"))
}

fn resolve_wallpaper(arg: &str) -> Option<PathBuf> {
    match arg {
        "none" => None,
        "auto" => {
            if let Some(home) = std::env::var_os("HOME") {
                let p = PathBuf::from(home).join(".config/hypr/wallpaper.png");
                if p.exists() {
                    return Some(p);
                }
            }
            None
        }
        path => Some(PathBuf::from(path)),
    }
}

struct EngineProducer {
    engine: Engine,
    registry: EffectsRegistry,
    clock: RealClock,
    last_frame: Duration,
    first: bool,
    exit_flag: Arc<std::sync::atomic::AtomicBool>,
    duration: Option<f32>,
    start: Duration,
}

impl FrameProducer for EngineProducer {
    fn produce(&mut self, frame: &mut Frame) {
        let now = self.clock.elapsed();
        if self.first {
            self.start = now;
            self.last_frame = now;
            self.first = false;
        }
        if let Some(d) = self.duration {
            if (now - self.start).as_secs_f32() >= d {
                self.exit_flag.store(true, Ordering::Release);
            }
        }
        let dt = now - self.last_frame;
        self.last_frame = now;
        self.engine.produce(frame, &self.registry, dt);
    }
}

// ============= read-only print helpers =============

fn print_effects_list(registry: &EffectsRegistry) {
    println!("{}", t("list-effects-header"));
    for key in registry.keys() {
        let effect = registry.instantiate(key).expect("registry consistency");
        let dur_ms = effect.duration().as_millis();
        println!(
            "  {}",
            t_str(
                "list-effect-line",
                &[
                    ("key", key),
                    ("title", effect.title()),
                    ("description", effect.description()),
                    ("duration_ms", &dur_ms.to_string()),
                ],
            )
        );
    }
}

fn print_logos_list() {
    println!("{}", t("list-logos-header"));
    for v in logos::LIBRARY {
        println!(
            "  {}",
            t_str(
                "list-logo-line",
                &[
                    ("key", v.name),
                    ("title", v.title),
                    ("description", v.description),
                ],
            )
        );
    }
}

fn print_help_effect(registry: &EffectsRegistry, name: &str) -> ExitCode {
    let factory = match registry.get(name) {
        Some(f) => f,
        None => {
            eprintln!("error: {}", t_str("error-unknown-effect", &[("name", name)]));
            return ExitCode::from(2);
        }
    };
    let effect = factory();
    let dur_ms = effect.duration().as_millis();
    println!("{}", t_str("help-effect-header", &[("name", name)]));
    println!("  Title: {}", effect.title());
    println!("  Description: {}", effect.description());
    println!("  Duration: {} ms", dur_ms);
    println!("  Audio-reactive: {}", if effect.reactive() { "yes" } else { "no" });
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
    fn cli_parses_all_documented_flags() {
        let _ = Cli::try_parse_from([
            "shedos-screensaver",
            "--effect", "rain",
            "--logo", "block",
            "--color", "#89b4fa",
            "--mode", "tty",
            "--fps", "30",
            "--duration", "5",
            "--hold", "2",
            "--audio-source", "desktop",
            "--wallpaper", "auto",
            "--wallpaper-dim", "0.3",
            "--locale", "en-US",
            "--font-path", "/tmp/x.ttf",
            "--cell-height-px", "20",
            "--cycle", "rain",
            "--cycle", "decrypt",
        ])
        .unwrap();
    }

    #[test]
    fn clap_command_is_well_formed() {
        Cli::command().debug_assert();
    }

    #[test]
    fn resolve_wallpaper_handles_modes() {
        assert_eq!(resolve_wallpaper("none"), None);
        assert_eq!(resolve_wallpaper("/tmp/wp.png"), Some(PathBuf::from("/tmp/wp.png")));
    }

    #[test]
    fn registry_count() {
        assert!(EffectsRegistry::new().len() >= 12);
    }

    #[test]
    fn logos_count() {
        assert!(logos::LIBRARY.len() >= 8);
    }

    #[test]
    fn engine_hold_zero_is_one_shot_mode() {
        // Regression: the user passed `--effect=rain --logo=small
        // --duration=6 --hold=0` and saw the animation re-render
        // twice without the SHEDOS art ever staying complete on
        // screen. With hold=0, after the effect resolves the engine
        // must sit on the resolved frame instead of restarting.
        let mut engine = Engine::new(
            Some("small".to_string()),
            Some("rain".to_string()),
            vec![],
            None,
            Duration::ZERO,
            None,
        );
        let registry = EffectsRegistry::new();
        let mut frame = Frame::new(40, 120);

        // Step the engine forward through the rain effect's full
        // 4.5 s duration plus a margin.
        let dt = Duration::from_millis(50);
        let mut transitions_to_holding = 0;
        let mut iterations_after_first_hold = 0;
        for _ in 0..200 {
            engine.produce(&mut frame, &registry, dt);
            if matches!(engine.state, EngineState::Holding { .. }) {
                if transitions_to_holding == 0 {
                    transitions_to_holding += 1;
                }
                iterations_after_first_hold += 1;
            } else if matches!(engine.state, EngineState::Animating { .. })
                && transitions_to_holding > 0
            {
                panic!(
                    "engine restarted a new animation under --hold=0; \
                     it should sit on the resolved frame in one-shot mode"
                );
            }
        }
        assert!(transitions_to_holding > 0, "rain never completed in 200 ticks");
        assert!(
            iterations_after_first_hold > 50,
            "engine spent only {iterations_after_first_hold} ticks in Holding; should have stayed there"
        );
    }

    #[test]
    fn engine_hold_positive_does_cycle() {
        // Sanity counterpart: with hold=0.5 s the engine MUST
        // restart a new animation after the hold expires. Locks in
        // that the hold=0 special case above doesn't accidentally
        // freeze cycling mode.
        let mut engine = Engine::new(
            Some("small".to_string()),
            Some("rain".to_string()),
            vec![],
            None,
            Duration::from_millis(500),
            None,
        );
        let registry = EffectsRegistry::new();
        let mut frame = Frame::new(40, 120);
        let dt = Duration::from_millis(50);

        let mut entered_hold = false;
        let mut restarted_after_hold = false;
        for _ in 0..400 {
            engine.produce(&mut frame, &registry, dt);
            match &engine.state {
                EngineState::Holding { .. } => entered_hold = true,
                EngineState::Animating { .. } if entered_hold => {
                    restarted_after_hold = true;
                    break;
                }
                _ => {}
            }
        }
        assert!(entered_hold, "rain never reached Holding state");
        assert!(restarted_after_hold, "engine never restarted after hold expired");
    }
}

// Force-keep Logo import used by the Engine via target::build_target.
#[allow(dead_code)]
fn _force_logo_use() -> Logo {
    Logo::embedded()
}
