//! Layer-shell overlay surface + wl_shm framebuffer + the Wayland
//! event loop that drives the frame producer.

use crate::font::FontAtlas;
use crate::wallpaper::Wallpaper;
use crate::{blend_over, pack_argb, FrameProducer, WaylandConfig};
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
use std::time::Instant;
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_keyboard::WlKeyboard, wl_output::WlOutput, wl_pointer::WlPointer, wl_seat::WlSeat, wl_shm, wl_surface::WlSurface},
    Connection, QueueHandle,
};

/// Renderer entry point. Owns the connection, surface, and frame
/// producer; runs until `should_exit` flips or the user generates
/// input (unless `idle_daemon` is set).
pub struct WaylandRenderer;

impl WaylandRenderer {
    pub fn run(
        config: WaylandConfig,
        producer: Box<dyn FrameProducer>,
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

        // Provisional surface; we'll resize the SHM pool once the
        // compositor sends the first configure with real dimensions.
        let surface = compositor_state.create_surface(&qh);
        let layer = layer_shell.create_layer_surface(
            &qh,
            surface,
            Layer::Overlay,
            Some("shedos-screensaver"),
            None, // place on the default output
        );
        layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
        layer.set_exclusive_zone(-1);
        layer.set_keyboard_interactivity(if config.idle_daemon {
            // Idle daemon mode wants pre-input dismiss via SIGUSR1, not a
            // direct keyboard grab — leaves the focused app reacting to
            // the user's first keypress while we tear down on the signal.
            KeyboardInteractivity::OnDemand
        } else {
            KeyboardInteractivity::Exclusive
        });
        layer.commit();

        // Provisional pool (1x1) — we'll grow it on first configure.
        let pool = SlotPool::new(4, &shm).map_err(|e| WaylandError::Pool(format!("{e}")))?;

        let font = FontAtlas::load(config.font_path.as_deref(), config.cell_height_px as f32)
            .map_err(|e| WaylandError::Font(format!("{e}")))?;

        let mut state = AppState {
            registry_state,
            output_state,
            seat_state,
            shm,
            layer,
            pool,
            width: 0,
            height: 0,
            configured: false,
            keyboard: None,
            pointer: None,
            should_exit: Arc::clone(&should_exit),
            input_dismissed: false,
            idle_daemon: config.idle_daemon,
            producer,
            font,
            wallpaper_path: config.wallpaper_path,
            wallpaper_dim: config.wallpaper_dim,
            wallpaper_cache: None,
            last_frame: None,
            frame: Frame::new(0, 0),
            needs_redraw: true,
        };
        let _ = config.fps_cap;

        // Wait for the first configure so width/height land before we
        // start rendering. Block on the queue until configured=true.
        while !state.configured && !state.should_exit() {
            event_queue
                .blocking_dispatch(&mut state)
                .map_err(|e| WaylandError::Dispatch(format!("{e}")))?;
        }
        if state.should_exit() {
            return Ok(());
        }

        // Render loop driven by frame callbacks. blocking_dispatch reads
        // the wayland socket so wl_buffer.release events arrive and the
        // SlotPool reuses slots — using dispatch_pending here would let
        // releases pile up in the kernel buffer and the pool would grow
        // by ~one framebuffer per render (≈8 MB/frame at 1080p).
        while !state.should_exit() && !state.input_dismissed {
            event_queue
                .blocking_dispatch(&mut state)
                .map_err(|e| WaylandError::Dispatch(format!("{e}")))?;
            if !state.needs_redraw {
                continue;
            }
            state.needs_redraw = false;
            state.render_one(&qh)?;
            state.last_frame = Some(Instant::now());
        }

        Ok(())
    }
}

struct AppState {
    registry_state: RegistryState,
    output_state: OutputState,
    seat_state: SeatState,
    shm: Shm,
    layer: LayerSurface,
    pool: SlotPool,
    width: u32,
    height: u32,
    configured: bool,
    keyboard: Option<WlKeyboard>,
    pointer: Option<WlPointer>,
    should_exit: Arc<AtomicBool>,
    input_dismissed: bool,
    idle_daemon: bool,
    producer: Box<dyn FrameProducer>,
    font: FontAtlas,
    wallpaper_path: Option<PathBuf>,
    wallpaper_dim: f32,
    wallpaper_cache: Option<Wallpaper>,
    last_frame: Option<Instant>,
    frame: Frame,
    needs_redraw: bool,
}

impl AppState {
    fn should_exit(&self) -> bool {
        self.should_exit.load(Ordering::Acquire)
    }

    fn handle_input(&mut self) {
        if self.idle_daemon {
            // Don't dismiss on input — the SIGUSR1 path is what tears
            // us down so the lock screen can take over without a race.
            return;
        }
        self.input_dismissed = true;
    }

