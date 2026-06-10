//! Per-output layer-shell overlay surfaces and the Wayland event loop
//! that drives independent frame producers per monitor.
//!
//! Single monitor: one layer surface anchored to all four edges of
//! the only `wl_output`. Multi-monitor: N surfaces, one per output,
//! each with its own frame producer so screens run independent
//! (LogoVariant, Effect) cycles.
//!
//! `producer_factory` mints producers lazily as outputs appear (boot
//! and hotplug). For once-per-process resources (cpal audio stream)
//! the closure `take()`s an Option on first call; later outputs get
//! `None` and audio-reactive effects fall back to silence.

use crate::font::FontAtlas;
use crate::lock::LockBinding;
use crate::wallpaper::Wallpaper;
use crate::dpms;
use crate::{
    blend_over, pack_argb, AuthFn, FrameProducer, LockConfig, WaylandConfig,
};
use std::sync::mpsc::Receiver;
use shedos_prompt_ui::{
    power::{self as power_ui, PowerAction, PowerHit, PowerMenuState},
    OutputRect, PromptState, RenderParams, Theme, WidgetCache,
};
use shedos_screensaver_core::{Color, Frame, LockPhase, LockState};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState},
    reexports::{
        calloop::EventLoop,
        calloop_wayland_source::WaylandSource,
    },
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers},
        pointer::{cursor_shape::CursorShapeManager, PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const ERROR_HOLD: Duration = Duration::from_secs(2);

/// Post-attempt dwell for the fingerprint icon before fading to Idle
/// (Failure) or releasing the lock (Success). 300ms was empirically
/// too brief: wayland double-buffering and compositor scheduling could
/// swallow the frame before the user noticed it.
const FP_FAILURE_HOLD: Duration = Duration::from_millis(1000);
const FP_SUCCESS_HOLD: Duration = Duration::from_millis(900);

/// Catppuccin Mocha green. Hardcoded because the theme schema has no
/// `green` field today; swap to `theme.green` if one is added.
const FP_SUCCESS_GREEN_ARGB: u32 = 0xFFA6E3A1;

#[derive(Clone, Copy, Default)]
enum FingerprintStatus {
    #[default]
    Idle,
    Failure(Instant),
    Success(Instant),
}
use wayland_client::{
    globals::{registry_queue_init, GlobalList},
    protocol::{wl_keyboard::WlKeyboard, wl_output::WlOutput, wl_pointer::WlPointer, wl_seat::WlSeat, wl_shm, wl_surface::WlSurface},
    Connection, Proxy, QueueHandle,
};
use wayland_protocols::ext::session_lock::v1::client::{
    ext_session_lock_manager_v1::ExtSessionLockManagerV1,
    ext_session_lock_surface_v1::ExtSessionLockSurfaceV1,
};
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::{
    Shape as CursorShape, WpCursorShapeDeviceV1,
};
use wayland_protocols_wlr::output_power_management::v1::client::{
    zwlr_output_power_manager_v1::ZwlrOutputPowerManagerV1,
    zwlr_output_power_v1::{Mode as DpmsMode, ZwlrOutputPowerV1},
};

/// Mints one frame producer per output. Called at boot and on
/// hotplug. Use `take()`-able captures for single-instance resources.
pub type ProducerFactory = Box<dyn FnMut() -> Box<dyn FrameProducer>>;

/// Renderer entry point. One layer surface + frame producer per
/// wl_output the compositor advertises.
pub struct WaylandRenderer;

impl WaylandRenderer {
    pub fn run(
        config: WaylandConfig,
        producer_factory: ProducerFactory,
        should_exit: Arc<AtomicBool>,
    ) -> Result<(), WaylandError> {
        let conn = Connection::connect_to_env()
            .map_err(|e| WaylandError::Connect(format!("{e}")))?;
        let (globals, event_queue) = registry_queue_init(&conn)
            .map_err(|e| WaylandError::Connect(format!("registry init: {e}")))?;
        let qh: QueueHandle<AppState> = event_queue.handle();

        let registry_state = RegistryState::new(&globals);
        let output_state = OutputState::new(&globals, &qh);
        let seat_state = SeatState::new(&globals, &qh);
        let compositor_state = CompositorState::bind(&globals, &qh)
            .map_err(|e| WaylandError::Bind(format!("compositor: {e}")))?;
        let layer_shell = LayerShell::bind(&globals, &qh)
            .map_err(|e| WaylandError::Bind(format!("wlr-layer-shell-unstable-v1: {e}")))?;
        let shm = Shm::bind(&globals, &qh)
            .map_err(|e| WaylandError::Bind(format!("wl_shm: {e}")))?;
        let cursor_shape = CursorShapeManager::bind(&globals, &qh).ok();

        let font = FontAtlas::load(config.font_path.as_deref(), config.cell_height_px as f32)
            .map_err(|e| WaylandError::Font(format!("{e}")))?;

        let mut state = AppState {
            registry_state,
            output_state,
            seat_state,
            shm,
            compositor_state,
            layer_shell,
            qh: qh.clone(),
            cursor_device: None,
            cursor_shape,
            pointer: None,
            keyboard: None,
            should_exit: Arc::clone(&should_exit),
            input_dismissed: false,
            idle_daemon: config.idle_daemon,
            producer_factory,
            font,
            wallpaper_path: config.wallpaper_path,
            wallpaper_dim: config.wallpaper_dim,
            surfaces: Vec::new(),
            is_lock_mode: false,
            lock_binding: None,
            lock_state: None,
            dpms_manager: None,
            theme: None,
            widget_cache: None,
            theme_dirty: None,
            authenticate: None,
            username: None,
            prompt_password: String::new(),
            prompt_capslock: false,
            power_menu: PowerMenuState::default(),
            error: None,
            fingerprint_rx: None,
            fingerprint_hint: None,
            fingerprint_paused: None,
            fingerprint_status: FingerprintStatus::Idle,
        };
        let _ = config.fps_cap;

        let mut event_loop: EventLoop<AppState> = EventLoop::try_new()
            .map_err(|e| WaylandError::Connect(format!("calloop event loop: {e}")))?;
        let loop_handle = event_loop.handle();

        let mut wayland_source = WaylandSource::new(conn, event_queue);
        wayland_source
            .queue()
            .roundtrip(&mut state)
            .map_err(|e| WaylandError::Dispatch(format!("initial roundtrip: {e}")))?;
        wayland_source
            .insert(loop_handle)
            .map_err(|e| WaylandError::Connect(format!("calloop insert: {e:?}")))?;

        run_loop(&mut state, &mut event_loop)
    }

    pub fn run_locked(
        config: WaylandConfig,
        producer_factory: ProducerFactory,
        should_exit: Arc<AtomicBool>,
        lock_config: LockConfig,
    ) -> Result<(), WaylandError> {
        let conn = Connection::connect_to_env()
            .map_err(|e| WaylandError::Connect(format!("{e}")))?;
        let (globals, event_queue) = registry_queue_init(&conn)
            .map_err(|e| WaylandError::Connect(format!("registry init: {e}")))?;
        let qh: QueueHandle<AppState> = event_queue.handle();

        let registry_state = RegistryState::new(&globals);
        let output_state = OutputState::new(&globals, &qh);
        let seat_state = SeatState::new(&globals, &qh);
        let compositor_state = CompositorState::bind(&globals, &qh)
            .map_err(|e| WaylandError::Bind(format!("compositor: {e}")))?;
        let layer_shell = LayerShell::bind(&globals, &qh)
            .map_err(|e| WaylandError::Bind(format!("wlr-layer-shell-unstable-v1: {e}")))?;
        let shm = Shm::bind(&globals, &qh)
            .map_err(|e| WaylandError::Bind(format!("wl_shm: {e}")))?;
        let cursor_shape = CursorShapeManager::bind(&globals, &qh).ok();

        let font = FontAtlas::load(config.font_path.as_deref(), config.cell_height_px as f32)
            .map_err(|e| WaylandError::Font(format!("{e}")))?;

        let LockConfig {
            theme,
            widget_cache,
            authenticate,
            theme_dirty,
            state_config,
            username,
            fingerprint,
        } = lock_config;
        let (fingerprint_rx, fingerprint_ping_source, fingerprint_hint, fingerprint_paused) =
            match fingerprint {
                Some(fp) => (
                    Some(fp.rx),
                    Some(fp.ping_source),
                    Some(fp.hint_text),
                    Some(fp.paused),
                ),
                None => (None, None, None, None),
            };

        let now = Instant::now();
        let lock_state = LockState::new(state_config, now);

        let mut state = AppState {
            registry_state,
            output_state,
            seat_state,
            shm,
            compositor_state,
            layer_shell,
            qh: qh.clone(),
            cursor_device: None,
            cursor_shape,
            pointer: None,
            keyboard: None,
            should_exit: Arc::clone(&should_exit),
            input_dismissed: false,
            idle_daemon: config.idle_daemon,
            producer_factory,
            font,
            wallpaper_path: config.wallpaper_path,
            wallpaper_dim: config.wallpaper_dim,
            surfaces: Vec::new(),
            is_lock_mode: true,
            lock_binding: None,
            lock_state: Some(lock_state),
            dpms_manager: None,
            theme: Some(theme),
            widget_cache: Some(widget_cache),
            theme_dirty: Some(theme_dirty),
            authenticate: Some(authenticate),
            username: Some(username),
            prompt_password: String::new(),
            prompt_capslock: false,
            power_menu: PowerMenuState::default(),
            error: None,
            fingerprint_rx,
            fingerprint_hint,
            fingerprint_paused,
            fingerprint_status: FingerprintStatus::Idle,
        };
        let _ = config.fps_cap;

        let mut event_loop: EventLoop<AppState> = EventLoop::try_new()
            .map_err(|e| WaylandError::Connect(format!("calloop event loop: {e}")))?;
        let loop_handle = event_loop.handle();

        // Fingerprint-thread ping source: wakes the wayland loop on
        // each scan completion. The callback is a no-op; result
        // handling reads from the channel in the render loop.
        if let Some(source) = fingerprint_ping_source {
            loop_handle
                .insert_source(source, |_, _, _state: &mut AppState| {})
                .map_err(|e| {
                    WaylandError::Connect(format!("calloop fingerprint ping insert: {e:?}"))
                })?;
        }

        let mut wayland_source = WaylandSource::new(conn.clone(), event_queue);

        // Sync wayland ops via wayland_source.queue() must happen
        // before insert; after insert, the loop owns the queue.
        wayland_source
            .queue()
            .roundtrip(&mut state)
            .map_err(|e| WaylandError::Dispatch(format!("initial roundtrip: {e}")))?;

        state.dpms_manager = dpms::bind_manager(&globals, &qh);
        if state.dpms_manager.is_none() {
            eprintln!(
                "shedos-screensaver: zwlr_output_power_manager_v1 not advertised; \
                 monitors will not power off"
            );
        }

        if let (Some(paused), Some(ls)) =
            (state.fingerprint_paused.as_ref(), state.lock_state.as_ref())
        {
            paused.store(ls.phase() != LockPhase::Prompt, Ordering::Release);
        }

        let manager: ExtSessionLockManagerV1 = bind_session_lock_manager(&globals, &qh)?;
        let lock = manager.lock(&qh, ());
        state.lock_binding = Some(LockBinding::new(lock));

        // Hyprland sends `Locked` only after we commit our first
        // lock-surface buffers, so this roundtrip catches an immediate
        // `Finished` from a policy denial. Otherwise mint surfaces and
        // let the render loop drive configure + commit.
        wayland_source
            .queue()
            .roundtrip(&mut state)
            .map_err(|e| WaylandError::Dispatch(format!("post-lock roundtrip: {e}")))?;
        if state.lock_binding.as_ref().is_some_and(|lb| lb.finished) {
            return Err(WaylandError::Bind(
                "compositor refused ext-session-lock-v1. Recovery: switch to a tty and \
                 run `loginctl unlock-session`."
                    .into(),
            ));
        }
        touch_lock_sentinel();

        let outputs: Vec<WlOutput> = state.output_state.outputs().collect();
        for o in outputs {
            state.add_output(o);
        }

        wayland_source
            .insert(loop_handle)
            .map_err(|e| WaylandError::Connect(format!("calloop insert: {e:?}")))?;

        let result = run_loop(&mut state, &mut event_loop);

        // Clear the sentinel only on a real unlock; close() is fail-closed.
        let authenticated = state
            .lock_binding
            .as_ref()
            .is_some_and(|lb| lb.authenticated);
        if authenticated {
            clear_lock_sentinel();
        }

        if let Some(lb) = state.lock_binding.as_mut() {
            lb.close();
        }
        let _ = conn.flush();

        result
    }
}

// Sentinel checked by relock-on-restart after a compositor crash:
// presence means the session was locked when Hyprland died, so
// re-engage the lock now that it's back.
fn lock_sentinel_path() -> Option<PathBuf> {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(|d| PathBuf::from(d).join("shedos-locked"))
}

fn touch_lock_sentinel() {
    if let Some(p) = lock_sentinel_path() {
        let _ = fs::write(&p, b"");
    }
}

fn clear_lock_sentinel() {
    if let Some(p) = lock_sentinel_path() {
        let _ = fs::remove_file(&p);
    }
}

fn bind_session_lock_manager(
    globals: &GlobalList,
    qh: &QueueHandle<AppState>,
) -> Result<ExtSessionLockManagerV1, WaylandError> {
    globals
        .bind(qh, 1..=1, ())
        .map_err(|e| WaylandError::Bind(format!("ext_session_lock_manager_v1: {e}")))
}

fn run_loop(
    state: &mut AppState,
    event_loop: &mut EventLoop<AppState>,
) -> Result<(), WaylandError> {
    // Render-then-dispatch ordering is load-bearing: dispatching first
    // would block because the initial configure was already consumed
    // by the pre-insert roundtrip. Render first, then block on dispatch.
    while !state.terminating() {
        // Drain pending fingerprint results. Success flashes green
        // (FP_SUCCESS_HOLD) then unlocks; failure flashes red
        // (FP_FAILURE_HOLD) then returns to Idle. Fingerprint failures
        // don't surface on the error line; that slot is for password
        // errors, and the icon color is the fingerprint feedback.
        let now = Instant::now();
        let fp_results: Vec<Result<(), ()>> = state
            .fingerprint_rx
            .as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default();
        let in_prompt = state
            .lock_state
            .as_ref()
            .map(|ls| ls.phase() == LockPhase::Prompt)
            .unwrap_or(false);
        for result in fp_results {
            match result {
                Ok(()) => {
                    state.fingerprint_status =
                        FingerprintStatus::Success(now + FP_SUCCESS_HOLD);
                    state.mark_all_dirty();
                }
                Err(()) if in_prompt => {
                    state.fingerprint_status =
                        FingerprintStatus::Failure(now + FP_FAILURE_HOLD);
                    state.mark_all_dirty();
                }
                Err(()) => {}
            }
        }
        // Post-attempt dwell: expire Failure back to Idle (and redraw);
        // release the lock when Success dwell expires (so the green
        // flash gets at least one render).
        match state.fingerprint_status {
            FingerprintStatus::Failure(until) if Instant::now() >= until => {
                state.fingerprint_status = FingerprintStatus::Idle;
                state.mark_all_dirty();
            }
            FingerprintStatus::Success(until) if Instant::now() >= until => {
                state.mark_authenticated();
            }
            _ => {}
        }

        // In lock mode, advance the state machine and react to any
        // phase change before rendering; the new phase decides what
        // gets drawn.
        let transition = state.lock_state.as_mut().and_then(|ls| {
            let prev = ls.phase();
            ls.tick(Instant::now());
            let curr = ls.phase();
            (prev != curr).then_some((prev, curr))
        });
        if let Some((from, to)) = transition {
            state.on_phase_change(from, to);
        }

        for i in 0..state.surfaces.len() {
            if state.surfaces[i].needs_redraw && state.surfaces[i].configured {
                state.render_surface(i)?;
                state.surfaces[i].needs_redraw = false;
                state.surfaces[i].last_frame = Some(Instant::now());
            }
        }
        if state.terminating() {
            break;
        }

        // Sleep until the next state-machine deadline or fingerprint
        // dwell expiry, whichever first, or until a Wayland event
        // arrives. In layer-shell mode lock_state is None.
        let now = Instant::now();
        let lock_timeout = state
            .lock_state
            .as_ref()
            .and_then(|ls| ls.time_until_next_transition(now));
        let fp_timeout = match state.fingerprint_status {
            FingerprintStatus::Idle => None,
            FingerprintStatus::Failure(until) | FingerprintStatus::Success(until) => {
                Some(until.saturating_duration_since(now))
            }
        };
        let timeout = match (lock_timeout, fp_timeout) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (None, None) => None,
        };

        event_loop
            .dispatch(timeout, state)
            .map_err(|e| WaylandError::Dispatch(format!("calloop dispatch: {e}")))?;
    }

    Ok(())
}

