//! Wayland fullscreen layer-shell surface + wallpaper blit.
//!
//! Binds compositor + wlr-layer-shell + wl_shm; creates a `Layer::Top`
//! surface anchored to all four edges with `exclusive_zone = -1` (covers
//! the screen, no panel reservation); on the compositor's first
//! `configure`, scales the wallpaper image to the surface size with
//! Lanczos3 and blits into a `wl_shm` ARGB8888 buffer.
//!
//! Subsequent commits layer text + the password input on top of this
//! base by keeping the wallpaper pre-rendered and only re-blitting the
//! interactive region per frame.

use std::path::Path;

use anyhow::{Context, Result};
use image::imageops::FilterType;
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
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
    protocol::{wl_output, wl_shm, wl_surface::WlSurface},
    Connection, QueueHandle,
};

use crate::text::{FontFace, JBM_BOLD_CANDIDATES, JBM_REGULAR_CANDIDATES};

// Catppuccin Mocha tokens used by the greeter chrome. Hardcoded for
// commit 3; the theme reconciler (commit 7) will feed these in.
const TEXT: (u8, u8, u8) = (0xcd, 0xd6, 0xf4);
const BLUE: (u8, u8, u8) = (0x89, 0xb4, 0xfa);

const CLOCK_PX: f32 = 120.0;
const DATE_PX: f32 = 24.0;
const BRAND_PX: f32 = 18.0;

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
    let compositor =
        CompositorState::bind(&globals, &qh).context("wl_compositor not advertised")?;
    let layer_shell = LayerShell::bind(&globals, &qh)
        .context("zwlr_layer_shell_v1 not advertised by compositor")?;
    let shm = Shm::bind(&globals, &qh).context("wl_shm not advertised")?;

    let surface = compositor.create_surface(&qh);
    let layer = layer_shell.create_layer_surface(
        &qh,
        surface,
        Layer::Top,
        Some("shedos-greeter"),
        None,
    );
    layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
    layer.set_exclusive_zone(-1);
    layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
    layer.commit();

    // Provisional pool; resized on the first configure once we know the surface size.
    let pool = SlotPool::new(4, &shm).context("create wl_shm slot pool")?;

    let regular = FontFace::load(JBM_REGULAR_CANDIDATES)?;
    let bold = FontFace::load(JBM_BOLD_CANDIDATES)?;

    let mut app = App {
        registry_state,
        output_state,
        shm,
        layer,
        pool,
        wallpaper,
        regular,
        bold,
        size: None,
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
    shm: Shm,
    layer: LayerSurface,
    pool: SlotPool,
    wallpaper: image::DynamicImage,
    regular: FontFace,
    bold: FontFace,
    size: Option<(u32, u32)>,
    exit: bool,
}

impl App {
    fn draw(&mut self) {
        let Some((w, h)) = self.size else { return };
        if w == 0 || h == 0 {
            return;
        }

        let stride = (w * 4) as i32;
        let total = (w as usize) * (h as usize) * 4;
        if total > self.pool.len() {
            self.pool.resize(total).expect("resize wl_shm pool");
        }

        let (buffer, canvas) = self
            .pool
            .create_buffer(w as i32, h as i32, stride, wl_shm::Format::Argb8888)
            .expect("create wl_shm buffer");

        let scaled = self
            .wallpaper
            .resize_to_fill(w, h, FilterType::Lanczos3)
            .to_rgba8();
        for (i, px) in scaled.pixels().enumerate() {
            let dst = i * 4;
            // wl_shm Argb8888 in little-endian = BGRA byte order on disk.
            canvas[dst] = px[2];
            canvas[dst + 1] = px[1];
            canvas[dst + 2] = px[0];
            canvas[dst + 3] = 0xff;
        }

        // Layer hyprlock-style chrome on top of the wallpaper.
        // Vertical positions are percentage-based so the layout adapts
        // to any resolution; font sizes are absolute px to mirror
        // hyprlock's exact sizing.
        let now = chrono::Local::now();
        let clock = now.format("%H:%M").to_string();
        let date = now.format("%A, %B %-d").to_string();
        let brand = "ShedOS";

        let clock_w = self.regular.measure_width(&clock, CLOCK_PX);
        let clock_x = (w as i32 - clock_w) / 2;
        let clock_y = (h as f32 * 0.30) as i32;
        self.regular
            .render(&clock, CLOCK_PX, clock_x, clock_y, TEXT, 0xff, canvas, w, h);

        let date_w = self.regular.measure_width(&date, DATE_PX);
        let date_x = (w as i32 - date_w) / 2;
        let date_y = clock_y + (CLOCK_PX as i32 / 4);
        self.regular
            .render(&date, DATE_PX, date_x, date_y, TEXT, 0xcc, canvas, w, h);

        let brand_w = self.bold.measure_width(brand, BRAND_PX);
        let brand_x = (w as i32 - brand_w) / 2;
        let brand_y = (h as f32 * 0.93) as i32;
        self.bold
            .render(brand, BRAND_PX, brand_x, brand_y, BLUE, 0x99, canvas, w, h);

        let surface = self.layer.wl_surface();
        surface.attach(Some(buffer.wl_buffer()), 0, 0);
        surface.damage_buffer(0, 0, w as i32, h as i32);
        surface.commit();
    }
}

impl LayerShellHandler for App {
    fn closed(&mut self, _conn: &Connection, _qh: &QueueHandle<Self>, _layer: &LayerSurface) {
        log::info!("layer surface closed; exiting");
        self.exit = true;
    }

    fn configure(
        &mut self,
        _conn: &Connection,
        _qh: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        let (mut w, mut h) = configure.new_size;
        // Compositors that want us to pick a size send (0, 0). Use a
        // 1080p fallback so a misconfigured headless test still draws.
        if w == 0 {
            w = 1920;
        }
        if h == 0 {
            h = 1080;
        }
        log::info!("configured at {}x{}", w, h);
        self.size = Some((w, h));
        self.draw();
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
    registry_handlers![OutputState];
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }
}

delegate_compositor!(App);
delegate_layer!(App);
delegate_output!(App);
delegate_registry!(App);
delegate_shm!(App);
