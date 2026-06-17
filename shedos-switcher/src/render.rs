use std::os::unix::net::UnixListener;

use anyhow::{Context, Result};
use shedos_prompt_ui::text::{FontFace, JBM_BOLD_CANDIDATES, JBM_REGULAR_CANDIDATES};
use shedos_prompt_ui::LiveTheme;
use smithay_client_toolkit::reexports::{
    calloop::{channel, ping::make_ping, EventLoop},
    calloop_wayland_source::WaylandSource,
};
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
            KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
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

use crate::model::{self, Window};
use crate::ui;

#[derive(Debug, Clone, Copy)]
pub enum Cmd {
    Next,
    Prev,
}

pub type CmdSender = channel::Sender<Cmd>;

pub fn run(windows: Vec<Window>, listener: UnixListener) -> Result<()> {
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

    let regular = FontFace::load(JBM_REGULAR_CANDIDATES)?;
    let bold = FontFace::load(JBM_BOLD_CANDIDATES)?;

    let (sw, sh) = ui::strip_size(windows.len());
    let surface = compositor.create_surface(&qh);
    let layer = layer_shell.create_layer_surface(
        &qh,
        surface,
        Layer::Overlay,
        Some("shedos-switcher"),
        None,
    );
    // No anchors: the compositor centers the surface on the output.
    layer.set_size(sw, sh);
    layer.set_exclusive_zone(-1);
    layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
    layer.commit();

    let pool = SlotPool::new(4, &shm).context("create wl_shm slot pool")?;

    let mut event_loop: EventLoop<App> =
        EventLoop::try_new().context("calloop event loop")?;
    let loop_handle = event_loop.handle();

    // The theme watcher pings this so the strip re-themes live if a
    // `shedman theme set` lands while it's open; the handler redraws.
    let (theme_ping, theme_ping_source) = make_ping().context("theme calloop ping")?;
    let live = LiveTheme::new(move || theme_ping.ping());

    let mut app = App {
        registry_state,
        output_state,
        seat_state,
        shm,
        layer,
        pool,
        pointer: None,
        keyboard: None,
        size: None,
        windows,
        // Classic Alt-Tab: start on the previous window so a quick
        // tap toggles between the two most recent.
        selected: 1,
        alt_seen_held: false,
        regular,
        bold,
        live,
        outcome: Outcome::Pending,
    };

    loop_handle
        .insert_source(theme_ping_source, |_, _, app: &mut App| app.draw())
        .map_err(|e| anyhow::anyhow!("calloop theme ping insert: {e}"))?;

    let (tx, rx) = channel::channel::<Cmd>();
    std::thread::spawn(move || crate::listen(listener, tx));
    loop_handle
        .insert_source(rx, |event, _, app: &mut App| {
            if let channel::Event::Msg(cmd) = event {
                match cmd {
                    Cmd::Next => app.cycle(1),
                    Cmd::Prev => app.cycle(-1),
                }
            }
        })
        .map_err(|e| anyhow::anyhow!("calloop channel insert: {e}"))?;

    WaylandSource::new(conn.clone(), event_queue)
        .insert(loop_handle)
        .map_err(|e| anyhow::anyhow!("calloop wayland insert: {e}"))?;

    while matches!(app.outcome, Outcome::Pending) {
        event_loop
            .dispatch(None, &mut app)
            .context("event loop dispatch")?;
    }

    let target = if let Outcome::Commit(i) = app.outcome {
        app.windows.get(i).map(|w| (w.address.clone(), w.title.clone()))
    } else {
        None
    };

    // Destroy the overlay BEFORE focusing: Hyprland restores focus to
    // the previously focused window when an exclusive-keyboard layer
    // surface closes, which would override our dispatch if it came
    // first.
    drop(app);
    conn.flush().ok();
    std::thread::sleep(std::time::Duration::from_millis(60));

    if let Some((addr, _title)) = target {
        model::focus(&addr);
    }
    Ok(())
}