/// Wayland shell role bound to an [`OutputSurface`].
pub(crate) enum ShellBinding {
    Layer(LayerSurface),
    Lock {
        wl_surface: WlSurface,
        lock_surface: ExtSessionLockSurfaceV1,
    },
}

impl ShellBinding {
    fn wl_surface(&self) -> &WlSurface {
        match self {
            Self::Layer(l) => l.wl_surface(),
            Self::Lock { wl_surface, .. } => wl_surface,
        }
    }

    fn commit(&self) {
        match self {
            Self::Layer(l) => l.commit(),
            Self::Lock { wl_surface, .. } => wl_surface.commit(),
        }
    }
}

struct OutputSurface {
    output: WlOutput,
    shell: ShellBinding,
    pool: SlotPool,
    width: u32,
    height: u32,
    configured: bool,
    wallpaper_cache: Option<Wallpaper>,
    last_frame: Option<Instant>,
    frame: Frame,
    producer: Box<dyn FrameProducer>,
    needs_redraw: bool,
    dpms_power: Option<ZwlrOutputPowerV1>,
}

pub(crate) struct AppState {
    registry_state: RegistryState,
    output_state: OutputState,
    seat_state: SeatState,
    shm: Shm,
    compositor_state: CompositorState,
    layer_shell: LayerShell,
    qh: QueueHandle<Self>,
    /// Must drop before `pointer`; keep declared first.
    cursor_device: Option<WpCursorShapeDeviceV1>,
    cursor_shape: Option<CursorShapeManager>,
    pointer: Option<WlPointer>,
    keyboard: Option<WlKeyboard>,
    should_exit: Arc<AtomicBool>,
    input_dismissed: bool,
    idle_daemon: bool,
    producer_factory: ProducerFactory,
    font: FontAtlas,
    wallpaper_path: Option<PathBuf>,
    wallpaper_dim: f32,
    surfaces: Vec<OutputSurface>,
    is_lock_mode: bool,
    pub(crate) lock_binding: Option<LockBinding>,
    lock_state: Option<LockState>,
    dpms_manager: Option<ZwlrOutputPowerManagerV1>,
    theme: Option<Theme>,
    widget_cache: Option<WidgetCache>,
    theme_dirty: Option<Arc<AtomicBool>>,
    authenticate: Option<AuthFn>,
    username: Option<String>,
    prompt_password: String,
    prompt_capslock: bool,
    power_menu: PowerMenuState,
    error: Option<(Instant, String)>,
    fingerprint_rx: Option<Receiver<Result<(), ()>>>,
    fingerprint_hint: Option<String>,
    fingerprint_paused: Option<Arc<AtomicBool>>,
    fingerprint_status: FingerprintStatus,
}

