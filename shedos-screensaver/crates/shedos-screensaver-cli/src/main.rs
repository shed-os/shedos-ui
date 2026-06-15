//! `shedos-screensaver` CLI binary.
//!
//! Architecture: an [`Engine`] cycles through (LogoVariant, Effect)
//! pairs. Each cycle picks a logo and effect (both random unless
//! `--logo=NAME` or `--effect=NAME` is set), renders the logo to a
//! target Frame, runs the effect to completion, then holds the
//! resolved art for `--hold` seconds. The animation is how the
//! SHEDOS art appears.

mod auth;

use clap::{ArgAction, CommandFactory, Parser, ValueEnum};
use clap_complete::Shell;
use crossterm::event::{self, Event};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use shedos_screensaver_audio::{AudioCapture, Source as AudioSrc};
use shedos_screensaver_core::{Clock, Color, Frame, RealClock, SignalListener};
use shedos_screensaver_effects::{target, Effect, EffectCtx, Registry as EffectsRegistry};
use shedos_screensaver_i18n::{t, t_str, I18n};
use shedos_screensaver_logos::{self as logos, LogoVariant};
use shedos_screensaver_tty::{detect_terminal_size, stdout_is_tty, TerminalGuard, TtyRenderer};
use shedos_prompt_ui::{watch as theme_watch, Theme, WidgetCache};
use shedos_screensaver_wayland::calloop_ping;
use shedos_screensaver_wayland::{
    AuthFn, FingerprintConfig, FrameProducer, LockConfig, LockStateConfig, ProducerFactory,
    WaylandConfig, WaylandRenderer,
};
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq, Eq)]
enum Mode {
    Tty,
    Wayland,
    Lock,
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
    version = env!("SHEDOS_VERSION"),
    about = "Animated SHEDOS screensaver with TTY + Wayland backends, 15 logo variants × 46 forming effects",
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
    /// order). Curates a subset, e.g. `--cycle rain --cycle decrypt`.
    #[arg(long = "cycle", value_name = "NAME", action = ArgAction::Append)]
    cycle: Vec<String>,

    /// Walk every (logo, effect) pair, run each to completion on a
    /// fixed 80×24 canvas, and print the final ASCII frame. Pipe to
    /// a file to review the catalog visually.
    #[arg(long)]
    survey: bool,

    /// Lock mode: screensaver dwell before the prompt appears
    /// (override `[lock] prompt_after_secs` in screensaver.toml).
    #[arg(long, value_name = "SECS")]
    prompt_after_secs: Option<u64>,

    /// Lock mode: prompt-idle dwell before it hides
    /// (override `[lock] prompt_idle_hide_secs` in screensaver.toml).
    #[arg(long, value_name = "SECS")]
    prompt_idle_hide_secs: Option<u64>,

