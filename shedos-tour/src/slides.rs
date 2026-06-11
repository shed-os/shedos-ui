use std::time::Instant;

use shedos_prompt_ui::primitives::{blend_pixel, draw_rounded_box};
use shedos_prompt_ui::text::FontFace;
use shedos_prompt_ui::wordmark::Wordmark;

// Colors come from the live theme (greeter.toml via prompt-ui's
// Theme); load_or_default falls back to Catppuccin Mocha.
pub struct Palette {
    pub text: (u8, u8, u8),
    pub muted: (u8, u8, u8),
    pub card_bg: (u8, u8, u8),
    pub card_border: (u8, u8, u8),
    pub cap_bg: (u8, u8, u8),
    pub backdrop_rgb: (u8, u8, u8),
    pub accent: (u8, u8, u8),
}

fn rgb(v: u32) -> (u8, u8, u8) {
    (((v >> 16) & 0xff) as u8, ((v >> 8) & 0xff) as u8, (v & 0xff) as u8)
}

pub fn palette() -> &'static Palette {
    static PALETTE: std::sync::OnceLock<Palette> = std::sync::OnceLock::new();
    PALETTE.get_or_init(|| {
        let t = shedos_prompt_ui::Theme::load_or_default();
        Palette {
            text: rgb(t.text),
            muted: rgb(t.overlay1),
            card_bg: rgb(t.mantle),
            card_border: rgb(t.surface2),
            cap_bg: rgb(t.surface0),
            backdrop_rgb: rgb(t.base),
            accent: rgb(t.accent),
        }
    })
}

const BACKDROP_ALPHA: u8 = 0xb3; // 0.7

const CARD_W: u32 = 700;
const CARD_H: u32 = 480;
const CARD_RADIUS: u32 = 16;
const CARD_PAD_X: i32 = 44;

const TITLE_PX: f32 = 19.0;
const BODY_PX: f32 = 13.0;
const CAP_PX: f32 = 12.0;
const HINT_PX: f32 = 11.5;
const LINE_GAP: i32 = 26;
const DOT_R: i32 = 4;
const DOT_GAP: i32 = 18;

const OPEN_MS: u32 = 150;
const SWAP_MS: u32 = 130;
const DISMISS_MS: u32 = 120;

/// One body row: optional key caps, description text.
pub struct Row {
    pub caps: &'static [&'static str],
    pub text: &'static str,
}

/// Per-slide drawn illustration.
#[derive(Clone, Copy, PartialEq)]
pub enum Art {
    Wordmark,
    Tiling,
    Workspaces,
    Shield,
    None,
}

pub struct Slide {
    pub art: Art,
    pub title: &'static str,
    pub intro: &'static str,
    pub rows: &'static [Row],
    pub footer: &'static str,
}

