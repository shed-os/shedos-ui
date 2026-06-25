use anyhow::{Context, Result};
use shedos_prompt_ui::text::{FontFace, JBM_BOLD_CANDIDATES, JBM_REGULAR_CANDIDATES};
use shedos_prompt_ui::LiveTheme;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState},
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
use smithay_client_toolkit::reexports::{
    calloop::{ping::make_ping, EventLoop},
    calloop_wayland_source::WaylandSource,
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{
        wl_keyboard::WlKeyboard, wl_output, wl_pointer::WlPointer, wl_seat::WlSeat, wl_shm,
        wl_surface::WlSurface,
    },
    Connection, QueueHandle,
};
use wayland_protocols::wp::cursor_shape::v1::client::wp_cursor_shape_device_v1::{
    Shape as CursorShape, WpCursorShapeDeviceV1,
};

use crate::recovery;
use crate::slides::{self, TourState};

pub fn run(recovery_only: bool) -> Result<()> {
    // Read the stashed recovery key up front. In --recovery mode (the in-place
    // re-trigger) there is nothing to show without one, so exit before grabbing
    // the screen; the normal tour just runs without the extra slide.
    let key = recovery::read_stash(&recovery::stash_path());
    if recovery_only && key.is_none() {
        return Ok(());
    }

    let conn = Connection::connect_to_env().context("connect to Wayland (WAYLAND_DISPLAY)")?;
    let (globals, event_queue) =
        registry_queue_init::<App>(&conn).context("init Wayland registry")?;
    let qh = event_queue.handle();

    let registry_state = RegistryState::new(&globals);
    let output_state = OutputState::new(&globals, &qh);
    let seat_state = SeatState::new(&globals, &qh);
    let compositor =
        CompositorState::bind(&globals, &qh).context("wl_compositor not advertised")?;
    let layer_shell =
        LayerShell::bind(&globals, &qh).context("wlr-layer-shell-unstable-v1 not advertised")?;
    let shm = Shm::bind(&globals, &qh).context("wl_shm not advertised")?;
    let cursor_shape = match CursorShapeManager::bind(&globals, &qh) {
        Ok(m) => Some(m),
        Err(e) => {
            log::warn!("cursor-shape-v1 not advertised: {e}; pointer will use compositor default");
            None
        }
    };

    let regular = FontFace::load(JBM_REGULAR_CANDIDATES)?;
    let bold = FontFace::load(JBM_BOLD_CANDIDATES)?;

    let surface = compositor.create_surface(&qh);
    let layer = layer_shell.create_layer_surface(
        &qh,
        surface,
        Layer::Overlay,
        Some("shedos-tour"),
        None,
    );
    layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
    layer.set_exclusive_zone(-1);
    layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
    layer.commit();

    let pool = SlotPool::new(4, &shm).context("create wl_shm slot pool")?;

    let mut event_loop: EventLoop<App> =
        EventLoop::try_new().context("calloop event loop")?;
    let loop_handle = event_loop.handle();

    // The theme watcher pings this so an idle tour repaints on a live
    // `shedman theme set`; the handler redraws, which reloads the theme.
    let (theme_ping, theme_ping_source) = make_ping().context("theme calloop ping")?;
    loop_handle
        .insert_source(theme_ping_source, |_, _, app: &mut App| app.draw())
        .map_err(|e| anyhow::anyhow!("insert theme ping source: {e}"))?;

    let live = LiveTheme::new(move || theme_ping.ping());
    let wordmark = match shedos_prompt_ui::wordmark::Wordmark::load(&live.theme().wordmark_on_dark)
    {
        Ok(wm) => Some(wm),
        Err(e) => {
            log::warn!("wordmark unavailable: {e:#}; slide 1 renders without it");
            None
        }
    };

    let mut app = App {
        registry_state,
        output_state,
        seat_state,
        shm,
        layer,
        pool,
        cursor_device: None,
        cursor_shape,
        pointer: None,
        keyboard: None,
        size: None,
        state: TourState::new(),
        regular,
        bold,
        wordmark,
        live,
        exit: false,
        recovery: key.map(recovery::Recovery::new),
        recovery_only,
    };

    WaylandSource::new(conn.clone(), event_queue)
        .insert(loop_handle)
        .map_err(|e| anyhow::anyhow!("calloop wayland insert: {e}"))?;

    while !app.exit {
        event_loop
            .dispatch(None, &mut app)
            .context("event loop dispatch")?;
    }

    if app.state.open_keybindings {
        // Detached: the dialog outlives this overlay.
        let _ = std::process::Command::new("/usr/bin/shedman")
            .arg("keybindings")
            .spawn();
    }
    Ok(())
}

struct App {
    registry_state: RegistryState,
    output_state: OutputState,
    seat_state: SeatState,
    shm: Shm,
    layer: LayerSurface,
    pool: SlotPool,
    /// Drops before `pointer`; keep declared first.
    cursor_device: Option<WpCursorShapeDeviceV1>,
    cursor_shape: Option<CursorShapeManager>,
    pointer: Option<WlPointer>,
    keyboard: Option<WlKeyboard>,
    size: Option<(u32, u32)>,
    state: TourState,
    regular: FontFace,
    bold: FontFace,
    wordmark: Option<shedos_prompt_ui::wordmark::Wordmark>,
    live: LiveTheme,
    exit: bool,
    /// Present + unacknowledged means the recovery-key slide owns the screen: all
    /// nav, Esc, and quit are inert until the user types the acknowledgement.
    recovery: Option<recovery::Recovery>,
    recovery_only: bool,
}