impl AppState {
    fn should_exit(&self) -> bool {
        self.should_exit.load(Ordering::Acquire)
    }

    fn terminating(&self) -> bool {
        self.should_exit()
            || self.input_dismissed
            || self.lock_binding.as_ref().is_some_and(|lb| lb.finished)
    }

    /// The only path that may release the lock: mark auth, then exit.
    fn mark_authenticated(&mut self) {
        if let Some(lb) = self.lock_binding.as_mut() {
            lb.authenticated = true;
        }
        self.should_exit.store(true, Ordering::Release);
    }

    fn handle_input(&mut self) {
        if self.idle_daemon {
            return;
        }
        self.input_dismissed = true;
    }

    fn mark_all_dirty(&mut self) {
        for s in self.surfaces.iter_mut() {
            s.needs_redraw = true;
        }
    }

    fn submit_password(&mut self) {
        let Some(auth) = self.authenticate.as_ref() else {
            return;
        };
        let password = std::mem::take(&mut self.prompt_password);
        if password.is_empty() {
            return;
        }
        match auth(&password) {
            Ok(()) => self.mark_authenticated(),
            Err(msg) => self.error = Some((Instant::now() + ERROR_HOLD, msg)),
        }
        self.mark_all_dirty();
    }

    fn on_phase_change(&mut self, from: LockPhase, to: LockPhase) {
        if from == LockPhase::Dpms {
            for s in &self.surfaces {
                if let Some(p) = &s.dpms_power {
                    p.set_mode(DpmsMode::On);
                }
            }
        }
        if let Some(paused) = self.fingerprint_paused.as_ref() {
            paused.store(to != LockPhase::Prompt, Ordering::Release);
        }
        match to {
            LockPhase::Screensaver => {
                // Mint a fresh producer per surface so animations
                // don't fast-forward through the prompt-phase gap.
                for s in self.surfaces.iter_mut() {
                    s.producer = (self.producer_factory)();
                }
            }
            LockPhase::Prompt => {
                self.prompt_password.clear();
                self.fingerprint_status = FingerprintStatus::Idle;
                if let Some(rx) = self.fingerprint_rx.as_ref() {
                    while rx.try_recv().is_ok() {}
                }
            }
            LockPhase::Dpms => {
                for s in &self.surfaces {
                    if let Some(p) = &s.dpms_power {
                        p.set_mode(DpmsMode::Off);
                    }
                }
            }
        }
        self.mark_all_dirty();
    }