    /// Lock mode: prompt-screensaver round-trips before DPMS off
    /// (override `[lock] prompt_cycles` in screensaver.toml).
    #[arg(long, value_name = "N")]
    prompt_cycles: Option<u32>,
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
    if cli.survey {
        return run_survey(&effects);
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

    // Lock mode needs a restricted signal set (see install_for_lock).
    let resolved_mode = resolve_mode(cli.mode);

    // Signal handling for graceful exit.
    let signal_listener = if matches!(resolved_mode, Mode::Lock) {
        SignalListener::install_for_lock()
    } else {
        SignalListener::install()
    }
    .unwrap_or_else(|e| panic!("signal install: {e}"));
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
    let result = match resolved_mode {
        Mode::Wayland => run_wayland(&cli, color_override, audio, Arc::clone(&exit_flag)),
        Mode::Lock => run_lock(&cli, color_override, audio, Arc::clone(&exit_flag)),
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
        // `Auto` must never resolve to `Lock`; explicit only.
        Mode::Tty | Mode::Wayland | Mode::Lock => m,
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
    /// First frame; pick the initial pair.
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
        let fg = self
            .color_override
            .unwrap_or_else(|| logo_variant.pick_color(&mut self.rng));
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
                        // Snap canvas to target so the held image
                        // looks clean regardless of where the effect ended.
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
                    // `--hold 0` is one-shot: animate to completion,
                    // then sit on the resolved art until SIGINT,
                    // SIGUSR1, keypress, or `--duration`. Without
                    // this, hold=0 would restart every frame and the
                    // resolved art would flash for one frame.
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

fn wayland_config(cli: &Cli, wallpaper_path: Option<std::path::PathBuf>) -> WaylandConfig {
    WaylandConfig {
        font_path: cli.font_path.clone(),
        cell_height_px: cli.cell_height_px,
        wallpaper_path,
        wallpaper_dim: cli.wallpaper_dim,
        fps_cap: cli.fps.unwrap_or(60).max(1),
        idle_daemon: cli.idle_daemon,
    }
}

/// One producer per output. Captures are clonable except `audio`,
/// which owns a non-duplicable cpal Stream: the closure `take()`s the
/// Option on first call, so the first output gets audio-reactive
/// effects and later outputs use the silence fallback.
fn engine_producer_factory(
    cli: &Cli,
    color_override: Option<Color>,
    audio: Option<AudioCapture>,
    exit_flag: &Arc<std::sync::atomic::AtomicBool>,
    duration: Option<f32>,
) -> ProducerFactory {
    let logo = cli.logo.clone();
    let effect = cli.effect.clone();
    let cycle = cli.cycle.clone();
    let hold = Duration::from_secs_f32(cli.hold.max(0.0));
    let exit_for_factory = Arc::clone(exit_flag);
    let mut audio_one_shot = audio;
    Box::new(move || {
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
    })
}

fn run_wayland(    cli: &Cli,
    color_override: Option<Color>,
    audio: Option<AudioCapture>,
    exit_flag: Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), String> {
    let cfg = wayland_config(cli, resolve_wallpaper(&cli.wallpaper));
    let factory =
        engine_producer_factory(cli, color_override, audio, &exit_flag, cli.duration);

    if let Some(d) = cli.duration {
        let f = Arc::clone(&exit_flag);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs_f32(d));
            f.store(true, Ordering::Release);
        });
    }

    WaylandRenderer::run(cfg, factory, exit_flag).map_err(|e| format!("wayland: {e}"))
}

fn run_lock(
    cli: &Cli,
    color_override: Option<Color>,
    audio: Option<AudioCapture>,
    exit_flag: Arc<std::sync::atomic::AtomicBool>,
) -> Result<(), String> {
    let lock_config = build_lock_config(cli)?;

    // Lock-mode Screensaver renders on solid Color::BASE so effects
    // read clearly without a wallpaper bleed. The Prompt-phase
    // wallpaper comes from the theme via WidgetCache.
    let cfg = wayland_config(cli, None);

    // --duration must never auto-unlock a lock; ignore it.
    if cli.duration.is_some() {
        eprintln!("shedos-screensaver: --duration is ignored in lock mode");
    }
    let factory = engine_producer_factory(cli, color_override, audio, &exit_flag, None);

    WaylandRenderer::run_locked(cfg, factory, exit_flag, lock_config)
        .map_err(|e| format!("lock: {e}"))
}

/// True when booted from the live ISO. archiso creates /run/archiso on a
/// live boot; an installed disk never has it.
fn live_boot(root: &std::path::Path) -> bool {
    root.join("run/archiso").exists()
}