    fn render_one(&mut self, qh: &QueueHandle<Self>) -> Result<(), WaylandError> {
        if self.width == 0 || self.height == 0 {
            return Ok(());
        }

        // Lazy-load the wallpaper now that we know the surface size.
        if self.wallpaper_cache.is_none() {
            if let Some(path) = &self.wallpaper_path {
                match Wallpaper::prepare(path, self.width, self.height, self.wallpaper_dim) {
                    Ok(w) => self.wallpaper_cache = Some(w),
                    Err(e) => eprintln!(
                        "shedos-screensaver-wayland: wallpaper '{}' failed: {e}; \
                         drawing on solid base",
                        path.display()
                    ),
                }
            }
        }

        let (cell_w, cell_h) = self.font.cell_size();
        let cols = (self.width / cell_w).max(1) as u16;
        let rows = (self.height / cell_h).max(1) as u16;
        if (self.frame.cols(), self.frame.rows()) != (cols, rows) {
            self.frame = Frame::new(rows, cols);
        }
        self.frame.clear();
        self.producer.produce(&mut self.frame);

        let stride = (self.width as i32) * 4;
        let total_bytes = (self.height as i32) * stride;
        let (buffer, canvas) = self
            .pool
            .create_buffer(
                self.width as i32,
                self.height as i32,
                stride,
                wl_shm::Format::Argb8888,
            )
            .map_err(|e| WaylandError::Pool(format!("create_buffer: {e}")))?;
        debug_assert_eq!(canvas.len(), total_bytes as usize);

        // 1) Fill with wallpaper or BASE color.
        let pixels: &mut [u32] = unsafe {
            // Safe: SlotPool guarantees 4-byte alignment of the canvas
            // (Argb8888 is 4 bytes per pixel) and the slice length is a
            // multiple of 4. We're writing into shared memory the
            // compositor will read after we attach the buffer.
            let ptr = canvas.as_mut_ptr() as *mut u32;
            std::slice::from_raw_parts_mut(ptr, (canvas.len() / 4) as usize)
        };
        if let Some(wp) = &self.wallpaper_cache {
            // Wallpaper is sized to (self.width, self.height) so this
            // copies one-to-one.
            pixels.copy_from_slice(&wp.pixels);
        } else {
            let base = pack_argb(Color::BASE);
            for p in pixels.iter_mut() {
                *p = base;
            }
        }

        // 2) Composite cells on top. Hoist the baseline outside the
        // loop so the per-cell `self.font.glyph()` mutable borrow
        // doesn't fight with the immutable `self.font.baseline()`.
        let baseline = self.font.baseline();
        for r in 0..self.frame.rows() {
            for c in 0..self.frame.cols() {
                let cell = self.frame.get(r, c).expect("in-bounds row/col");
                if cell.ch == ' ' {
                    continue; // wallpaper / base shows through
                }
                let glyph = self.font.glyph(cell.ch);
                let cell_x0 = (c as i32) * (cell_w as i32);
                let cell_y0 = (r as i32) * (cell_h as i32);
                let glyph_x0 = cell_x0 + glyph.x_offset;
                let glyph_y0 = cell_y0 + baseline + glyph.y_offset;
                for gy in 0..glyph.height as i32 {
                    let dst_y = glyph_y0 + gy;
                    if dst_y < 0 || dst_y >= self.height as i32 {
                        continue;
                    }
                    for gx in 0..glyph.width as i32 {
                        let dst_x = glyph_x0 + gx;
                        if dst_x < 0 || dst_x >= self.width as i32 {
                            continue;
                        }
                        let alpha = glyph.bitmap[(gy * glyph.width as i32 + gx) as usize];
                        if alpha == 0 {
                            continue;
                        }
                        let i = (dst_y as usize) * (self.width as usize) + dst_x as usize;
                        pixels[i] = blend_over(cell.fg, pixels[i], alpha);
                    }
                }
            }
        }

        // 3) Request the next frame callback before commit so the
        // compositor schedules us at vsync, then attach + commit.
        let surface = self.layer.wl_surface().clone();
        surface.frame(qh, surface.clone());
        surface.damage_buffer(0, 0, self.width as i32, self.height as i32);
        buffer
            .attach_to(&surface)
            .map_err(|e| WaylandError::Buffer(format!("attach: {e}")))?;
        self.layer.commit();
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
        _surface: &WlSurface,
        _time: u32,
    ) {
        self.needs_redraw = true;
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
    fn new_output(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _output: WlOutput) {}
    fn update_output(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _output: WlOutput) {}
    fn output_destroyed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _output: WlOutput) {}
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
        _event: KeyEvent,
    ) {
        self.handle_input();
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
        _modifiers: Modifiers,
        _layout: u32,
    ) {}
}

impl PointerHandler for AppState {
    fn pointer_frame(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _pointer: &WlPointer,
        events: &[PointerEvent],
    ) {
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
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.input_dismissed = true;
    }
    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        let (w, h) = configure.new_size;
        if w > 0 && h > 0 {
            self.width = w;
            self.height = h;
            // Grow the SHM pool to fit one Argb8888 framebuffer.
            let needed = (w * h * 4) as usize;
            let _ = self.pool.resize(needed.max(4));
            self.configured = true;
            // Wallpaper cache is sized to the surface; force re-prep on next render.
            self.wallpaper_cache = None;
            self.needs_redraw = true;
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