// Every cap and gesture below is verified against the shipped
// keybindings.lua / gestures.lua — change those, change this.
pub const SLIDES: &[Slide] = &[
    Slide {
        art: Art::Wordmark,
        title: "Your machine is ready",
        intro: "Everything is installed and configured — this tour is the\nshort version of what makes ShedOS different. Six slides,\nthree minutes, and the keyboard already works:",
        rows: &[
            Row { caps: &["→"], text: "next slide (or click anywhere)" },
            Row { caps: &["Esc"], text: "skip — replay anytime with: shedman tour" },
        ],
        footer: "",
    },
    Slide {
        art: Art::None,
        title: "Open things",
        intro: "Hold the Super key (Windows/Cmd) — it drives the desktop.",
        rows: &[
            Row { caps: &["Super", "Enter"], text: "terminal" },
            Row { caps: &["Super", "D"], text: "app launcher — type a few letters, Enter" },
            Row { caps: &["Super", "B"], text: "browser" },
            Row { caps: &["Super", "E"], text: "code editor" },
            Row { caps: &["Super", "N"], text: "file manager" },
            Row { caps: &["Super", "C"], text: "clipboard history" },
        ],
        footer: "Apps open tiled — no overlapping clutter to manage.",
    },
    Slide {
        art: Art::Tiling,
        title: "Windows tile themselves",
        intro: "New windows split the space automatically. You just steer:",
        rows: &[
            Row { caps: &["Super", "Q"], text: "close the focused window" },
            Row { caps: &["Super", "←→↑↓"], text: "move focus between windows" },
            Row { caps: &["Super", "Shift", "←→↑↓"], text: "move the window itself" },
            Row { caps: &["Super", "F"], text: "fullscreen — again to undo" },
            Row { caps: &["Super", "V"], text: "float a window free of the grid" },
        ],
        footer: "Dragging with Super + left mouse held works too.",
    },
    Slide {
        art: Art::Workspaces,
        title: "Workspaces hold your contexts",
        intro: "Code on 1, browser on 2, chat on 3 — switch, don't shuffle.",
        rows: &[
            Row { caps: &["Super", "1..9"], text: "jump to a workspace" },
            Row { caps: &["Super", "Shift", "1..9"], text: "send the window there" },
            Row { caps: &["Super", "Tab"], text: "overview of every workspace" },
            Row { caps: &[], text: "Touchpad: swipe sideways with 3 fingers to switch," },
            Row { caps: &[], text: "swipe up with 3 fingers for the overview, and swipe" },
            Row { caps: &[], text: "sideways with 4 fingers to carry the window along." },
        ],
        footer: "",
    },
    Slide {
        art: Art::Shield,
        title: "You are safe to experiment",
        intro: "ShedOS assumes things will break — and makes it not matter:",
        rows: &[
            Row { caps: &["shedman update"], text: "upgrades take a snapshot first" },
            Row { caps: &["shedman rollback"], text: "undoes any of them, one command" },
            Row { caps: &[], text: "Three failed boots in a row? The system boots its" },
            Row { caps: &[], text: "last good snapshot by itself and tells you." },
            Row { caps: &[], text: "Configs you edit are never overwritten by updates." },
        ],
        footer: "shedman doctor answers \"is everything as it should be?\"",
    },
    Slide {
        art: Art::None,
        title: "That's the foundation",
        intro: "There is more — screenshots, screen recording, theming,\nthe scratchpad, night light — and one key lists all of it:",
        rows: &[
            Row { caps: &["Super", "Alt", "K"], text: "searchable list of every key and gesture" },
        ],
        footer: "Enter opens that list now — Esc starts you working.",
    },
];

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Open,
    Swap,
    Dismiss,
}

pub struct TourState {
    pub slide: usize,
    pub open_keybindings: bool,
    phase: Phase,
    phase_at: Instant,
}

impl TourState {
    pub fn new() -> Self {
        Self {
            slide: 0,
            open_keybindings: false,
            phase: Phase::Open,
            phase_at: Instant::now(),
        }
    }

    pub fn on_last_slide(&self) -> bool {
        self.slide + 1 >= SLIDES.len()
    }

    pub fn next(&mut self) {
        if self.slide + 1 < SLIDES.len() {
            self.slide += 1;
            self.phase = Phase::Swap;
            self.phase_at = Instant::now();
        }
    }

    pub fn prev(&mut self) {
        if self.slide > 0 {
            self.slide -= 1;
            self.phase = Phase::Swap;
            self.phase_at = Instant::now();
        }
    }

    pub fn enter_dismiss(&mut self) {
        self.phase = Phase::Dismiss;
        self.phase_at = Instant::now();
    }

    pub fn is_dismissing(&self) -> bool {
        self.phase == Phase::Dismiss
    }

    pub fn dismiss_done(&self, now: Instant) -> bool {
        self.phase == Phase::Dismiss
            && now.duration_since(self.phase_at).as_millis() as u32 >= DISMISS_MS
    }

    pub fn is_settled(&self, now: Instant) -> bool {
        let elapsed = now.duration_since(self.phase_at).as_millis() as u32;
        match self.phase {
            Phase::Open => elapsed >= OPEN_MS,
            Phase::Swap => elapsed >= SWAP_MS,
            Phase::Dismiss => false,
        }
    }

    /// 0.0–1.0 content opacity for the current phase.
    fn content_alpha(&self, now: Instant) -> f32 {
        let elapsed = now.duration_since(self.phase_at).as_millis() as f32;
        match self.phase {
            Phase::Open => (elapsed / OPEN_MS as f32).min(1.0),
            Phase::Swap => (elapsed / SWAP_MS as f32).min(1.0),
            Phase::Dismiss => 1.0 - (elapsed / DISMISS_MS as f32).min(1.0),
        }
    }
}