fn build_lock_config(cli: &Cli) -> Result<LockConfig, String> {
    let username = auth::current_username().map_err(|e| format!("username: {e:#}"))?;
    let theme = Theme::load_or_default();
    let widget_cache =
        WidgetCache::new(&theme).map_err(|e| format!("widget cache: {e:#}"))?;

    let theme_dirty = Arc::new(AtomicBool::new(false));
    let dirty_clone = theme_dirty.clone();
    if let Err(e) = theme_watch::watch(
        Path::new("/etc/shedos/themes"),
        "current",
        move || dirty_clone.store(true, Ordering::Release),
    ) {
        eprintln!("warning: theme watcher disabled: {e:#}");
    }

    let session = auth::PamSession::new("shedos-screensaver", username.clone());
    let authenticate: AuthFn = Box::new(move |password: &str| {
        session.authenticate(password).map_err(|e| {
            eprintln!("shedos-screensaver: pam: {e:?}");
            e.user_message()
        })
    });

    let state_config = build_lock_state_config(cli)?;
    // Live ISO (/run/archiso present): unlock on any key, and skip the
    // fingerprint thread (no fingers are enrolled on a live boot anyway).
    let no_auth = live_boot(std::path::Path::new("/"));
    let fingerprint = if no_auth {
        None
    } else {
        build_fingerprint_config(&username)
    };

    Ok(LockConfig {
        theme,
        widget_cache,
        authenticate,
        theme_dirty,
        state_config,
        username,
        fingerprint,
        no_auth,
    })
}

fn build_fingerprint_config(username: &str) -> Option<FingerprintConfig> {
    let info = auth::fingerprint_available(username)?;
    let (ping, ping_source) = calloop_ping::make_ping().ok()?;
    let paused = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let (rx, _handle) = auth::spawn_fingerprint_auth_loop(
        username.to_owned(),
        ping,
        paused.clone(),
    );
    let hint_text = if info.finger_count == 1 {
        "Touch fingerprint sensor or type password".to_string()
    } else {
        format!(
            "Touch any of your {} enrolled fingers, or type password",
            info.finger_count
        )
    };
    Some(FingerprintConfig {
        rx,
        ping_source,
        hint_text,
        paused,
    })
}

#[derive(Default, serde::Deserialize)]
struct ScreensaverToml {
    lock: Option<LockToml>,
}

#[derive(Default, serde::Deserialize)]
struct LockToml {
    prompt_after_secs: Option<u64>,
    prompt_idle_hide_secs: Option<u64>,
    prompt_cycles: Option<u32>,
}

