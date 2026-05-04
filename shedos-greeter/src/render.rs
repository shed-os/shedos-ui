//! Wayland fullscreen layer-shell surface, wallpaper blit, hyprlock-
//! style text chrome, and password input handling.
//!
//! Binds compositor + wlr-layer-shell + wl_shm + wl_seat; creates a
//! `Layer::Top` surface anchored to all four edges with `exclusive_zone
//! = -1`; on configure scales the wallpaper with Lanczos3 and blits to
//! a wl_shm Argb8888 buffer; layers clock + date + greeting + a
//! rounded-square password input + branding on top; routes keyboard
//! events into a password buffer that submits on Enter.
//!
//! The greetd IPC submission lands in the next commit; for now Enter
//! just logs and clears.

use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use image::imageops::FilterType;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_output, delegate_registry, delegate_seat,
    delegate_shm, delegate_xdg_shell, delegate_xdg_window,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers},
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
    protocol::{wl_keyboard::WlKeyboard, wl_output, wl_seat::WlSeat, wl_shm, wl_surface::WlSurface},
    Connection, QueueHandle,
};

use crate::greetd;
use crate::text::{FontFace, JBM_BOLD_CANDIDATES, JBM_REGULAR_CANDIDATES};
use crate::user;

// Catppuccin Mocha tokens used by the greeter chrome. Hardcoded for
// commit 6; the theme reconciler (commit 7) will feed these in.
const TEXT: (u8, u8, u8) = (0xcd, 0xd6, 0xf4);
const BLUE: (u8, u8, u8) = (0x89, 0xb4, 0xfa);
const BASE: (u8, u8, u8) = (0x1e, 0x1e, 0x2e);
const RED: (u8, u8, u8) = (0xf3, 0x8b, 0xa8);

const ERROR_HOLD: Duration = Duration::from_secs(2);
const ERROR_TEXT: &str = "Authentication Failed";

const CLOCK_PX: f32 = 120.0;
const DATE_PX: f32 = 24.0;
const GREET_PX: f32 = 32.0;
const BRAND_PX: f32 = 18.0;
const INPUT_FONT_PX: f32 = 18.0;

// Input box geometry (matches hyprlock's input box: 300×50, 2 px outline,
// 10 px corner radius). Corner rounding is applied as a soft mask.
const INPUT_W: u32 = 300;
const INPUT_H: u32 = 50;
const INPUT_RADIUS: u32 = 10;
const INPUT_BORDER: u32 = 2;