fn fill_backdrop(canvas: &mut [u8], w: u32, h: u32, alpha: u8) {
    let p = palette();
    let (r, g, b) = p.backdrop_rgb;
    let a = alpha as u32;
    let pre = |c: u8| ((c as u32 * a) / 255) as u8;
    let (rr, gg, bb) = (pre(r), pre(g), pre(b));
    for px in canvas.chunks_exact_mut(4).take((w * h) as usize) {
        px[0] = bb;
        px[1] = gg;
        px[2] = rr;
        px[3] = alpha;
    }
}

fn fill_circle(canvas: &mut [u8], w: u32, cx: i32, cy: i32, r: i32, color: (u8, u8, u8), a: u8) {
    for yy in -r..=r {
        for xx in -r..=r {
            if xx * xx + yy * yy <= r * r {
                blend_pixel(canvas, w, cx + xx, cy + yy, color, a);
            }
        }
    }
}

/// Slide-top illustrations, drawn with the same primitives as the
/// card so they recolor with the theme.
#[allow(clippy::too_many_arguments)]
fn draw_art(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    art: Art,
    wm: Option<&mut Wordmark>,
    cx: i32,
    top: i32,
    a8: u8,
) {
    let p = palette();
    match art {
        Art::Wordmark => {
            if let Some(wm) = wm {
                wm.blit_centered(canvas, w, h, cx, top + 34, 240);
            }
        }
        Art::Tiling => {
            // A miniature tiled desktop: master window + two stack
            // windows; the focused one carries the accent border.
            let aw = 180;
            let x0 = cx - aw / 2;
            draw_rounded_box(canvas, w, h, x0, top, 104, 68, 6, 2, p.cap_bg, a8, p.accent, a8);
            draw_rounded_box(canvas, w, h, x0 + 110, top, 70, 31, 5, 1, p.cap_bg, a8, p.card_border, a8);
            draw_rounded_box(canvas, w, h, x0 + 110, top + 37, 70, 31, 5, 1, p.cap_bg, a8, p.card_border, a8);
        }
        Art::Workspaces => {
            // Three workspace cards, middle active, swipe arrow under.
            let cw = 52;
            let gap = 14;
            let x0 = cx - (3 * cw + 2 * gap) / 2;
            for i in 0..3 {
                let x = x0 + i * (cw + gap);
                let border = if i == 1 { p.accent } else { p.card_border };
                draw_rounded_box(canvas, w, h, x, top, cw as u32, 38, 5, 1, p.cap_bg, a8, border, a8);
            }
            // Swipe arrow: tapering dot trail, then a chevron head.
            let ay = top + 56;
            for (off, r) in [(0i32, 1i32), (14, 2), (28, 3)] {
                fill_circle(canvas, w, cx - 28 + off, ay, r, p.accent, a8);
            }
            for t in 0..10 {
                blend_pixel(canvas, w, cx + 14 + t, ay - t, p.accent, a8);
                blend_pixel(canvas, w, cx + 14 + t, ay + t, p.accent, a8);
                blend_pixel(canvas, w, cx + 13 + t, ay - t, p.accent, a8);
                blend_pixel(canvas, w, cx + 13 + t, ay + t, p.accent, a8);
            }
        }
        Art::Shield => {
            // Ring + check: snapshot taken, all good.
            fill_circle(canvas, w, cx, top + 30, 26, p.cap_bg, a8);
            for ring in 24..26 {
                for deg in 0..360 {
                    let rad = (deg as f32).to_radians();
                    let x = cx + (rad.cos() * ring as f32) as i32;
                    let y = top + 30 + (rad.sin() * ring as f32) as i32;
                    blend_pixel(canvas, w, x, y, p.accent, a8);
                }
            }
            for t in 0..8 {
                blend_pixel(canvas, w, cx - 10 + t, top + 30 + t, p.accent, a8);
                blend_pixel(canvas, w, cx - 10 + t, top + 31 + t, p.accent, a8);
            }
            for t in 0..16 {
                blend_pixel(canvas, w, cx - 2 + t, top + 37 - t, p.accent, a8);
                blend_pixel(canvas, w, cx - 2 + t, top + 38 - t, p.accent, a8);
            }
        }
        Art::None => {}
    }
}

