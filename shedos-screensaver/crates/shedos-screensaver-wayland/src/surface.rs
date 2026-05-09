//! Per-output layer-shell overlay surfaces + the Wayland event loop
//! that drives independent frame producers per monitor.
//!
//! Single monitor: one layer surface anchored to all four edges of
//! the only `wl_output`, layered above everything. Multi-monitor: N
//! surfaces, one per `wl_output`, each with its own frame producer
//! so each screen runs an independent (LogoVariant, Effect) cycle.
//!
//! The `producer_factory` closure mints producers lazily as outputs
//! appear (boot-time and via hotplug). For resources that exist
//! once-per-process (like the cpal audio stream) the closure typically
//! `take()`s an Option on first call so a single output gets it and
//! later outputs get `None` — the audio-reactive effects fall back to
//! their silence path on those screens.

use crate::font::FontAtlas;
use crate::lock::LockBinding;
use crate::wallpaper::Wallpaper;
use crate::{blend_over, pack_argb, AuthFn, FrameProducer, LockConfig, WaylandConfig};
use shedos_prompt_ui::{OutputRect, PromptState, RenderParams, Theme, WidgetCache};
use shedos_screensaver_core::{Color, Frame};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler},
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
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const ERROR_HOLD: Duration = Duration::from_secs(2);
use wayland_client::{
    globals::{registry_queue_init, GlobalList},
    protocol::{wl_keyboard::WlKeyboard, wl_output::WlOutput, wl_pointer::WlPointer, wl_seat::WlSeat, wl_shm, wl_surface::WlSurface},
    Connection, EventQueue, Proxy, QueueHandle,
};
use wayland_protocols::ext::session_lock::v1::client::{
    ext_session_lock_manager_v1::ExtSessionLockManagerV1,
    ext_session_lock_surface_v1::ExtSessionLockSurfaceV1,
};

/// Mints frame producers — one per output. Called as outputs are
/// discovered (boot-time and via hotplug). Closures with `take()`-able
/// captures are the canonical way to hand single-instance resources
/// to the first call only.
pub type ProducerFactory = Box<dyn FnMut() -> Box<dyn FrameProducer>>;

/// Renderer entry point. Discovers every wl_output the compositor
/// advertises and runs an independent layer surface + frame producer
/// on each.
pub struct WaylandRenderer;

impl WaylandRenderer {
    pub fn run(
        config: WaylandConfig,
        producer_factory: ProducerFactory,
        should_exit: Arc<AtomicBool>,
    ) -> Result<(), WaylandError> {
        let conn = Connection::connect_to_env()
            .map_err(|e| WaylandError::Connect(format!("{e}")))?;
        let (globals, mut event_queue) = registry_queue_init(&conn)
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
            keyboard: None,
            pointer: None,
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
            theme: None,
            widget_cache: None,
            theme_dirty: None,
            authenticate: None,
            prompt_password: String::new(),
            prompt_capslock: false,
            error: None,
        };
        let _ = config.fps_cap;

        event_queue
            .roundtrip(&mut state)
            .map_err(|e| WaylandError::Dispatch(format!("initial roundtrip: {e}")))?;

        run_render_loop(&mut state, &mut event_queue)
    }

    pub fn run_locked(
        config: WaylandConfig,
        producer_factory: ProducerFactory,
        should_exit: Arc<AtomicBool>,
        lock_config: LockConfig,
    ) -> Result<(), WaylandError> {
        let conn = Connection::connect_to_env()
            .map_err(|e| WaylandError::Connect(format!("{e}")))?;
        let (globals, mut event_queue) = registry_queue_init(&conn)
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

        let font = FontAtlas::load(config.font_path.as_deref(), config.cell_height_px as f32)
            .map_err(|e| WaylandError::Font(format!("{e}")))?;

        let LockConfig {
            theme,
            widget_cache,
            authenticate,
            theme_dirty,
        } = lock_config;

        let mut state = AppState {
            registry_state,
            output_state,
            seat_state,
            shm,
            compositor_state,
            layer_shell,
            qh: qh.clone(),
            keyboard: None,
            pointer: None,
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
            theme: Some(theme),
            widget_cache: Some(widget_cache),
            theme_dirty: Some(theme_dirty),
            authenticate: Some(authenticate),
            prompt_password: String::new(),
            prompt_capslock: false,
            error: None,
        };
        let _ = config.fps_cap;

        event_queue
            .roundtrip(&mut state)
            .map_err(|e| WaylandError::Dispatch(format!("initial roundtrip: {e}")))?;

        let manager: ExtSessionLockManagerV1 = bind_session_lock_manager(&globals, &qh)?;
        let lock = manager.lock(&qh, ());
        state.lock_binding = Some(LockBinding::new(lock));

        let result = drive_locked_session(&mut state, &mut event_queue);

        if let Some(lb) = state.lock_binding.as_mut() {
            lb.close();
        }
        let _ = event_queue.roundtrip(&mut state);

        result
    }
}