pub fn run(wallpaper_path: &Path) -> Result<()> {
    log::info!("loading wallpaper from {}", wallpaper_path.display());
    let wallpaper = image::open(wallpaper_path)
        .with_context(|| format!("opening wallpaper {}", wallpaper_path.display()))?;
    log::info!("wallpaper decoded: {}x{}", wallpaper.width(), wallpaper.height());

    let conn = Connection::connect_to_env()
        .context("connect to Wayland display (is WAYLAND_DISPLAY set?)")?;
    let (globals, mut event_queue) =
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

    // xdg-shell instead of wlr-layer-shell because the kiosk hosting
    // compositors we target (cage) advertise xdg_wm_base but not
    // zwlr_layer_shell_v1. cage forces single-window-fullscreen at the
    // toplevel level, so we get the same "fullscreen greeter" UX as
    // layer-shell + Anchor::all without the protocol mismatch.
    let surface = compositor.create_surface(&qh);
    let window = xdg_shell.create_window(surface, WindowDecorations::ServerDefault, &qh);
    window.set_title("ShedOS Greeter".to_string());
    window.set_app_id("shedos-greeter".to_string());
    // Hint at fullscreen so the compositor sends a configure with the
    // output's full size; cage ignores this (always fullscreens its only
    // window), and Hyprland-as-greeter can pair it with a windowrule.
    window.set_fullscreen(None);
    window.commit();

    let pool = SlotPool::new(4, &shm).context("create wl_shm slot pool")?;
    let regular = FontFace::load(JBM_REGULAR_CANDIDATES)?;
    let bold = FontFace::load(JBM_BOLD_CANDIDATES)?;
    let username = user::resolve();

    let mut app = App {
        registry_state,
        output_state,
        seat_state,
        shm,
        window,
        pool,
        wallpaper,
        wallpaper_cache: None,
        regular,
        bold,
        keyboard: None,
        size: None,
        username,
        password: String::new(),
        error_text: String::new(),
        error_until: None,
        exit: false,
    };

    while !app.exit {
        event_queue
            .blocking_dispatch(&mut app)
            .context("Wayland event dispatch")?;
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
    wallpaper: image::DynamicImage,
    /// Lanczos-scaled wallpaper, pre-converted to wl_shm BGRA byte
    /// order. Re-computed only when the surface size changes; on each
    /// keystroke draw() just memcpy's this into the buffer. Without
    /// this cache, every keystroke triggers a multi-hundred-ms Lanczos
    /// resize of the source image and the input feels laggy.
    wallpaper_cache: Option<(u32, u32, Vec<u8>)>,
    regular: FontFace,
    bold: FontFace,
    keyboard: Option<WlKeyboard>,
    size: Option<(u32, u32)>,
    username: Option<String>,
    password: String,
    /// Error message to render below the input box during the
    /// `error_until` hold window. Populated by `submit()` with the
    /// actual greetd error (truncated) so PAM-side rejection reasons
    /// surface without having to grep the journal.
    error_text: String,
    error_until: Option<Instant>,
    exit: bool,
}

impl App {
    fn submit(&mut self) {
        let Some(username) = self.username.clone() else {
            log::warn!("submit: no username configured (set /etc/shedos/login-user)");
            self.password.clear();
            return;
        };
        let password = std::mem::take(&mut self.password);
        let cmd = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "exec /usr/bin/uwsm start -g -1 -e -D Hyprland hyprland.desktop \
             > /dev/null 2>&1".to_string(),
        ];
        match greetd::Auth::connect().and_then(|mut a| a.login(&username, &password, cmd)) {
            Ok(()) => {
                log::info!("auth + start_session OK; greeter exiting for {}", username);
                self.exit = true;
            }
            Err(e) => {
                log::warn!("login failed: {:#}", e);
                let msg = format!("{:#}", e);
                self.error_text = if msg.is_empty() {
                    "Authentication Failed".to_string()
                } else {
                    msg
                };
                self.error_until = Some(Instant::now() + ERROR_HOLD);
            }
        }
    }


    fn draw(&mut self) {
        let Some((w, h)) = self.size else { return };
        if w == 0 || h == 0 {
            return;
        }

        // Resolve transient UI state before we acquire the wl_shm
        // buffer borrow (which holds &mut self.pool until commit).
        let error = match self.error_until {
            Some(ts) if Instant::now() < ts => true,
            Some(_) => {
                self.error_until = None;
                false
            }
            None => false,
        };

        let stride = (w * 4) as i32;
        let total = (w as usize) * (h as usize) * 4;
        if total > self.pool.len() {
            self.pool.resize(total).expect("resize wl_shm pool");
        }
        let (buffer, canvas) = self
            .pool
            .create_buffer(w as i32, h as i32, stride, wl_shm::Format::Argb8888)
            .expect("create wl_shm buffer");

        // Wallpaper.
        // Wallpaper: pre-scaled BGRA bytes cached. Lanczos3 only on
        // size change (typically once, at first configure). Subsequent
        // redraws (one per keystroke) memcpy the cached buffer.
        let cache_hit = self
            .wallpaper_cache
            .as_ref()
            .is_some_and(|(cw, ch, _)| *cw == w && *ch == h);
        if !cache_hit {
            log::info!("rebuilding wallpaper cache for {}x{}", w, h);
            let scaled = self
                .wallpaper
                .resize_to_fill(w, h, FilterType::Lanczos3)
                .to_rgba8();
            let mut bgra = Vec::with_capacity((w as usize) * (h as usize) * 4);
            for px in scaled.pixels() {
                bgra.push(px[2]);
                bgra.push(px[1]);
                bgra.push(px[0]);
                bgra.push(0xff);
            }
            self.wallpaper_cache = Some((w, h, bgra));
        }
        let cached_bgra = &self.wallpaper_cache.as_ref().expect("just populated").2;
        canvas[..cached_bgra.len()].copy_from_slice(cached_bgra);

        let now = chrono::Local::now();
        let clock = now.format("%H:%M").to_string();
        let date = now.format("%A, %B %-d").to_string();
        let brand = "ShedOS";

        // Clock at ~30% from top.
        let clock_w = self.regular.measure_width(&clock, CLOCK_PX);
        let clock_x = (w as i32 - clock_w) / 2;
        let clock_y = (h as f32 * 0.30) as i32;
        self.regular
            .render(&clock, CLOCK_PX, clock_x, clock_y, TEXT, 0xff, canvas, w, h);

        // Date just under the clock.
        let date_w = self.regular.measure_width(&date, DATE_PX);
        let date_x = (w as i32 - date_w) / 2;
        let date_y = clock_y + (CLOCK_PX as i32 / 4);
        self.regular
            .render(&date, DATE_PX, date_x, date_y, TEXT, 0xcc, canvas, w, h);

        // Password input box centered ~58% from top.
        let border_color = if error { RED } else { BLUE };
        let box_x = (w as i32 - INPUT_W as i32) / 2;
        let box_y = (h as f32 * 0.58) as i32;
        draw_rounded_box(
            canvas,
            w,
            h,
            box_x,
            box_y,
            INPUT_W,
            INPUT_H,
            INPUT_RADIUS,
            INPUT_BORDER,
            BASE,
            0xe6,
            border_color,
            0xee,
        );
        // Render password as bullet glyphs.
        let dots: String = "●".repeat(self.password.chars().count());
        if !dots.is_empty() {
            let dots_w = self.regular.measure_width(&dots, INPUT_FONT_PX);
            let dots_x = box_x + ((INPUT_W as i32 - dots_w) / 2);
            // Approx baseline placement inside the input box.
            let dots_y = box_y + (INPUT_H as i32 * 2 / 3);
            self.regular.render(
                &dots,
                INPUT_FONT_PX,
                dots_x,
                dots_y,
                TEXT,
                0xff,
                canvas,
                w,
                h,
            );
        }

        // Below the input: either "Authentication Failed" (during the
        // error hold) or the "Hi, $username" greeting.
        let line_y = box_y + INPUT_H as i32 + 56;
        if error {
            let msg = if self.error_text.is_empty() {
                ERROR_TEXT
            } else {
                self.error_text.as_str()
            };
            let err_w = self.regular.measure_width(msg, GREET_PX);
            let err_x = (w as i32 - err_w) / 2;
            self.regular
                .render(msg, GREET_PX, err_x, line_y, RED, 0xff, canvas, w, h);
        } else {
            let greet = match &self.username {
                Some(name) => format!("Hi, {}", name),
                None => "Hi".to_string(),
            };
            let greet_w = self.regular.measure_width(&greet, GREET_PX);
            let greet_x = (w as i32 - greet_w) / 2;
            self.regular
                .render(&greet, GREET_PX, greet_x, line_y, BLUE, 0xff, canvas, w, h);
        }

        // Branding bottom center.
        let brand_w = self.bold.measure_width(brand, BRAND_PX);
        let brand_x = (w as i32 - brand_w) / 2;
        let brand_y = (h as f32 * 0.93) as i32;
        self.bold
            .render(brand, BRAND_PX, brand_x, brand_y, BLUE, 0x99, canvas, w, h);

        let surface = self.window.wl_surface();
        surface.attach(Some(buffer.wl_buffer()), 0, 0);
        surface.damage_buffer(0, 0, w as i32, h as i32);
        surface.commit();
    }
}