#[allow(clippy::too_many_arguments)]
pub fn paint(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    state: &TourState,
    regular: &FontFace,
    bold: &FontFace,
    wordmark: Option<&mut Wordmark>,
    now: Instant,
) {
    let p = palette();
    let alpha = state.content_alpha(now);
    let backdrop = (BACKDROP_ALPHA as f32 * alpha) as u8;
    fill_backdrop(canvas, w, h, backdrop);

    let a8 = (255.0 * alpha) as u8;
    if a8 == 0 {
        return;
    }

    let card_x = (w as i32 - CARD_W as i32) / 2;
    let card_y = (h as i32 - CARD_H as i32) / 2;
    draw_rounded_box(
        canvas, w, h, card_x, card_y, CARD_W, CARD_H, CARD_RADIUS, 1,
        p.card_bg, a8, p.card_border, a8,
    );

    let slide = &SLIDES[state.slide];
    let cx = w as i32 / 2;
    let mut y = card_y + 36;

    if slide.art != Art::None {
        draw_art(canvas, w, h, slide.art, wordmark, cx, y, a8);
        y += 92;
    } else {
        y += 24;
    }

    let tw = bold.measure_width(slide.title, TITLE_PX);
    bold.render(slide.title, TITLE_PX, cx - tw / 2, y, p.text, a8, canvas, w, h);
    y += 34;

    for line in slide.intro.split('\n') {
        let lw = regular.measure_width(line, BODY_PX);
        regular.render(line, BODY_PX, cx - lw / 2, y, p.muted, a8, canvas, w, h);
        y += 20;
    }
    y += 14;

    // Body rows: caps right-aligned to a shared column, text after it.
    let has_caps = slide.rows.iter().any(|r| !r.caps.is_empty());
    let cap_col = card_x + CARD_PAD_X + 250;
    for row in slide.rows {
        if row.caps.is_empty() {
            if has_caps {
                regular.render(
                    row.text, BODY_PX, card_x + CARD_PAD_X + 24, y, p.text, a8, canvas, w, h,
                );
            } else {
                let tw = regular.measure_width(row.text, BODY_PX);
                regular.render(row.text, BODY_PX, cx - tw / 2, y, p.text, a8, canvas, w, h);
            }
        } else {
            let mut total = 0;
            for c in row.caps {
                total += bold.measure_width(c, CAP_PX) + 14 + 6;
            }
            let mut x = cap_col - total;
            for c in row.caps {
                let cw = bold.measure_width(c, CAP_PX);
                draw_rounded_box(
                    canvas, w, h, x, y - 14, (cw + 14) as u32, 20, 5, 1,
                    p.cap_bg, a8, p.card_border, a8,
                );
                bold.render(c, CAP_PX, x + 7, y, p.accent, a8, canvas, w, h);
                x += cw + 14 + 6;
            }
            regular.render(row.text, BODY_PX, cap_col + 16, y, p.text, a8, canvas, w, h);
        }
        y += LINE_GAP;
    }

    if !slide.footer.is_empty() {
        let fy = card_y + CARD_H as i32 - 72;
        let fw = regular.measure_width(slide.footer, BODY_PX);
        regular.render(slide.footer, BODY_PX, cx - fw / 2, fy, p.muted, a8, canvas, w, h);
    }

    // Progress dots.
    let n = SLIDES.len() as i32;
    let dots_w = (n - 1) * DOT_GAP;
    let dy = card_y + CARD_H as i32 - 44;
    for i in 0..n {
        let dx = cx - dots_w / 2 + i * DOT_GAP;
        let color = if i as usize == state.slide { p.accent } else { p.cap_bg };
        fill_circle(canvas, w, dx, dy, DOT_R, color, a8);
    }

    // Key hints.
    let hint = if state.on_last_slide() {
        "←  back     Enter  open the key list     Esc  done"
    } else {
        "←  back     →  next     Esc  skip"
    };
    let hw = regular.measure_width(hint, HINT_PX);
    regular.render(
        hint, HINT_PX, cx - hw / 2, card_y + CARD_H as i32 - 18, p.muted, a8, canvas, w, h,
    );
}