fn build_lock_state_config(cli: &Cli) -> Result<LockStateConfig, String> {
    const DEFAULT_T2: u64 = 300;
    const DEFAULT_T3: u64 = 120;
    const DEFAULT_N: u32 = 3;
    const CONFIG_PATH: &str = "/etc/shedos/screensaver.toml";

    let toml_section = std::fs::read_to_string(CONFIG_PATH)
        .ok()
        .and_then(|s| match toml::from_str::<ScreensaverToml>(&s) {
            Ok(c) => c.lock,
            Err(e) => {
                eprintln!(
                    "warning: {CONFIG_PATH} parse failed ({e}); using defaults"
                );
                None
            }
        })
        .unwrap_or_default();

    let prompt_after = cli
        .prompt_after_secs
        .or(toml_section.prompt_after_secs)
        .unwrap_or(DEFAULT_T2);
    let prompt_idle_hide = cli
        .prompt_idle_hide_secs
        .or(toml_section.prompt_idle_hide_secs)
        .unwrap_or(DEFAULT_T3);
    let prompt_cycles = cli
        .prompt_cycles
        .or(toml_section.prompt_cycles)
        .unwrap_or(DEFAULT_N);

    if prompt_idle_hide == 0 {
        return Err(
            "prompt_idle_hide_secs must be > 0; the prompt would be unusable at 0".into(),
        );
    }

    Ok(LockStateConfig {
        prompt_after: Duration::from_secs(prompt_after),
        prompt_idle_hide: Duration::from_secs(prompt_idle_hide),
        cycles_before_dpms: prompt_cycles,
    })
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
        let palette = v
            .colors
            .iter()
            .map(|c| c.name)
            .collect::<Vec<_>>()
            .join(", ");
        println!(
            "    {}",
            t_str("list-logo-colors", &[("palette", palette.as_str())])
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

fn run_survey(registry: &EffectsRegistry) -> ExitCode {
    // Fixed canvas so every combination is judged against the same
    // dimensions; eyeball-comparable across the run.
    const ROWS: u16 = 24;
    const COLS: u16 = 80;
    // Deterministic seed so re-running produces the same frames,
    // handy for diffing against an earlier capture.
    const SEED: u64 = 0x5348_4544_4F53_5343;

    let palette_pairs: usize = logos::LIBRARY.iter().map(|v| v.colors.len()).sum();
    let total = palette_pairs * registry.len();
    println!(
        "# shedos-screensaver survey: {} (logo, color) pairs × {} effects = {} combinations",
        palette_pairs,
        registry.len(),
        total,
    );
    println!("# canvas: {COLS}×{ROWS} cells, fixed RNG seed");
    println!("# colors are emitted as 24-bit ANSI escapes — view with `less -R` or `cat`");
    println!();

    for logo_variant in logos::LIBRARY {
        let logo = logo_variant.load();

        for named_color in logo_variant.colors {
            let fg = named_color.color;
            let target = target::build_target(ROWS, COLS, &logo, fg);

            for effect_key in registry.keys() {
                let mut effect = match registry.instantiate(effect_key) {
                    Some(e) => e,
                    None => continue,
                };
                let mut rng = ChaCha8Rng::seed_from_u64(SEED);
                let mut ctx = EffectCtx { final_color: fg, rng: &mut rng };
                effect.setup(&target, &mut ctx);

                let mut frame = Frame::new(ROWS, COLS);
                let dt = Duration::from_millis(16);
                // Run for at most 10 s of animation time; effects with
                // duration() shorter return earlier via step()→true.
                for _ in 0..600 {
                    if effect.step(&mut frame, dt, None) {
                        break;
                    }
                }

                println!("{}", "=".repeat(80));
                println!(
                    "# logo:   {:<14}  ({})",
                    logo_variant.name, logo_variant.title
                );
                println!(
                    "# effect: {:<14}  ({})",
                    effect_key,
                    effect.title()
                );
                println!("# color:  {}", named_color.name);
                println!("{}", "=".repeat(80));
                print_frame_ascii(&frame);
                println!();
            }
        }
    }

    ExitCode::SUCCESS
}

fn print_frame_ascii(frame: &Frame) {
    // Emit each non-blank cell as `\x1b[38;2;R;G;Bm<ch>` so ANSI-
    // aware viewers (less -R, truecolor cat) render the cell color.
    // Blanks stay uncolored; SGR only re-emits when fg changes, so
    // the per-frame overhead stays near one escape per row.
    for r in 0..frame.rows() {
        let mut last_fg: Option<Color> = None;
        for c in 0..frame.cols() {
            if let Some(cell) = frame.get(r, c) {
                if cell.ch == ' ' {
                    print!(" ");
                    continue;
                }
                if last_fg != Some(cell.fg) {
                    print!("\x1b[38;2;{};{};{}m", cell.fg.r, cell.fg.g, cell.fg.b);
                    last_fg = Some(cell.fg);
                }
                print!("{}", cell.ch);
            }
        }
        print!("\x1b[0m");
        println!();
    }
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
        assert!(logos::LIBRARY.len() >= 4);
    }

    #[test]
    fn engine_hold_zero_is_one_shot_mode() {
        // Regression: the user passed `--effect=rain --logo=block
        // --duration=6 --hold=0` and saw the animation re-render
        // twice without the SHEDOS art ever staying complete on
        // screen. With hold=0, after the effect resolves the engine
        // must sit on the resolved frame instead of restarting.
        let mut engine = Engine::new(
            Some("block".to_string()),
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
        // Counterpart to the hold=0 test: with hold=0.5s the engine
        // restarts after the hold expires. Locks in that hold=0's
        // special case doesn't break cycling mode.
        let mut engine = Engine::new(
            Some("block".to_string()),
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

    #[test]
    fn live_boot_keys_on_run_archiso() {
        // /run/archiso is the live-ISO marker the lock keys "no password"
        // on; an installed disk never has it.
        let base = std::env::temp_dir().join(format!("shedos-livetest-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        assert!(!super::live_boot(&base), "absent /run/archiso must read as installed");
        std::fs::create_dir_all(base.join("run/archiso")).unwrap();
        assert!(super::live_boot(&base), "present /run/archiso must read as live");
        let _ = std::fs::remove_dir_all(&base);
    }
}
