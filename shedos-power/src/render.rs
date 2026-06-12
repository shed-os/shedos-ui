use std::time::Instant;

use anyhow::{Context, Result};
use shedos_prompt_ui::text::{FontFace, JBM_BOLD_CANDIDATES, JBM_REGULAR_CANDIDATES};
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

use crate::widgets::{
    self, dispatch_action, hit_test_confirm, hit_test_menu, is_inside_confirm_card,
    is_inside_menu_card, Action, ConfirmFocus, Phase, PowerState,
};

pub fn run() -> Result<()> {
    let conn = Connection::connect_to_env().context("connect to Wayland (WAYLAND_DISPLAY)")?;
    let (globals, mut event_queue) =
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
        Some("shedos-power"),
        None,
    );
    layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
    layer.set_exclusive_zone(-1);
    layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
    layer.commit();

    let pool = SlotPool::new(4, &shm).context("create wl_shm slot pool")?;

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
        qh: qh.clone(),
        size: None,
        state: PowerState::new(),
        regular,
        bold,
        pointer_pos: None,
        exit: false,
    };

    while !app.exit {
        event_queue
            .blocking_dispatch(&mut app)
            .context("Wayland event dispatch")?;
        let now = Instant::now();
        if app.state.tick(now) {
            app.request_redraw();
        }
        if app.state.dismiss_done(now) {
            app.exit = true;
        }
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
    qh: QueueHandle<Self>,
    size: Option<(u32, u32)>,
    state: PowerState,
    regular: FontFace,
    bold: FontFace,
    pointer_pos: Option<(f32, f32)>,
    exit: bool,
}

impl App {
    fn request_redraw(&mut self) {
        self.draw();
    }

    fn draw(&mut self) {
        let Some((w, h)) = self.size else {
            return;
        };
        if w == 0 || h == 0 {
            return;
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

        let now = Instant::now();
        widgets::paint(canvas, w, h, &self.state, &self.regular, &self.bold, now);

        let surface = self.layer.wl_surface().clone();
        surface.attach(Some(buffer.wl_buffer()), 0, 0);
        surface.damage_buffer(0, 0, w as i32, h as i32);
        surface.frame(&self.qh, surface.clone());
        surface.commit();
    }

    fn handle_pointer_motion(&mut self, x: f32, y: f32) {
        self.pointer_pos = Some((x, y));
        let Some((w, h)) = self.size else { return };
        let mut changed = false;
        match self.state.phase {
            Phase::Menu => {
                if let Some(row) = hit_test_menu(w, h, x, y) {
                    if self.state.focus_menu != row {
                        self.state.focus_menu = row;
                        changed = true;
                    }
                }
            }
            Phase::Confirming => {
                if let Some(focus) = hit_test_confirm(w, h, x, y) {
                    if self.state.focus_confirm != focus {
                        self.state.focus_confirm = focus;
                        changed = true;
                    }
                }
            }
            _ => {}
        }
        if changed {
            self.draw();
        }
    }

    fn handle_click(&mut self, x: f32, y: f32) {
        let Some((w, h)) = self.size else { return };
        match self.state.phase {
            Phase::Menu => {
                if let Some(row) = hit_test_menu(w, h, x, y) {
                    let action = Action::all()[row];
                    self.activate(action);
                } else if !is_inside_menu_card(w, h, x, y) {
                    self.state.enter_dismiss();
                    self.draw();
                }
            }
            Phase::Confirming => {
                let action = self.state.action.unwrap_or(Action::Lock);
                if let Some(focus) = hit_test_confirm(w, h, x, y) {
                    match focus {
                        ConfirmFocus::Cancel => {
                            self.state.enter_menu();
                            self.draw();
                        }
                        ConfirmFocus::Commit => {
                            self.commit_action(action);
                        }
                    }
                } else if !is_inside_confirm_card(w, h, x, y) {
                    self.state.enter_dismiss();
                    self.draw();
                }
            }
            _ => {}
        }
    }

    fn activate(&mut self, action: Action) {
        match action {
            // Lock and Sleep are non-destructive; no confirmation step.
            // Lock is trivially reversible; everything else gets the
            // confirm card — sleep/hibernate interrupt work too.
            Action::Lock => {
                self.commit_action(action);
            }
            Action::Suspend | Action::Hibernate | Action::Restart | Action::Shutdown => {
                self.state.enter_confirming(action);
                self.draw();
            }
        }
    }

    fn commit_action(&mut self, action: Action) {
        match dispatch_action(action) {
            Ok(_) => {}
            Err(e) => {
                log::warn!("dispatch {:?} failed: {e}", action);
            }
        }
        self.state.action = Some(action);
        self.state.enter_dismiss();
        self.draw();
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
    fn frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: u32) {
        // Animations drive the next frame: while we're animating,
        // every frame callback triggers a redraw. Once settled, we
        // stop scheduling new frame callbacks.
        if !self.state.is_settled() || self.state.is_dismissing() {
            self.draw();
        }
    }
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
        if matches!(self.state.phase, Phase::Dismissing) {
            return;
        }
        match event.keysym {
            Keysym::Escape | Keysym::q => match self.state.phase {
                Phase::Confirming | Phase::SwapToConfirm => {
                    self.state.enter_menu();
                    self.draw();
                }
                _ => {
                    self.state.enter_dismiss();
                    self.draw();
                }
            },
            Keysym::Return | Keysym::KP_Enter => match self.state.phase {
                Phase::Menu | Phase::Opening => {
                    let action = self.state.focused_action();
                    self.activate(action);
                }
                Phase::Confirming => {
                    let action = self.state.action.unwrap_or(Action::Lock);
                    match self.state.focus_confirm {
                        ConfirmFocus::Cancel => {
                            self.state.enter_menu();
                            self.draw();
                        }
                        ConfirmFocus::Commit => {
                            self.commit_action(action);
                        }
                    }
                }
                _ => {}
            },
            Keysym::Down | Keysym::j | Keysym::Tab => {
                match self.state.phase {
                    Phase::Menu | Phase::Opening => self.state.move_menu(1),
                    Phase::Confirming | Phase::SwapToConfirm => self.state.toggle_confirm(),
                    _ => {}
                }
                self.draw();
            }
            Keysym::Up | Keysym::k | Keysym::ISO_Left_Tab => {
                match self.state.phase {
                    Phase::Menu | Phase::Opening => self.state.move_menu(-1),
                    Phase::Confirming | Phase::SwapToConfirm => self.state.toggle_confirm(),
                    _ => {}
                }
                self.draw();
            }
            Keysym::h => {
                if matches!(self.state.phase, Phase::Confirming) {
                    self.state.focus_confirm = ConfirmFocus::Cancel;
                    self.draw();
                }
            }
            Keysym::l => {
                if matches!(self.state.phase, Phase::Confirming) {
                    self.state.focus_confirm = ConfirmFocus::Commit;
                    self.draw();
                }
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
        let mut click: Option<(f32, f32)> = None;
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
                    self.handle_pointer_motion(e.position.0 as f32, e.position.1 as f32);
                }
                PointerEventKind::Motion { .. } => {
                    self.handle_pointer_motion(e.position.0 as f32, e.position.1 as f32);
                }
                PointerEventKind::Leave { .. } => {
                    self.pointer_pos = None;
                }
                PointerEventKind::Press { button: 0x110, .. } => {
                    click = Some((e.position.0 as f32, e.position.1 as f32));
                }
                _ => {}
            }
        }
        if let Some((x, y)) = click {
            self.handle_click(x, y);
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