enum Outcome {
    Pending,
    Commit(usize),
    Cancel,
}

struct App {
    registry_state: RegistryState,
    output_state: OutputState,
    seat_state: SeatState,
    shm: Shm,
    layer: LayerSurface,
    pool: SlotPool,
    pointer: Option<WlPointer>,
    keyboard: Option<WlKeyboard>,
    size: Option<(u32, u32)>,
    windows: Vec<Window>,
    selected: usize,
    /// Set once a modifiers update reports Alt held; release-commit
    /// only arms after that, so launches without Alt (e.g. from a
    /// terminal for testing) don't instantly close.
    alt_seen_held: bool,
    regular: FontFace,
    bold: FontFace,
    live: LiveTheme,
    outcome: Outcome,
}

impl App {
    fn draw(&mut self) {
        let Some((w, h)) = self.size else { return };
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
        // The rounded panel leaves corner pixels untouched; clear so
        // they're transparent rather than stale.
        canvas.fill(0);

        self.live.reload_if_dirty(); // no derived caches to refresh
        let palette = ui::Palette::from_theme(self.live.theme());
        ui::paint(
            canvas, w, h, &self.windows, self.selected, &self.regular, &self.bold, &palette,
        );

        let surface = self.layer.wl_surface().clone();
        surface.attach(Some(buffer.wl_buffer()), 0, 0);
        surface.damage_buffer(0, 0, w as i32, h as i32);
        surface.commit();
    }

    fn cycle(&mut self, delta: i32) {
        let n = self.windows.len() as i32;
        self.selected = ((self.selected as i32 + delta).rem_euclid(n)) as usize;
        self.draw();
    }
}

impl LayerShellHandler for App {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.outcome = Outcome::Cancel;
    }

    fn configure(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _: u32,
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
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlSurface, _: i32) {}
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
        _: &Connection,
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
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlKeyboard, _: &WlSurface, _: u32) {
        // Losing the grab (another exclusive surface) cancels cleanly.
        self.outcome = Outcome::Cancel;
    }
    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        match event.keysym {
            Keysym::Tab | Keysym::Right | Keysym::Down => self.cycle(1),
            Keysym::ISO_Left_Tab | Keysym::Left | Keysym::Up => self.cycle(-1),
            Keysym::Return | Keysym::KP_Enter | Keysym::space => {
                self.outcome = Outcome::Commit(self.selected);
            }
            Keysym::Escape | Keysym::q => {
                self.outcome = Outcome::Cancel;
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
        event: KeyEvent,
    ) {
        // The raw Alt key-up is the commit signal. More reliable than
        // the modifiers event under an active compositor bind, which
        // may not forward modifier-state changes to the layer surface.
        if matches!(event.keysym, Keysym::Alt_L | Keysym::Alt_R) {
            self.outcome = Outcome::Commit(self.selected);
        }
    }
    fn update_modifiers(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlKeyboard,
        _: u32,
        modifiers: Modifiers,
        _: u32,
    ) {
        if modifiers.alt {
            self.alt_seen_held = true;
        } else if self.alt_seen_held {
            // The defining Alt-Tab gesture: releasing Alt commits.
            self.outcome = Outcome::Commit(self.selected);
        }
    }
}

impl PointerHandler for App {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlPointer,
        events: &[PointerEvent],
    ) {
        for e in events {
            match e.kind {
                PointerEventKind::Motion { .. } | PointerEventKind::Enter { .. } => {
                    let x = e.position.0 as i32;
                    let i = (x - ui::STRIP_PAD) / (ui::CELL_W + ui::CELL_GAP);
                    if i >= 0 && (i as usize) < self.windows.len() && i as usize != self.selected
                    {
                        self.selected = i as usize;
                        self.draw();
                    }
                }
                PointerEventKind::Press { button: 0x110, .. } => {
                    self.outcome = Outcome::Commit(self.selected);
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
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
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