    fn add_output(&mut self, output: WlOutput) {
        if self.surfaces.iter().any(|s| s.output == output) {
            return;
        }
        if self.is_lock_mode {
            self.add_lock_output(output);
        } else {
            self.add_layer_output(output);
        }
    }

    fn add_layer_output(&mut self, output: WlOutput) {
        let surface = self.compositor_state.create_surface(&self.qh);
        let layer = self.layer_shell.create_layer_surface(
            &self.qh,
            surface,
            Layer::Overlay,
            Some("shedos-screensaver"),
            Some(&output),
        );
        layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(if self.idle_daemon {
            KeyboardInteractivity::OnDemand
        } else {
            KeyboardInteractivity::Exclusive
        });
        layer.commit();

        let Some(pool) = self.new_pool() else {
            return;
        };

        self.surfaces.push(OutputSurface {
            output,
            shell: ShellBinding::Layer(layer),
            pool,
            width: 0,
            height: 0,
            configured: false,
            wallpaper_cache: None,
            last_frame: None,
            frame: Frame::new(0, 0),
            producer: (self.producer_factory)(),
            needs_redraw: true,
            dpms_power: None,
        });
    }

    fn add_lock_output(&mut self, output: WlOutput) {
        let Some(lb) = self.lock_binding.as_ref() else {
            return;
        };
        let wl_surface = self.compositor_state.create_surface(&self.qh);
        let lock_surface = lb.lock.get_lock_surface(&wl_surface, &output, &self.qh, ());

        let Some(pool) = self.new_pool() else {
            return;
        };

        let dpms_power = self
            .dpms_manager
            .as_ref()
            .map(|m| m.get_output_power(&output, &self.qh, ()));

        self.surfaces.push(OutputSurface {
            output,
            shell: ShellBinding::Lock {
                wl_surface,
                lock_surface,
            },
            pool,
            width: 0,
            height: 0,
            configured: false,
            wallpaper_cache: None,
            last_frame: None,
            frame: Frame::new(0, 0),
            producer: (self.producer_factory)(),
            needs_redraw: true,
            dpms_power,
        });
    }

