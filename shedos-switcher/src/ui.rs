use shedos_prompt_ui::primitives::{blend_pixel, draw_rounded_box};
use shedos_prompt_ui::text::FontFace;

use crate::model::{Window, ICON_PX};

pub struct Palette {
    pub text: (u8, u8, u8),
    pub muted: (u8, u8, u8),
    pub card_bg: (u8, u8, u8),
    pub card_border: (u8, u8, u8),
    pub tile_bg: (u8, u8, u8),
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
            tile_bg: rgb(t.surface0),
            accent: rgb(t.accent),
        }
    })
}

// Card geometry. The strip is one centered row; the surface is sized
// to fit it exactly (no fullscreen dim — Alt-Tab should feel weightless).
pub const CELL_W: i32 = 148;
pub const CELL_H: i32 = 124;
pub const CELL_GAP: i32 = 12;
pub const STRIP_PAD: i32 = 18;
pub const TITLE_PX: f32 = 11.5;
pub const BADGE_PX: f32 = 10.0;

pub fn strip_size(n: usize) -> (u32, u32) {
    let w = STRIP_PAD * 2 + (n as i32) * CELL_W + (n as i32 - 1) * CELL_GAP;
    let h = STRIP_PAD * 2 + CELL_H + 24; // +24: title line under the cells
    (w as u32, h as u32)
}

fn blit_icon(canvas: &mut [u8], cw: u32, ch: u32, icon: &[u8], x0: i32, y0: i32) {
    let n = ICON_PX as i32;
    for row in 0..n {
        let dy = y0 + row;
        if dy < 0 || dy as u32 >= ch {
            continue;
        }
        for col in 0..n {
            let dx = x0 + col;
            if dx < 0 || dx as u32 >= cw {
                continue;
            }
            let s = ((row * n + col) * 4) as usize;
            let a = icon[s + 3];
            if a == 0 {
                continue;
            }
            let color = (icon[s + 2], icon[s + 1], icon[s]);
            blend_pixel(canvas, cw, dx, dy, color, a);
        }
    }
}

fn ellipsize(face: &FontFace, text: &str, px: f32, max_w: i32) -> String {
    if face.measure_width(text, px) <= max_w {
        return text.to_string();
    }
    let mut s: String = text.to_string();
    while !s.is_empty() {
        s.pop();
        let candidate = format!("{s}…");
        if face.measure_width(&candidate, px) <= max_w {
            return candidate;
        }
    }
    "…".into()
}

pub fn paint(
    canvas: &mut [u8],
    w: u32,
    h: u32,
    windows: &[Window],
    selected: usize,
    regular: &FontFace,
    bold: &FontFace,
) {
    let p = palette();

    // Panel backdrop (the whole surface IS the panel).
    draw_rounded_box(
        canvas, w, h, 0, 0, w, h, 16, 1, p.card_bg, 0xf2, p.card_border, 0xff,
    );

    for (i, win) in windows.iter().enumerate() {
        let x = STRIP_PAD + (i as i32) * (CELL_W + CELL_GAP);
        let y = STRIP_PAD;
        let is_sel = i == selected;
        let (border, thick) = if is_sel { (p.accent, 2) } else { (p.card_border, 1) };
        draw_rounded_box(
            canvas, w, h, x, y, CELL_W as u32, CELL_H as u32, 10, thick,
            if is_sel { p.tile_bg } else { p.card_bg }, 0xff, border, 0xff,
        );

        // Icon (or themed letter tile), centered in the upper cell.
        let icon_x = x + (CELL_W - ICON_PX as i32) / 2;
        let icon_y = y + 14;
        match &win.icon {
            Some(px_data) => blit_icon(canvas, w, h, px_data, icon_x, icon_y),
            None => {
                draw_rounded_box(
                    canvas, w, h, icon_x, icon_y, ICON_PX, ICON_PX, 12, 0,
                    p.tile_bg, 0xff, p.tile_bg, 0xff,
                );
                let letter = win
                    .class
                    .chars()
                    .next()
                    .unwrap_or('?')
                    .to_uppercase()
                    .to_string();
                let lw = bold.measure_width(&letter, 30.0);
                bold.render(
                    &letter, 30.0,
                    icon_x + (ICON_PX as i32 - lw) / 2,
                    icon_y + 44,
                    p.accent, 0xff, canvas, w, h,
                );
            }
        }

        // Workspace badge, top-right of the cell.
        let badge = format!("{}", win.workspace);
        let bw = regular.measure_width(&badge, BADGE_PX);
        draw_rounded_box(
            canvas, w, h, x + CELL_W - bw - 18, y + 8, (bw + 10) as u32, 16, 5, 0,
            p.tile_bg, 0xff, p.tile_bg, 0xff,
        );
        regular.render(
            &badge, BADGE_PX, x + CELL_W - bw - 13, y + 20, p.muted, 0xff, canvas, w, h,
        );

        // Class label inside the cell, under the icon.
        let label = ellipsize(regular, &win.class, TITLE_PX, CELL_W - 16);
        let lw = regular.measure_width(&label, TITLE_PX);
        regular.render(
            &label, TITLE_PX, x + (CELL_W - lw) / 2, y + CELL_H - 14,
            if is_sel { p.text } else { p.muted }, 0xff, canvas, w, h,
        );
    }

    // Selected window's full title on the line under the strip.
    if let Some(win) = windows.get(selected) {
        let title = ellipsize(regular, &win.title, TITLE_PX, w as i32 - STRIP_PAD * 2);
        let tw = regular.measure_width(&title, TITLE_PX);
        regular.render(
            &title, TITLE_PX, (w as i32 - tw) / 2,
            STRIP_PAD + CELL_H + 18, p.text, 0xff, canvas, w, h,
        );
    }
}