/// Alpha-blend a single (B, G, R, A) pixel into the canvas at (x, y).
#[inline]
fn blend_pixel(canvas: &mut [u8], cw: u32, x: i32, y: i32, color: (u8, u8, u8), alpha: u8) {
    if x < 0 || y < 0 || alpha == 0 {
        return;
    }
    if (x as u32) >= cw {
        return;
    }
    let dst = ((y as u32) * cw * 4 + (x as u32) * 4) as usize;
    if dst + 3 >= canvas.len() {
        return;
    }
    if alpha == 255 {
        canvas[dst] = color.2;
        canvas[dst + 1] = color.1;
        canvas[dst + 2] = color.0;
        canvas[dst + 3] = 0xff;
        return;
    }
    let av = alpha as u32;
    let inv = 255 - av;
    canvas[dst] = ((color.2 as u32 * av + canvas[dst] as u32 * inv) / 255) as u8;
    canvas[dst + 1] = ((color.1 as u32 * av + canvas[dst + 1] as u32 * inv) / 255) as u8;
    canvas[dst + 2] = ((color.0 as u32 * av + canvas[dst + 2] as u32 * inv) / 255) as u8;
    canvas[dst + 3] = 0xff;
}

/// Coverage of a pixel (`px`, `py`) by a rounded-rect of given geometry,
/// returning 0.0..=1.0. Corners are quarter-circles of radius `r`. Anti-
/// aliased via the squared-distance comparison against `r-0.5` and `r+0.5`.
fn rounded_rect_coverage(
    px: f32,
    py: f32,
    bx: f32,
    by: f32,
    bw: f32,
    bh: f32,
    r: f32,
) -> f32 {
    // Quick reject outside the bbox.
    if px < bx || py < by || px > bx + bw || py > by + bh {
        return 0.0;
    }
    // Identify which corner (if any) the pixel is inside the rounding region for.
    let cx = if px < bx + r {
        bx + r
    } else if px > bx + bw - r {
        bx + bw - r
    } else {
        return 1.0; // straight edge interior
    };
    let cy = if py < by + r {
        by + r
    } else if py > by + bh - r {
        by + bh - r
    } else {
        return 1.0;
    };
    let dx = px - cx;
    let dy = py - cy;
    let d = (dx * dx + dy * dy).sqrt();
    if d <= r - 0.5 {
        1.0
    } else if d >= r + 0.5 {
        0.0
    } else {
        // Soft 1-pixel anti-alias band.
        (r + 0.5 - d).clamp(0.0, 1.0)
    }
}