    fn new_pool(&self) -> Option<SlotPool> {
        match SlotPool::new(4, &self.shm) {
            Ok(p) => Some(p),
            Err(e) => {
                eprintln!("shedos-screensaver-wayland: pool for new output failed: {e}");
                None
            }
        }
    }

    pub(crate) fn apply_lock_surface_configure(
        &mut self,
        target: &ExtSessionLockSurfaceV1,
        width: u32,
        height: u32,
    ) {
        let Some(idx) = self.surfaces.iter().position(|s| match &s.shell {
            ShellBinding::Lock { lock_surface, .. } => lock_surface.id() == target.id(),
            _ => false,
        }) else {
            return;
        };
        let output = self.surfaces[idx].output.clone();
        let (mut w, mut h) = (width, height);
        if w == 0 || h == 0 {
            if let Some(info) = self.output_state.info(&output) {
                let fallback = info
                    .logical_size
                    .map(|(lw, lh)| (lw.max(0) as u32, lh.max(0) as u32))
                    .or_else(|| {
                        info.modes
                            .iter()
                            .find(|m| m.current)
                            .map(|m| (m.dimensions.0.max(0) as u32, m.dimensions.1.max(0) as u32))
                    });
                if let Some((fw, fh)) = fallback {
                    if w == 0 {
                        w = fw;
                    }
                    if h == 0 {
                        h = fh;
                    }
                }
            }
        }
        let s = &mut self.surfaces[idx];
        if w > 0 && h > 0 {
            s.width = w;
            s.height = h;
            let needed = (w * h * 4) as usize;
            let _ = s.pool.resize(needed.max(4));
            s.configured = true;
            s.wallpaper_cache = None;
            s.needs_redraw = true;
        }
    }

    fn drop_output(&mut self, output: &WlOutput) {
        self.surfaces.retain(|s| &s.output != output);
    }

    fn surface_index_by_layer(&self, target: &LayerSurface) -> Option<usize> {
        self.surfaces
            .iter()
            .position(|s| s.shell.wl_surface() == target.wl_surface())
    }

    fn surface_index_by_wl_surface(&self, target: &WlSurface) -> Option<usize> {
        self.surfaces
            .iter()
            .position(|s| s.shell.wl_surface() == target)
    }

    fn render_surface(&mut self, idx: usize) -> Result<(), WaylandError> {
        if idx >= self.surfaces.len() {
            return Ok(());
        }

        let phase = self.lock_state.as_ref().map(|s| s.phase());
        if matches!(phase, Some(LockPhase::Dpms)) {
            return Ok(());
        }

        // Theme reload + stale-error decay (lock mode only).
        if self.is_lock_mode {
            if let Some(dirty) = &self.theme_dirty {
                if dirty.swap(false, Ordering::AcqRel) {
                    if let Some(theme) = self.theme.as_mut() {
                        *theme = Theme::load_or_default();
                        if let Some(cache) = self.widget_cache.as_mut() {
                            let _ = cache.refresh_wallpaper(theme);
                        }
                    }
                }
            }
            if let Some((t, _)) = self.error.as_ref() {
                if Instant::now() >= *t {
                    self.error = None;
                }
            }
        }

        // Lock-mode Prompt phase: prompt UI only. Otherwise (layer-
        // shell or lock-mode Screensaver) render the full screensaver.
        if self.is_lock_mode && matches!(phase, Some(LockPhase::Prompt)) {
            self.render_lock_prompt(idx)
        } else {
            self.render_screensaver_content(idx)
        }
    }