fn drive_locked_session(
    state: &mut AppState,
    event_queue: &mut EventQueue<AppState>,
) -> Result<(), WaylandError> {
    // The compositor sends `Locked` only after we commit our first lock-surface
    // buffers, so we don't wait — roundtrip catches any immediate `Finished`,
    // then we mint surfaces and let the render loop drive configures + commits.
    event_queue
        .roundtrip(state)
        .map_err(|e| WaylandError::Dispatch(format!("post-lock roundtrip: {e}")))?;
    if state.lock_binding.as_ref().is_some_and(|lb| lb.finished) {
        return Err(WaylandError::Bind(
            "compositor refused ext-session-lock-v1. Recovery: switch to a tty and \
             run `loginctl unlock-session`."
                .into(),
        ));
    }

    let outputs: Vec<WlOutput> = state.output_state.outputs().collect();
    for o in outputs {
        state.add_output(o);
    }

    run_render_loop(state, event_queue)
}

fn bind_session_lock_manager(
    globals: &GlobalList,
    qh: &QueueHandle<AppState>,
) -> Result<ExtSessionLockManagerV1, WaylandError> {
    globals
        .bind(qh, 1..=1, ())
        .map_err(|e| WaylandError::Bind(format!("ext_session_lock_manager_v1: {e}")))
}

fn run_render_loop(
    state: &mut AppState,
    event_queue: &mut EventQueue<AppState>,
) -> Result<(), WaylandError> {
    // Wait until at least one surface gets its first configure.
    while !state.any_configured() && !state.terminating() {
        event_queue
            .blocking_dispatch(state)
            .map_err(|e| WaylandError::Dispatch(format!("{e}")))?;
    }
    if state.terminating() {
        return Ok(());
    }

    // Render loop. Each surface flips its own `needs_redraw` from
    // its frame callback; we render whichever are ready, then
    // dispatch to wait for the next callback. Order matters: a
    // dispatch-first loop would block on the first iteration
    // because the configure event has already been consumed
    // upstream and no further events arrive until we commit the
    // first buffer.
    while !state.terminating() {
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
        event_queue
            .blocking_dispatch(state)
            .map_err(|e| WaylandError::Dispatch(format!("{e}")))?;
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
}

pub(crate) struct AppState {
    registry_state: RegistryState,
    output_state: OutputState,
    seat_state: SeatState,
    shm: Shm,
    compositor_state: CompositorState,
    layer_shell: LayerShell,
    qh: QueueHandle<Self>,
    keyboard: Option<WlKeyboard>,
    pointer: Option<WlPointer>,
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
    theme: Option<Theme>,
    widget_cache: Option<WidgetCache>,
    theme_dirty: Option<Arc<AtomicBool>>,
    authenticate: Option<AuthFn>,
    prompt_password: String,
    prompt_capslock: bool,
    error: Option<(Instant, String)>,
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

    fn any_configured(&self) -> bool {
        self.surfaces
            .iter()
            .any(|s| s.configured && s.width > 0 && s.height > 0)
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
            Ok(()) => self.should_exit.store(true, Ordering::Release),
            Err(msg) => self.error = Some((Instant::now() + ERROR_HOLD, msg)),
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
        if self.is_lock_mode {
            return self.render_lock_surface(idx);
        }

        // Borrow split: &self.font / &self.wallpaper_path / &self.qh
        // are read-only; the OutputSurface at idx is mutable.
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

    fn render_lock_surface(&mut self, idx: usize) -> Result<(), WaylandError> {
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

        let s = &mut self.surfaces[idx];
        if s.width == 0 || s.height == 0 {
            return Ok(());
        }

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

        let Some(theme) = self.theme.as_ref() else {
            return Ok(());
        };
        let Some(cache) = self.widget_cache.as_mut() else {
            return Ok(());
        };

        let active_error = self
            .error
            .as_ref()
            .filter(|(t, _)| Instant::now() < *t);
        let prompt_state = PromptState {
            typed_chars: self.prompt_password.chars().count(),
            fail: active_error.is_some(),
            success: false,
            capslock: self.prompt_capslock,
        };
        let params = RenderParams {
            greeting: None,
            error_message: active_error.map(|(_, m)| m.as_str()),
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
        _qh: &QueueHandle<Self>,
        _pointer: &WlPointer,
        events: &[PointerEvent],
    ) {
        if self.is_lock_mode {
            return;
        }
        for e in events {
            match e.kind {
                PointerEventKind::Press { .. }
                | PointerEventKind::Release { .. }
                | PointerEventKind::Axis { .. } => self.handle_input(),
                _ => {}
            }
        }
    }
}

impl LayerShellHandler for AppState {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, layer: &LayerSurface) {
        if let Some(idx) = self.surface_index_by_layer(layer) {
            self.surfaces.remove(idx);
        }
        // If the compositor closed every surface we owned, treat it
        // as a dismiss — there's nothing left to render to.
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
        // Per the wlr-layer-shell-unstable-v1 spec, a `configure` with
        // dimension zero in either axis means "compositor leaves it
        // up to the client". Some compositors (Hyprland on certain
        // builds) send (0, 0) for fullscreen-anchored overlays; fall
        // back to the output's logical or current-mode dimensions so
        // we always end up with a usable size.
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