/// Fill + stroke a rounded rectangle at (`x`,`y`) with size `w`×`h`,
/// corner radius `radius`, border thickness `thick`. Fill color blends
/// with whatever's already on the canvas at `fill_alpha`; stroke
/// blends at `border_alpha`.
#[allow(clippy::too_many_arguments)]
fn draw_rounded_box(
    canvas: &mut [u8],
    cw: u32,
    ch: u32,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    radius: u32,
    thick: u32,
    fill: (u8, u8, u8),
    fill_alpha: u8,
    border: (u8, u8, u8),
    border_alpha: u8,
) {
    let bx = x as f32;
    let by = y as f32;
    let bw = w as f32;
    let bh = h as f32;
    let r = radius as f32;
    let t = thick as f32;
    for dy in 0..h as i32 {
        for dx in 0..w as i32 {
            let px = (x + dx) as f32 + 0.5;
            let py = (y + dy) as f32 + 0.5;
            let outer = rounded_rect_coverage(px, py, bx, by, bw, bh, r);
            if outer <= 0.0 {
                continue;
            }
            // Inner shape (the fill) is the outer shape eroded by `t`.
            let inner = rounded_rect_coverage(
                px,
                py,
                bx + t,
                by + t,
                bw - 2.0 * t,
                bh - 2.0 * t,
                (r - t).max(0.0),
            );
            let cx = x + dx;
            let cy = y + dy;
            // First the fill.
            let fa = (fill_alpha as f32 * inner) as u8;
            if fa > 0 {
                blend_pixel(canvas, cw, cx, cy, fill, fa);
            }
            // Then the stroke (outer minus inner).
            let stroke = (outer - inner).max(0.0);
            let ba = (border_alpha as f32 * stroke) as u8;
            if ba > 0 {
                blend_pixel(canvas, cw, cx, cy, border, ba);
            }
            // (`ch` boundary is enforced by blend_pixel via canvas.len bounds.)
            let _ = ch;
        }
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
        // Compositors that want us to choose a size send None for both
        // axes — fall back to 1080p so a misconfigured headless test
        // still draws something.
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
        match event.keysym {
            Keysym::Escape => {
                log::info!("Escape pressed; exiting");
                self.exit = true;
                return;
            }
            Keysym::BackSpace => {
                self.password.pop();
            }
            Keysym::Return | Keysym::KP_Enter => {
                self.submit();
            }
            _ => {
                // Append printable utf8; reject control chars.
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
        _: Modifiers,
        _: u32,
    ) {
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
delegate_output!(App);
delegate_registry!(App);
delegate_seat!(App);
delegate_shm!(App);
delegate_xdg_shell!(App);
delegate_xdg_window!(App);