    fn render_lock_prompt(&mut self, idx: usize) -> Result<(), WaylandError> {
        let s = &mut self.surfaces[idx];
        if s.width == 0 || s.height == 0 {
            return Ok(());
        }
        let (Some(theme), Some(cache)) =
            (self.theme.as_ref(), self.widget_cache.as_mut())
        else {
            return Ok(());
        };

        let stride = (s.width as i32) * 4;
        let (buffer, canvas) = s
            .pool
            .create_buffer(
                s.width as i32,
                s.height as i32,
                stride,
                wl_shm::Format::Argb8888,
            )
            .map_err(|e| WaylandError::Pool(format!("create_buffer: {e}")))?;

        let active_error = self
            .error
            .as_ref()
            .filter(|(t, _)| Instant::now() < *t);
        let prompt_state = PromptState {
            typed_chars: self.prompt_password.chars().count(),
            fail: active_error.is_some(),
            success: false,
            capslock: self.prompt_capslock,
            power_menu: self.power_menu.clone(),
        };
        let greeting = self
            .username
            .as_ref()
            .map(|n| format!("Hi, {n}"))
            .unwrap_or_else(|| "Hi".to_string());
        let fingerprint = self.fingerprint_hint.as_deref().map(|idle_hint| {
            // Color + hint vary by post-attempt state so the user
            // sees feedback per scan.
            let (icon_color_argb, hint) = match self.fingerprint_status {
                FingerprintStatus::Idle => (theme.accent, idle_hint),
                FingerprintStatus::Failure(_) => {
                    (theme.red, "Fingerprint not recognized — try again")
                }
                FingerprintStatus::Success(_) => {
                    (FP_SUCCESS_GREEN_ARGB, "Fingerprint recognized")
                }
            };
            shedos_prompt_ui::FingerprintRender { hint, icon_color_argb }
        });
        let params = RenderParams {
            greeting: Some(greeting.as_str()),
            error_message: active_error.map(|(_, m)| m.as_str()),
            fingerprint,
        };
        let rect = OutputRect {
            x: 0,
            y: 0,
            w: s.width as i32,
            h: s.height as i32,
        };
        shedos_prompt_ui::render(
            canvas,
            s.width,
            s.height,
            &[rect],
            &prompt_state,
            theme,
            cache,
            &params,
        );

        let surface = s.shell.wl_surface().clone();
        surface.damage_buffer(0, 0, s.width as i32, s.height as i32);
        buffer
            .attach_to(&surface)
            .map_err(|e| WaylandError::Buffer(format!("attach: {e}")))?;
        s.shell.commit();
        Ok(())
    }

    fn render_screensaver_content(&mut self, idx: usize) -> Result<(), WaylandError> {
        let s = &mut self.surfaces[idx];
        if s.width == 0 || s.height == 0 {
            return Ok(());
        }

        if s.wallpaper_cache.is_none() {
            if let Some(path) = &self.wallpaper_path {
                match Wallpaper::prepare(path, s.width, s.height, self.wallpaper_dim) {
                    Ok(w) => s.wallpaper_cache = Some(w),
                    Err(e) => eprintln!(
                        "shedos-screensaver-wayland: wallpaper '{}' failed: {e}; \
                         drawing on solid base",
                        path.display()
                    ),
                }
            }
        }

        let (cell_w, cell_h) = self.font.cell_size();
        let cols = (s.width / cell_w).max(1) as u16;
        let rows = (s.height / cell_h).max(1) as u16;
        if (s.frame.cols(), s.frame.rows()) != (cols, rows) {
            s.frame = Frame::new(rows, cols);
        }
        s.frame.clear();
        s.producer.produce(&mut s.frame);

        let stride = (s.width as i32) * 4;
        let total_bytes = (s.height as i32) * stride;
        let (buffer, canvas) = s
            .pool
            .create_buffer(
                s.width as i32,
                s.height as i32,
                stride,
                wl_shm::Format::Argb8888,
            )
            .map_err(|e| WaylandError::Pool(format!("create_buffer: {e}")))?;
        debug_assert_eq!(canvas.len(), total_bytes as usize);

        let pixels: &mut [u32] = unsafe {
            // Safe: SlotPool gives 4-byte alignment for Argb8888 and
            // canvas.len() is a multiple of 4.
            let ptr = canvas.as_mut_ptr() as *mut u32;
            std::slice::from_raw_parts_mut(ptr, canvas.len() / 4)
        };
        if let Some(wp) = &s.wallpaper_cache {
            pixels.copy_from_slice(&wp.pixels);
        } else {
            let base = pack_argb(Color::BASE);
            for p in pixels.iter_mut() {
                *p = base;
            }
        }

        let baseline = self.font.baseline();
        for r in 0..s.frame.rows() {
            for c in 0..s.frame.cols() {
                let cell = s.frame.get(r, c).expect("in-bounds row/col");
                if cell.ch == ' ' {
                    continue;
                }
                let glyph = self.font.glyph(cell.ch);
                let cell_x0 = (c as i32) * (cell_w as i32);
                let cell_y0 = (r as i32) * (cell_h as i32);
                let glyph_x0 = cell_x0 + glyph.x_offset;
                let glyph_y0 = cell_y0 + baseline + glyph.y_offset;
                for gy in 0..glyph.height as i32 {
                    let dst_y = glyph_y0 + gy;
                    if dst_y < 0 || dst_y >= s.height as i32 {
                        continue;
                    }
                    for gx in 0..glyph.width as i32 {
                        let dst_x = glyph_x0 + gx;
                        if dst_x < 0 || dst_x >= s.width as i32 {
                            continue;
                        }
                        let alpha = glyph.bitmap[(gy * glyph.width as i32 + gx) as usize];
                        if alpha == 0 {
                            continue;
                        }
                        let i = (dst_y as usize) * (s.width as usize) + dst_x as usize;
                        pixels[i] = blend_over(cell.fg, pixels[i], alpha);
                    }
                }
            }
        }

        let surface = s.shell.wl_surface().clone();
        surface.frame(&self.qh, surface.clone());
        surface.damage_buffer(0, 0, s.width as i32, s.height as i32);
        buffer
            .attach_to(&surface)
            .map_err(|e| WaylandError::Buffer(format!("attach: {e}")))?;
        s.shell.commit();
        Ok(())
    }
}

#[derive(Debug)]
pub enum WaylandError {
    Connect(String),
    Bind(String),
    Pool(String),
    Buffer(String),
    Dispatch(String),
    Font(String),
}

impl std::fmt::Display for WaylandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connect(s) => write!(f, "wayland connect: {s}"),
            Self::Bind(s) => write!(f, "wayland bind: {s}"),
            Self::Pool(s) => write!(f, "wl_shm pool: {s}"),
            Self::Buffer(s) => write!(f, "wl_buffer: {s}"),
            Self::Dispatch(s) => write!(f, "wayland dispatch: {s}"),
            Self::Font(s) => write!(f, "font load: {s}"),
        }
    }
}

