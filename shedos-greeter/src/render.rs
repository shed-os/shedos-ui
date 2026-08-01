//! Wayland xdg-shell fullscreen surface and keyboard handling.
//! Pixel rendering goes through `shedos-prompt-ui::render` so the
//! greeter stays pixel-identical to the lock screen.
//!
//! Multi-monitor: cage gives a single spanned surface; we collect
//! one `OutputRect` per `wl_output` and let `prompt_ui::render`
//! mirror the full UI on each rect.

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use shedos_prompt_ui::{
    self as prompt_ui, enumerate,
    power::{self as power_ui, PowerAction, PowerHit, PowerMenuState},
    show_username,
    username::{self as username_ui},
    LiveTheme, OutputRect, PromptState, RenderParams, UsernameHit, UsernameMenuState, WidgetCache,
};
use smithay_client_toolkit::reexports::calloop::{
    ping::{make_ping, Ping},
    EventLoop,
};
use smithay_client_toolkit::reexports::calloop_wayland_source::WaylandSource;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_output, delegate_pointer, delegate_registry,
    delegate_seat, delegate_shm, delegate_xdg_shell, delegate_xdg_window,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers},
        pointer::{cursor_shape::CursorShapeManager, PointerEvent, PointerEventKind, PointerHandler},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        xdg::{
            window::{Window, WindowConfigure, WindowDecorations, WindowHandler},
            XdgShell,
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
use wayland_protocols::wp::cursor_shape::v1::client::{
    wp_cursor_shape_device_v1::{Shape as CursorShape, WpCursorShapeDeviceV1},
};
use zeroize::{Zeroize, Zeroizing};

use crate::greetd;
use crate::user;

const ERROR_HOLD: Duration = Duration::from_secs(2);
const ERROR_TEXT: &str = "Authentication Failed";

pub fn run() -> Result<()> {
    let conn = Connection::connect_to_env()
        .context("connect to Wayland display (is WAYLAND_DISPLAY set?)")?;
    let (globals, event_queue) =
        registry_queue_init::<App>(&conn).context("init Wayland registry")?;
    let qh = event_queue.handle();

    let registry_state = RegistryState::new(&globals);
    let output_state = OutputState::new(&globals, &qh);
    let seat_state = SeatState::new(&globals, &qh);
    let compositor =
        CompositorState::bind(&globals, &qh).context("wl_compositor not advertised")?;
    let xdg_shell = XdgShell::bind(&globals, &qh)
        .context("xdg_wm_base not advertised by compositor")?;
    let shm = Shm::bind(&globals, &qh).context("wl_shm not advertised")?;
    let cursor_shape = match CursorShapeManager::bind(&globals, &qh) {
        Ok(m) => Some(m),
        Err(e) => {
            log::warn!("cursor-shape-v1 not advertised: {e}; cursor will be hidden");
            None
        }
    };

    // xdg-shell because cage (the kiosk compositor) advertises
    // xdg_wm_base but not zwlr_layer_shell_v1. cage forces fullscreen
    // at the toplevel, so xdg-shell gives the same UX without the
    // protocol mismatch.
    let surface = compositor.create_surface(&qh);
    let window = xdg_shell.create_window(surface, WindowDecorations::ServerDefault, &qh);
    window.set_title("ShedOS Greeter".to_string());
    window.set_app_id("shedos-greeter".to_string());
    window.set_fullscreen(None);
    window.commit();

    let pool = SlotPool::new(4, &shm).context("create wl_shm slot pool")?;
    let users = enumerate();
    let show_username = show_username();
    let mut username_menu = UsernameMenuState { users: users.clone(), ..Default::default() };
    let username = user::preselect(&users)
        .or_else(|| user::default_pick(&users))
        .or_else(user::resolve);
    if let Some(name) = username.as_deref() {
        username_menu.put_first(name);
    }

    // calloop drives the Wayland queue plus two pings: the auth worker
    // fires `ping` after posting a greetd event (PAM progress repaints
    // without input), and the theme watcher fires `theme_ping` so a live
    // `shedman theme set` repaints even an idle login screen.
    let mut event_loop: EventLoop<App> =
        EventLoop::try_new().context("create calloop event loop")?;
    let (ping, ping_source) = make_ping().context("create calloop ping")?;
    let (theme_ping, theme_ping_source) = make_ping().context("create theme ping")?;

    let live = LiveTheme::new(move || theme_ping.ping());
    log::info!(
        "theme loaded: wallpaper={} blurred={}",
        live.theme().wallpaper.display(),
        live.theme().wallpaper_blurred.display()
    );
    let cache =
        WidgetCache::new(live.theme()).context("initialise prompt-ui cache (fonts + wallpaper)")?;

    let mut app = App {
        registry_state,
        output_state,
        seat_state,
        shm,
        window,
        pool,
        live,
        cache,
        cursor_device: None,
        cursor_shape,
        pointer: None,
        keyboard: None,
        size: None,
        outputs: Vec::new(),
        username,
        username_menu,
        show_username,
        auth_ping: ping.clone(),
        password: Zeroizing::new(String::new()),
        capslock: false,
        power_menu: PowerMenuState::default(),
        error_text: String::new(),
        error_until: None,
        password_tx: None,
        auth_events: None,
        auth_handle: None,
        fingerprint_hint: None,
        authenticating: false,
        exit: false,
    };

    event_loop
        .handle()
        .insert_source(ping_source, |_, _, app: &mut App| {
            app.drain_auth_events();
        })
        .map_err(|e| anyhow::anyhow!("insert ping source: {e}"))?;
    event_loop
        .handle()
        .insert_source(theme_ping_source, |_, _, app: &mut App| app.draw())
        .map_err(|e| anyhow::anyhow!("insert theme ping source: {e}"))?;
    WaylandSource::new(conn.clone(), event_queue)
        .insert(event_loop.handle())
        .map_err(|e| anyhow::anyhow!("insert wayland source: {e}"))?;

    // Start the greetd conversation eagerly so the PAM session is
    // already waiting at the password prompt on the first keystroke.
    if let Some(username) = app.username.clone() {
        let (ev_tx, ev_rx) = std::sync::mpsc::channel();
        let wake_ping = ping.clone();
        let cmd = vec!["/usr/lib/shedos/start-hyprland-session.sh".to_string()];
        let (tx, handle) = greetd::spawn(username, cmd, app.show_username, ev_tx, move || {
            wake_ping.ping();
        });
        app.password_tx = Some(tx);
        app.auth_handle = Some(handle);
        app.auth_events = Some(ev_rx);
    }

    while !app.exit {
        event_loop
            .dispatch(None, &mut app)
            .context("event loop dispatch")?;
    }
    Ok(())
}

struct App {
    registry_state: RegistryState,
    output_state: OutputState,
    seat_state: SeatState,
    shm: Shm,
    window: Window,
    pool: SlotPool,
    live: LiveTheme,
    cache: WidgetCache,
    /// Must drop before `pointer`; keep declared first.
    cursor_device: Option<WpCursorShapeDeviceV1>,
    cursor_shape: Option<CursorShapeManager>,
    pointer: Option<WlPointer>,
    keyboard: Option<WlKeyboard>,
    size: Option<(u32, u32)>,
    /// One rect per `wl_output`, in canvas-local pixels (cage's
    /// spanned surface places its origin at the topleft-most output).
    /// Empty until the first output is announced.
    outputs: Vec<OutputRect>,
    username: Option<String>,
    username_menu: UsernameMenuState,
    show_username: bool,
    /// Wakes the calloop loop after the auth worker posts an event;
    /// cloned into each greetd::spawn, kept here so rebind_user can
    /// hand a fresh worker the same ping.
    auth_ping: Ping,
    /// Zeroized on drop and on clear so the typed secret doesn't linger
    /// in freed heap pages.
    password: Zeroizing<String>,
    capslock: bool,
    power_menu: PowerMenuState,
    /// Error message rendered below the input box during the
    /// `error_until` hold. Populated by `submit()` with the greetd
    /// error so PAM rejection reasons surface inline.
    error_text: String,
    error_until: Option<Instant>,
    /// Auth conversation worker (greetd.rs). Passwords go down the
    /// sender; events come back via the receiver, drained when the
    /// worker pings the calloop loop.
    password_tx: Option<std::sync::mpsc::Sender<Zeroizing<String>>>,
    auth_events: Option<std::sync::mpsc::Receiver<greetd::AuthEvent>>,
    /// The auth worker thread. Joined on rebind so its greetd session is
    /// fully cancelled before the replacement worker connects.
    auth_handle: Option<std::thread::JoinHandle<()>>,
    /// fprintd window open: show the affordance with this hint.
    fingerprint_hint: Option<String>,
    /// A password has been submitted and PAM hasn't answered yet.
    authenticating: bool,
    exit: bool,
}

impl App {
    /// Rebuild `self.outputs` from the current OutputState. Called
    /// from each OutputHandler hook (announce, update, destroy).
    /// Falls back to wl_output::geometry coords + the preferred
    /// mode when xdg-output isn't advertised.
    fn refresh_outputs(&mut self) {
        let mut outs: Vec<OutputRect> = Vec::new();
        for output in self.output_state.outputs() {
            let Some(info) = self.output_state.info(&output) else {
                continue;
            };
            let (x, y) = info.logical_position.unwrap_or(info.location);
            let (w, h) = info.logical_size.unwrap_or_else(|| {
                let mode = info
                    .modes
                    .iter()
                    .find(|m| m.preferred)
                    .or_else(|| info.modes.first());
                let (mw, mh) = mode.map(|m| m.dimensions).unwrap_or((1920, 1080));
                let s = info.scale_factor.max(1);
                (mw / s, mh / s)
            });
            outs.push(OutputRect { x, y, w, h });
        }
        outs.sort_by_key(|o| (o.x, o.y));
        self.outputs = outs;
    }

    fn submit(&mut self) {
        let password = std::mem::take(&mut self.password);
        let Some(tx) = &self.password_tx else {
            log::warn!("submit: no auth worker (set /etc/shedos/login-user)");
            return;
        };
        // The worker answers PAM's prompt with this — immediately if
        // the prompt is already waiting, or buffered until the
        // fingerprint window closes.
        if tx.send(password).is_err() {
            log::warn!("submit: auth worker gone");
            return;
        }
        self.authenticating = true;
        self.draw();
    }

    /// Tear down the current auth worker and spawn a fresh one bound to
    /// `name`. Dropping the old sender ends the worker's `recv`, so its
    /// thread exits and the in-flight Session cancels via Drop; the new
    /// worker re-arms greetd (and the fingerprint reader) for `name`.
    fn rebind_user(&mut self, name: String) {
        self.username = Some(name.clone());
        self.password.zeroize();
        self.authenticating = false;
        self.fingerprint_hint = None;
        self.auth_events = None;
        // Drop the sender to end the worker's recv, then wait for it to
        // exit so its greetd session is fully cancelled before the new
        // worker connects — otherwise the two contend over greetd's
        // single session and wedge it.
        self.password_tx = None;
        if let Some(handle) = self.auth_handle.take() {
            let _ = handle.join();
        }
        let (ev_tx, ev_rx) = std::sync::mpsc::channel();
        let wake_ping = self.auth_ping.clone();
        let cmd = vec!["/usr/lib/shedos/start-hyprland-session.sh".to_string()];
        let (tx, handle) = greetd::spawn(name, cmd, self.show_username, ev_tx, move || {
            wake_ping.ping();
        });
        self.password_tx = Some(tx);
        self.auth_handle = Some(handle);
        self.auth_events = Some(ev_rx);
    }

    /// Drain worker events; called from the calloop ping source.
    fn drain_auth_events(&mut self) {
        let mut redraw = false;
        while let Some(Ok(ev)) = self.auth_events.as_ref().map(|rx| rx.try_recv()) {
            redraw = true;
            match ev {
                greetd::AuthEvent::Fingerprint(hint) => {
                    self.fingerprint_hint = Some(hint);
                }
                greetd::AuthEvent::PromptReady => {
                    // fprintd window over (or never open).
                    self.fingerprint_hint = None;
                }
                greetd::AuthEvent::Failed(msg) => {
                    log::warn!("login failed: {msg}");
                    self.authenticating = false;
                    self.fingerprint_hint = None;
                    self.error_text = if msg.is_empty() { ERROR_TEXT.to_string() } else { msg };
                    self.error_until = Some(Instant::now() + ERROR_HOLD);
                }
                greetd::AuthEvent::SessionStarted => {
                    log::info!("auth + start_session OK; greeter exiting");
                    self.exit = true;
                }
            }
        }
        if redraw {
            self.draw();
        }
    }

    fn draw(&mut self) {
        let Some((w, h)) = self.size else { return };
        if w == 0 || h == 0 {
            return;
        }

        // Live theme reload: pick up reconciler swap before composing.
        if self.live.reload_if_dirty() {
            if let Err(e) = self.cache.refresh_wallpaper(self.live.theme()) {
                log::warn!("wallpaper refresh failed after theme reload: {e:#}");
            }
        }

        // Resolve transient UI state before we acquire the wl_shm
        // buffer borrow (which holds &mut self.pool until commit).
        let error_active = match self.error_until {
            Some(ts) if Instant::now() < ts => true,
            Some(_) => {
                self.error_until = None;
                false
            }
            None => false,
        };
        let typed_chars = self.password.chars().count();
        let greeting = if self.authenticating {
            "Authenticating…".to_string()
        } else {
            self.username.as_ref().map(|n| format!("Hi, {n}")).unwrap_or_else(|| "Hi".to_string())
        };
        let error_msg = if error_active {
            Some(if self.error_text.is_empty() { ERROR_TEXT } else { self.error_text.as_str() })
        } else {
            None
        };
        let state = PromptState {
            typed_chars,
            fail: error_active,
            success: false,
            capslock: self.capslock,
            power_menu: self.power_menu.clone(),
            username_menu: self.username_menu.clone(),
        };

        let stride = (w * 4) as i32;
        let total = (w as usize) * (h as usize) * 4;
        // Skip the frame rather than abort the greeter: a transient shm
        // failure must not strand the user at a dead login screen.
        if total > self.pool.len() {
            if let Err(e) = self.pool.resize(total) {
                log::warn!("wl_shm pool resize to {total} failed: {e}; skipping frame");
                return;
            }
        }
        let (buffer, canvas) =
            match self
                .pool
                .create_buffer(w as i32, h as i32, stride, wl_shm::Format::Argb8888)
            {
                Ok(bc) => bc,
                Err(e) => {
                    log::warn!("wl_shm buffer create failed: {e}; skipping frame");
                    return;
                }
            };

        prompt_ui::render(
            canvas,
            w,
            h,
            &self.outputs,
            &state,
            self.live.theme(),
            &mut self.cache,
            &RenderParams {
                greeting: Some(greeting.as_str()),
                error_message: error_msg,
                fingerprint: self.fingerprint_hint.as_deref().map(|hint| {
                    prompt_ui::FingerprintRender {
                        hint,
                        icon_color_argb: 0xff89b4fa,
                    }
                }),
                show_username: self.show_username,
            },
        );

        let surface = self.window.wl_surface();
        surface.attach(Some(buffer.wl_buffer()), 0, 0);
        surface.damage_buffer(0, 0, w as i32, h as i32);
        surface.commit();
    }
}

impl WindowHandler for App {
    fn request_close(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _window: &Window) {
        log::info!("xdg-toplevel close requested; exiting");
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _window: &Window,
        configure: WindowConfigure,
        _serial: u32,
    ) {
        // Compositors that want us to pick a size send None for both
        // axes; fall back to 1080p for headless test environments.
        let w = configure.new_size.0.map(|n| n.get()).unwrap_or(1920);
        let h = configure.new_size.1.map(|n| n.get()).unwrap_or(1080);
        log::info!("configured at {}x{}", w, h);
        self.size = Some((w, h));
        self.draw();
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
            match self.seat_state.get_keyboard(qh, &seat, None) {
                Ok(kb) => self.keyboard = Some(kb),
                Err(e) => log::warn!("get_keyboard: {}", e),
            }
        }
        if capability == Capability::Pointer && self.pointer.is_none() {
            match self.seat_state.get_pointer(qh, &seat) {
                Ok(p) => self.pointer = Some(p),
                Err(e) => log::warn!("get_pointer: {}", e),
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
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlKeyboard, _: &WlSurface, _: u32) {
    }
    fn press_key(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &WlKeyboard,
        _: u32,
        event: KeyEvent,
    ) {
        if self.power_menu.open {
            match event.keysym {
                Keysym::Escape => {
                    self.power_menu.open = false;
                    self.power_menu.kb_active = false;
                    self.draw();
                    return;
                }
                Keysym::Up => {
                    self.power_menu.kb_active = true;
                    self.power_menu.select_prev();
                    self.draw();
                    return;
                }
                Keysym::Down => {
                    self.power_menu.kb_active = true;
                    self.power_menu.select_next();
                    self.draw();
                    return;
                }
                Keysym::Return | Keysym::KP_Enter => {
                    self.power_menu.kb_active = true;
                    if let Some(action) = self.power_menu.current() {
                        self.power_menu.open = false;
                        self.power_menu.kb_active = false;
                        dispatch_power(action);
                    }
                    self.draw();
                    return;
                }
                Keysym::F12 => {
                    self.power_menu.open = false;
                    self.power_menu.kb_active = false;
                    self.draw();
                    return;
                }
                _ => {}
            }
        }
        if self.show_username && self.username_menu.open {
            match event.keysym {
                Keysym::Escape => {
                    self.username_menu.open = false;
                    self.username_menu.kb_active = false;
                    self.draw();
                    return;
                }
                Keysym::Up => {
                    self.username_menu.kb_active = true;
                    self.username_menu.select_prev();
                    self.draw();
                    return;
                }
                Keysym::Down => {
                    self.username_menu.kb_active = true;
                    self.username_menu.select_next();
                    self.draw();
                    return;
                }
                Keysym::Return | Keysym::KP_Enter | Keysym::space => {
                    self.username_menu.open = false;
                    self.username_menu.kb_active = false;
                    if let Some(name) = self.username_menu.selected_name().map(str::to_string) {
                        if self.username.as_deref() != Some(name.as_str()) {
                            self.rebind_user(name);
                        }
                    }
                    self.draw();
                    return;
                }
                _ => {}
            }
        }
        match event.keysym {
            Keysym::F12 => {
                self.power_menu.open = true;
                self.power_menu.kb_active = true;
                self.power_menu.clamp_selection();
            }
            Keysym::Escape => {
                // Wipe, not just truncate: clear() would leave the
                // secret in the buffer's backing capacity.
                self.password.zeroize();
            }
            Keysym::BackSpace => {
                self.password.pop();
            }
            Keysym::Return | Keysym::KP_Enter => {
                self.submit();
            }
            _ => {
                if let Some(s) = event.utf8.as_deref() {
                    if !s.is_empty() && !s.chars().any(|c| c.is_control()) {
                        self.password.push_str(s);
                    }
                }
            }
        }
        self.draw();
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
        modifiers: Modifiers,
        _: u32,
    ) {
        if self.capslock != modifiers.caps_lock {
            self.capslock = modifiers.caps_lock;
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

impl OutputHandler for App {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.refresh_outputs();
        self.draw();
    }
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.refresh_outputs();
        self.draw();
    }
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
        self.refresh_outputs();
        self.draw();
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

impl PointerHandler for App {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        pointer: &WlPointer,
        events: &[PointerEvent],
    ) {
        let mut dirty = false;
        let mut clicked_at: Option<(f32, f32)> = None;
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
                    let pos = Some((e.position.0 as f32, e.position.1 as f32));
                    self.power_menu.pointer = pos;
                    self.username_menu.pointer = pos;
                    if self.power_menu.open || self.username_menu.open {
                        dirty = true;
                    }
                }
                PointerEventKind::Motion { .. } => {
                    let new_x = e.position.0 as f32;
                    let new_y = e.position.1 as f32;
                    if self.power_menu.open {
                        let old_hit = self.power_menu.pointer.map(|(ox, oy)| {
                            power_ui::hit_test(&self.power_menu, &self.outputs, ox, oy)
                        });
                        let new_hit =
                            power_ui::hit_test(&self.power_menu, &self.outputs, new_x, new_y);
                        if old_hit != Some(new_hit) {
                            dirty = true;
                        }
                    }
                    if self.username_menu.open {
                        let old_hit = self.username_menu.pointer.map(|(ox, oy)| {
                            username_ui::hit_test(&self.username_menu, &self.outputs, ox, oy)
                        });
                        let new_hit =
                            username_ui::hit_test(&self.username_menu, &self.outputs, new_x, new_y);
                        if old_hit != Some(new_hit) {
                            dirty = true;
                        }
                    }
                    self.power_menu.pointer = Some((new_x, new_y));
                    self.username_menu.pointer = Some((new_x, new_y));
                }
                PointerEventKind::Leave { .. } => {
                    let was_open = self.power_menu.open || self.username_menu.open;
                    self.power_menu.pointer = None;
                    self.username_menu.pointer = None;
                    if was_open {
                        dirty = true;
                    }
                }
                PointerEventKind::Press { button: 0x110, .. } => {
                    clicked_at = Some((e.position.0 as f32, e.position.1 as f32));
                }
                _ => {}
            }
        }
        if let Some((cx, cy)) = clicked_at {
            match power_ui::hit_test(&self.power_menu, &self.outputs, cx, cy) {
                PowerHit::ToggleButton => {
                    self.power_menu.open = !self.power_menu.open;
                    self.power_menu.kb_active = false;
                    self.power_menu.clamp_selection();
                    dirty = true;
                }
                PowerHit::Item(action) => {
                    self.power_menu.open = false;
                    self.power_menu.kb_active = false;
                    dispatch_power(action);
                    dirty = true;
                }
                PowerHit::None => {
                    if self.power_menu.open {
                        self.power_menu.open = false;
                        self.power_menu.kb_active = false;
                        dirty = true;
                    } else if self.show_username {
                        match username_ui::hit_test(&self.username_menu, &self.outputs, cx, cy) {
                            UsernameHit::Field => {
                                self.username_menu.open = !self.username_menu.open;
                                self.username_menu.kb_active = false;
                                self.username_menu.clamp_selection();
                                dirty = true;
                            }
                            UsernameHit::Item(idx) => {
                                self.username_menu.selected = idx;
                                self.username_menu.open = false;
                                self.username_menu.kb_active = false;
                                if let Some(name) =
                                    self.username_menu.selected_name().map(str::to_string)
                                {
                                    if self.username.as_deref() != Some(name.as_str()) {
                                        self.rebind_user(name);
                                    }
                                }
                                dirty = true;
                            }
                            UsernameHit::None => {
                                if self.username_menu.open {
                                    self.username_menu.open = false;
                                    dirty = true;
                                }
                            }
                        }
                    }
                }
            }
        }
        if dirty {
            self.draw();
        }
    }
}

fn dispatch_power(action: PowerAction) {
    let verb = match action {
        PowerAction::Suspend => "suspend",
        PowerAction::Hibernate => "hibernate",
        PowerAction::Restart => "reboot",
        PowerAction::Shutdown => "poweroff",
    };
    log::info!("dispatch_power: {:?} (systemctl {})", action, verb);
    std::thread::spawn(move || {
        let out = std::process::Command::new("systemctl").arg(verb).output();
        match out {
            Ok(o) if o.status.success() => {}
            Ok(o) => log::warn!(
                "systemctl {}: status={} stderr={:?}",
                verb,
                o.status,
                String::from_utf8_lossy(&o.stderr).trim()
            ),
            Err(e) => log::warn!("systemctl {} spawn failed: {e}", verb),
        }
    });
}

delegate_compositor!(App);
delegate_keyboard!(App);
delegate_output!(App);
delegate_pointer!(App);
delegate_registry!(App);
delegate_seat!(App);
delegate_shm!(App);
delegate_xdg_shell!(App);
delegate_xdg_window!(App);