impl App {
    fn draw(&mut self) {
        let Some((w, h)) = self.size else {
            return;
        };
        if w == 0 || h == 0 {
            return;
        }
        if self.live.reload_if_dirty() {
            self.wordmark = match shedos_prompt_ui::wordmark::Wordmark::load(
                &self.live.theme().wordmark_on_dark,
            ) {
                Ok(wm) => Some(wm),
                Err(e) => {
                    log::warn!("wordmark reload failed after theme change: {e:#}");
                    None
                }
            };
        }
        let stride = (w * 4) as i32;
        let need = (w as usize) * (h as usize) * 4;
        if need > self.pool.len() {
            if let Err(e) = self.pool.resize(need) {
                log::warn!("shm pool resize failed: {e}");
                return;
            }
        }
        let (buffer, canvas) = match self
            .pool
            .create_buffer(w as i32, h as i32, stride, wl_shm::Format::Argb8888)
        {
            Ok(b) => b,
            Err(e) => {
                log::warn!("create wl_shm buffer: {e}");
                return;
            }
        };

        let palette = slides::Palette::from_theme(self.live.theme());
        if let Some(rec) = self.recovery.as_ref() {
            slides::paint_recovery(canvas, w, h, rec, &self.regular, &self.bold, &palette);
        } else {
            slides::paint(
                canvas, w, h, &self.state, &self.regular, &self.bold,
                self.wordmark.as_mut(), &palette,
            );
        }

        let surface = self.layer.wl_surface().clone();
        surface.attach(Some(buffer.wl_buffer()), 0, 0);
        surface.damage_buffer(0, 0, w as i32, h as i32);
        surface.commit();
    }

    fn advance(&mut self) {
        self.state.next();
        self.draw();
    }

    // Drive the recovery-key acknowledgement: Backspace edits, any other text feeds
    // the phrase buffer. Once it matches, shred the stash and either exit (the
    // --recovery re-trigger) or fall through to the normal tour's first slide.
    fn handle_recovery_key(&mut self, event: KeyEvent) {
        {
            let Some(rec) = self.recovery.as_mut() else { return };
            if event.keysym == Keysym::BackSpace {
                rec.backspace();
            } else if let Some(text) = &event.utf8 {
                for c in text.chars() {
                    rec.type_char(c);
                }
            }
        }
        if self.recovery.as_ref().is_some_and(|r| r.acknowledged()) {
            recovery::shred_stash(&recovery::stash_path());
            if self.recovery_only {
                self.exit = true;
            } else {
                self.recovery = None;
                self.draw();
            }
        } else {
            self.draw();
        }
    }
}

impl LayerShellHandler for App {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if layer != &self.layer {
            return;
        }
        let (w, h) = configure.new_size;
        if w > 0 && h > 0 {
            self.size = Some((w, h));
            self.draw();
        }
    }
}

impl CompositorHandler for App {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlSurface,
        _: i32,
    ) {
    }
    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlSurface,
        _: wl_output::Transform,
    ) {
    }
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: u32) {}
    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl SeatHandler for App {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat) {}
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
        _: &Connection,
        _: &QueueHandle<Self>,
        _: WlSeat,
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
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: WlSeat) {}
}

impl KeyboardHandler for App {
    fn enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlKeyboard,
        _: &WlSurface,
        _: u32,
        _: &[u32],
        _: &[Keysym],
    ) {
    }
    fn leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlKeyboard,
        _: &WlSurface,
        _: u32,
    ) {
    }
    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        // The recovery slide grabs everything until acknowledged — no skip, no quit.
        if self.recovery.is_some() {
            self.handle_recovery_key(event);
            return;
        }
        match event.keysym {
            Keysym::Escape | Keysym::q => {
                self.exit = true;
            }
            Keysym::Return | Keysym::KP_Enter => {
                if self.state.on_last_slide() {
                    self.state.open_keybindings = true;
                    self.exit = true;
                } else {
                    self.advance();
                }
            }
            Keysym::space | Keysym::Right | Keysym::l | Keysym::Down | Keysym::j => {
                self.advance();
            }
            Keysym::Left | Keysym::h | Keysym::Up | Keysym::k => {
                self.state.prev();
                self.draw();
            }
            _ => {}
        }
    }
    fn release_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlKeyboard,
        _: u32,
        _: KeyEvent,
    ) {
    }
    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlKeyboard,
        _: u32,
        _: Modifiers,
        _: u32,
    ) {
    }
}

impl PointerHandler for App {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        pointer: &WlPointer,
        events: &[PointerEvent],
    ) {
        for e in events {
            match e.kind {
                PointerEventKind::Enter { serial } => {
                    if self.cursor_device.is_none() {
                        if let Some(mgr) = self.cursor_shape.as_ref() {
                            self.cursor_device = Some(mgr.get_shape_device(pointer, qh));
                        }
                    }
                    if let Some(dev) = self.cursor_device.as_ref() {
                        dev.set_shape(serial, CursorShape::Default);
                    }
                }
                // Clicks advance the tour, but never the gated recovery slide.
                PointerEventKind::Press { button: 0x110, .. } if self.recovery.is_none() => {
                    self.advance();
                }
                _ => {}
            }
        }
    }
}

impl OutputHandler for App {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
    }
}

impl ShmHandler for App {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl ProvidesRegistryState for App {
    registry_handlers![OutputState, SeatState];
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
}

delegate_compositor!(App);
delegate_keyboard!(App);
delegate_layer!(App);
delegate_output!(App);
delegate_pointer!(App);
delegate_registry!(App);
delegate_seat!(App);
delegate_shm!(App);