impl std::error::Error for WaylandError {}

// ----- Handler impls -----

impl CompositorHandler for AppState {
    fn scale_factor_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &WlSurface,
        _new_factor: i32,
    ) {}

    fn transform_changed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &WlSurface,
        _new_transform: wayland_client::protocol::wl_output::Transform,
    ) {}

    fn frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        surface: &WlSurface,
        _time: u32,
    ) {
        if let Some(idx) = self.surface_index_by_wl_surface(surface) {
            self.surfaces[idx].needs_redraw = true;
        }
    }

    fn surface_enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &WlSurface,
        _output: &WlOutput,
    ) {}

    fn surface_leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _surface: &WlSurface,
        _output: &WlOutput,
    ) {}
}

impl OutputHandler for AppState {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, output: WlOutput) {
        if self.is_lock_mode && self.lock_binding.is_none() {
            return;
        }
        self.add_output(output);
    }
    fn update_output(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _output: WlOutput) {}
    fn output_destroyed(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        output: WlOutput,
    ) {
        self.drop_output(&output);
    }
}

impl SeatHandler for AppState {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
    fn new_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: WlSeat) {}
    fn new_capability(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        seat: WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            if let Ok(kb) = self.seat_state.get_keyboard(qh, &seat, None) {
                self.keyboard = Some(kb);
            }
        }
        if capability == Capability::Pointer && self.pointer.is_none() {
            if let Ok(p) = self.seat_state.get_pointer(qh, &seat) {
                self.pointer = Some(p);
            }
        }
    }
    fn remove_capability(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _seat: WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard {
            if let Some(kb) = self.keyboard.take() {
                kb.release();
            }
        }
        if capability == Capability::Pointer {
            self.cursor_device = None;
            if let Some(p) = self.pointer.take() {
                p.release();
            }
        }
    }
    fn remove_seat(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _seat: WlSeat) {}
}

impl KeyboardHandler for AppState {
    fn enter(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _surface: &WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[Keysym],
    ) {}
    fn leave(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _surface: &WlSurface,
        _serial: u32,
    ) {}
    fn press_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        if !self.is_lock_mode {
            self.handle_input();
            return;
        }

        // Feed input to the state machine first; a keypress in
        // Screensaver or Dpms transitions to Prompt before we
        // dispatch the key.
        let transition = self.lock_state.as_mut().and_then(|ls| {
            let prev = ls.phase();
            ls.on_input(Instant::now());
            let curr = ls.phase();
            (prev != curr).then_some((prev, curr))
        });
        if let Some((from, to)) = transition {
            self.on_phase_change(from, to);
        }

        if self.power_menu.open {
            match event.keysym {
                Keysym::Escape | Keysym::F12 => {
                    self.power_menu.open = false;
                    self.power_menu.kb_active = false;
                    self.mark_all_dirty();
                    return;
                }
                Keysym::Up => {
                    self.power_menu.kb_active = true;
                    self.power_menu.select_prev();
                    self.mark_all_dirty();
                    return;
                }
                Keysym::Down => {
                    self.power_menu.kb_active = true;
                    self.power_menu.select_next();
                    self.mark_all_dirty();
                    return;
                }
                Keysym::Return | Keysym::KP_Enter => {
                    self.power_menu.kb_active = true;
                    if let Some(action) = self.power_menu.current() {
                        self.power_menu.open = false;
                        self.power_menu.kb_active = false;
                        self.mark_all_dirty();
                        dispatch_power_lock(action);
                    }
                    return;
                }
                _ => {}
            }
        }
        if event.keysym == Keysym::F12 {
            self.power_menu.open = true;
            self.power_menu.kb_active = true;
            self.power_menu.clamp_selection();
            self.mark_all_dirty();
            return;
        }

        match event.keysym {
            Keysym::Return | Keysym::KP_Enter => self.submit_password(),
            Keysym::BackSpace => {
                self.prompt_password.pop();
                self.mark_all_dirty();
            }
            Keysym::Escape => {
                self.prompt_password.clear();
                self.mark_all_dirty();
            }
            _ => {
                if let Some(s) = event.utf8.as_deref() {
                    if !s.is_empty() && !s.chars().any(|c| c.is_control()) {
                        self.prompt_password.push_str(s);
                        self.mark_all_dirty();
                    }
                }
            }
        }
    }
    fn release_key(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _serial: u32,
        _event: KeyEvent,
    ) {}
    fn update_modifiers(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _keyboard: &WlKeyboard,
        _serial: u32,
        modifiers: Modifiers,
        _layout: u32,
    ) {
        if self.is_lock_mode && self.prompt_capslock != modifiers.caps_lock {
            self.prompt_capslock = modifiers.caps_lock;
            self.mark_all_dirty();
        }
    }
}

