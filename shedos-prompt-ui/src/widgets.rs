//! Composed widgets onto an Argb8888 canvas: clock, date, prompt
//! input, greeting/error line, ShedOS branding. Stateless.

use crate::primitives::{draw_fingerprint_icon, draw_rounded_box};
use crate::FingerprintRender;
use crate::text::FontFace;
use crate::theme::Theme;
use crate::{OutputRect, PromptState};

const CLOCK_PX: f32 = 120.0;
const DATE_PX: f32 = 24.0;
const GREET_PX: f32 = 32.0;
const BRAND_PX: f32 = 18.0;
const INPUT_FONT_PX: f32 = 18.0;
const FP_HINT_PX: f32 = 16.0;

const INPUT_W: u32 = 300;
const INPUT_H: u32 = 50;
const INPUT_RADIUS: u32 = 10;
const INPUT_BORDER: u32 = 2;

const FP_ICON_SIZE: f32 = 40.0;
const FP_ICON_GAP: i32 = 18;

/// Convert 0xAARRGGBB → (R, G, B). Alpha is read separately by callers.
fn rgb(c: u32) -> (u8, u8, u8) {
    (((c >> 16) & 0xff) as u8, ((c >> 8) & 0xff) as u8, (c & 0xff) as u8)
}

/// Paint widgets centered on `output_rect`. The wallpaper is assumed
/// to already cover the full canvas.
#[allow(clippy::too_many_arguments)]
pub fn paint_widgets(
    canvas: &mut [u8],
    canvas_w: u32,
    canvas_h: u32,
    output_rect: &OutputRect,
    state: &PromptState,
    theme: &Theme,
    regular: &FontFace,
    bold: &FontFace,
    error_message: Option<&str>,
    greeting: Option<&str>,
    fingerprint: Option<&FingerprintRender<'_>>,
) {
    let (px, py, pw, ph) = (output_rect.x, output_rect.y, output_rect.w, output_rect.h);
    let text_color = rgb(theme.text);
    let accent_color = rgb(theme.accent);
    let base_color = rgb(theme.base);
    let red_color = rgb(theme.red);

    let now = chrono::Local::now();
    let clock = now.format("%H:%M").to_string();
    let date = now.format("%A, %B %-d").to_string();

    // Clock at ~30% from the top of the rect.
    let clock_w = regular.measure_width(&clock, CLOCK_PX);
    let clock_x = px + (pw - clock_w) / 2;
    let clock_y = py + (ph as f32 * 0.30) as i32;
    regular.render(
        &clock, CLOCK_PX, clock_x, clock_y, text_color, 0xff, canvas, canvas_w, canvas_h,
    );

    // Date just under the clock.
    let date_w = regular.measure_width(&date, DATE_PX);
    let date_x = px + (pw - date_w) / 2;
    let date_y = clock_y + (CLOCK_PX as i32 / 4);
    regular.render(
        &date, DATE_PX, date_x, date_y, text_color, 0xcc, canvas, canvas_w, canvas_h,
    );

    // Password input box at ~58% from top.
    let border = if state.fail {
        red_color
    } else if state.success {
        // success flash: caller should also clamp the duration
        rgb(theme.accent)
    } else {
        accent_color
    };
    let box_x = px + (pw - INPUT_W as i32) / 2;
    let box_y = py + (ph as f32 * 0.58) as i32;
    draw_rounded_box(
        canvas, canvas_w, canvas_h, box_x, box_y,
        INPUT_W, INPUT_H, INPUT_RADIUS, INPUT_BORDER,
        base_color, 0xe6,
        border, 0xee,
    );

    if let Some(fp) = fingerprint {
        let icon_cx = (box_x as f32) - (FP_ICON_GAP as f32) - (FP_ICON_SIZE / 2.0);
        let icon_cy = (box_y + INPUT_H as i32 / 2) as f32;
        draw_fingerprint_icon(
            canvas, canvas_w, icon_cx, icon_cy, FP_ICON_SIZE,
            rgb(fp.icon_color_argb), 0xee,
        );
    }

    // Render password as bullet glyphs.
    if state.typed_chars > 0 {
        let dots: String = "●".repeat(state.typed_chars);
        let dots_w = regular.measure_width(&dots, INPUT_FONT_PX);
        let dots_x = box_x + ((INPUT_W as i32 - dots_w) / 2);
        let dots_y = box_y + (INPUT_H as i32 * 2 / 3);
        regular.render(
            &dots, INPUT_FONT_PX, dots_x, dots_y, text_color, 0xff, canvas, canvas_w, canvas_h,
        );
    }

    // Caps-lock indicator (small uppercase tag at the right edge of the input).
    if state.capslock {
        let tag = "CAPS";
        let tag_w = bold.measure_width(tag, BRAND_PX);
        let tag_x = box_x + INPUT_W as i32 - tag_w - 12;
        let tag_y = box_y + (INPUT_H as i32 * 2 / 3);
        bold.render(
            tag, BRAND_PX, tag_x, tag_y, accent_color, 0xcc,
            canvas, canvas_w, canvas_h,
        );
    }

    // Below the input: error message (if any) or greeting.
    let line_y = box_y + INPUT_H as i32 + 56;
    if let Some(msg) = error_message {
        let err_w = regular.measure_width(msg, GREET_PX);
        let err_x = px + (pw - err_w) / 2;
        regular.render(
            msg, GREET_PX, err_x, line_y, red_color, 0xff,
            canvas, canvas_w, canvas_h,
        );
    } else if let Some(greet) = greeting {
        let greet_w = regular.measure_width(greet, GREET_PX);
        let greet_x = px + (pw - greet_w) / 2;
        regular.render(
            greet, GREET_PX, greet_x, line_y, accent_color, 0xff,
            canvas, canvas_w, canvas_h,
        );
    }

    // Fingerprint hint sits below the greeting/error in muted text.
    if let Some(fp) = fingerprint {
        let hint_y = line_y + GREET_PX as i32 + 8;
        let hint_w = regular.measure_width(fp.hint, FP_HINT_PX);
        let hint_x = px + (pw - hint_w) / 2;
        regular.render(
            fp.hint, FP_HINT_PX, hint_x, hint_y, text_color, 0x99,
            canvas, canvas_w, canvas_h,
        );
    }

    // Branding near bottom of the rect.
    let brand = "ShedOS";
    let brand_w = bold.measure_width(brand, BRAND_PX);
    let brand_x = px + (pw - brand_w) / 2;
    let brand_y = py + (ph as f32 * 0.93) as i32;
    bold.render(
        brand, BRAND_PX, brand_x, brand_y, accent_color, 0x99,
        canvas, canvas_w, canvas_h,
    );
}