impl PointerHandler for AppState {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        qh: &QueueHandle<Self>,
        pointer: &WlPointer,
        events: &[PointerEvent],
    ) {
        for e in events {
            if let PointerEventKind::Enter { serial } = e.kind {
                if self.cursor_device.is_none() {
                    if let Some(mgr) = self.cursor_shape.as_ref() {
                        self.cursor_device = Some(mgr.get_shape_device(pointer, qh));
                    }
                }
                if let Some(dev) = self.cursor_device.as_ref() {
                    dev.set_shape(serial, CursorShape::Default);
                }
            }
        }

        if !self.is_lock_mode {
            for e in events {
                match e.kind {
                    PointerEventKind::Press { .. }
                    | PointerEventKind::Release { .. }
                    | PointerEventKind::Axis { .. } => self.handle_input(),
                    _ => {}
                }
            }
            return;
        }

        let mut dirty = false;
        let mut press_at: Option<(usize, f32, f32)> = None;
        for e in events {
            let surface_idx = self
                .surfaces
                .iter()
                .position(|s| s.shell.wl_surface() == &e.surface);
            match e.kind {
                PointerEventKind::Enter { .. } => {
                    if surface_idx.is_some() {
                        self.power_menu.pointer =
                            Some((e.position.0 as f32, e.position.1 as f32));
                        if self.power_menu.open {
                            dirty = true;
                        }
                    }
                }
                PointerEventKind::Motion { .. } => {
                    if let Some(idx) = surface_idx {
                        let new_x = e.position.0 as f32;
                        let new_y = e.position.1 as f32;
                        let rect = OutputRect {
                            x: 0,
                            y: 0,
                            w: self.surfaces[idx].width as i32,
                            h: self.surfaces[idx].height as i32,
                        };
                        if self.power_menu.open {
                            let old_hit = self.power_menu.pointer.map(|(ox, oy)| {
                                power_ui::hit_test(&self.power_menu, &[rect], ox, oy)
                            });
                            let new_hit =
                                power_ui::hit_test(&self.power_menu, &[rect], new_x, new_y);
                            if old_hit != Some(new_hit) {
                                dirty = true;
                            }
                        }
                        self.power_menu.pointer = Some((new_x, new_y));
                    }
                }
                PointerEventKind::Leave { .. } => {
                    let was_open = self.power_menu.open;
                    self.power_menu.pointer = None;
                    if was_open {
                        dirty = true;
                    }
                }
                PointerEventKind::Press { button, .. } if button == 0x110 => {
                    if let Some(idx) = surface_idx {
                        press_at = Some((idx, e.position.0 as f32, e.position.1 as f32));
                    }
                }
                _ => {}
            }
        }

        if let Some((idx, cx, cy)) = press_at {
            // Wake the prompt first; only after the surface is visible
            // does the hit-test make sense.
            let transition = self.lock_state.as_mut().and_then(|ls| {
                let prev = ls.phase();
                ls.on_input(Instant::now());
                let curr = ls.phase();
                (prev != curr).then_some((prev, curr))
            });
            if let Some((from, to)) = transition {
                self.on_phase_change(from, to);
            }

            let rect = OutputRect {
                x: 0,
                y: 0,
                w: self.surfaces[idx].width as i32,
                h: self.surfaces[idx].height as i32,
            };
            match power_ui::hit_test(&self.power_menu, &[rect], cx, cy) {
                PowerHit::ToggleButton => {
                    self.power_menu.open = !self.power_menu.open;
                    self.power_menu.kb_active = false;
                    self.power_menu.clamp_selection();
                    dirty = true;
                }
                PowerHit::Item(action) => {
                    self.power_menu.open = false;
                    self.power_menu.kb_active = false;
                    dirty = true;
                    dispatch_power_lock(action);
                }
                PowerHit::None => {
                    if self.power_menu.open {
                        self.power_menu.open = false;
                        self.power_menu.kb_active = false;
                        dirty = true;
                    }
                }
            }
        }

        if dirty {
            self.mark_all_dirty();
        }
    }
}

fn dispatch_power_lock(action: PowerAction) {
    let verb = match action {
        PowerAction::Restart => "reboot",
        PowerAction::Shutdown => "poweroff",
    };
    eprintln!("shedos-screensaver-wayland: dispatch_power_lock: {:?} (systemctl {})", action, verb);
    std::thread::spawn(move || {
        let out = std::process::Command::new("systemctl").arg(verb).output();
        match out {
            Ok(o) if o.status.success() => {}
            Ok(o) => eprintln!(
                "shedos-screensaver-wayland: systemctl {}: status={} stderr={:?}",
                verb,
                o.status,
                String::from_utf8_lossy(&o.stderr).trim()
            ),
            Err(e) => eprintln!(
                "shedos-screensaver-wayland: systemctl {} spawn failed: {e}",
                verb
            ),
        }
    });
}

impl LayerShellHandler for AppState {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, layer: &LayerSurface) {
        if let Some(idx) = self.surface_index_by_layer(layer) {
            self.surfaces.remove(idx);
        }
        // If the compositor closed every surface, treat as a dismiss.
        if self.surfaces.is_empty() {
            self.input_dismissed = true;
        }
    }
    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        let Some(idx) = self.surface_index_by_layer(layer) else {
            return;
        };
        // wlr-layer-shell spec: `configure` with a zero dimension means
        // "compositor leaves it to the client". Hyprland sends (0, 0)
        // for fullscreen-anchored overlays; fall back to the output's
        // logical or current-mode size.
        let output = self.surfaces[idx].output.clone();
        let (mut w, mut h) = configure.new_size;
        if w == 0 || h == 0 {
            if let Some(info) = self.output_state.info(&output) {
                let fallback = info
                    .logical_size
                    .map(|(lw, lh)| (lw.max(0) as u32, lh.max(0) as u32))
                    .or_else(|| {
                        info.modes
                            .iter()
                            .find(|m| m.current)
                            .map(|m| (m.dimensions.0.max(0) as u32, m.dimensions.1.max(0) as u32))
                    });
                if let Some((fw, fh)) = fallback {
                    if w == 0 {
                        w = fw;
                    }
                    if h == 0 {
                        h = fh;
                    }
                }
            }
        }
        let s = &mut self.surfaces[idx];
        if w > 0 && h > 0 {
            s.width = w;
            s.height = h;
            let needed = (w * h * 4) as usize;
            let _ = s.pool.resize(needed.max(4));
            s.configured = true;
            s.wallpaper_cache = None;
            s.needs_redraw = true;
        }
    }
}

impl ShmHandler for AppState {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for AppState {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
    registry_handlers![OutputState, SeatState];
}

delegate_compositor!(AppState);
delegate_output!(AppState);
delegate_seat!(AppState);
delegate_keyboard!(AppState);
delegate_pointer!(AppState);
delegate_layer!(AppState);
delegate_shm!(AppState);
delegate_registry!(AppState);
