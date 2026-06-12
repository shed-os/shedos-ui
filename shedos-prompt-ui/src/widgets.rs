//! Composed widgets onto an Argb8888 canvas: clock, date, prompt
//! input, greeting/error line, ShedOS branding. Stateless.

use crate::power;
use crate::primitives::{draw_fingerprint_icon, draw_rounded_box};
use crate::FingerprintRender;
use crate::text::FontFace;
use crate::theme::Theme;
use crate::{OutputRect, PromptState};

const POWER_GLYPH: char = '\u{F011}';
const RESTART_GLYPH: char = '\u{F021}';
const SLEEP_GLYPH: char = '\u{F186}';
const HIBERNATE_GLYPH: char = '\u{F236}';

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
const INPUT_PAD: i32 = 12;

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
    wordmark: &mut crate::wordmark::Wordmark,
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
    let clock = now.format("%-I:%M %p").to_string();
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

    // Caps-lock tag at the left edge of the input. Rendered before
    // the password dots so the dot-centering math can reserve room
    // for it. When caps is off, the dots use the full inner width.
    let caps_region_w = if state.capslock {
        let tag = "CAPS";
        let tag_w = bold.measure_width(tag, BRAND_PX);
        let tag_x = box_x + INPUT_PAD;
        let tag_y = box_y + (INPUT_H as i32 * 2 / 3);
        bold.render(
            tag, BRAND_PX, tag_x, tag_y, accent_color, 0xcc,
            canvas, canvas_w, canvas_h,
        );
        INPUT_PAD + tag_w + INPUT_PAD
    } else {
        0
    };

    // Render password as bullet glyphs. Cap the visible count to what
    // fits inside the input box (minus the CAPS reservation on the
    // left) so long passwords don't render past the rounded edges;
    // doubles as a shoulder-surfer guard since the displayed length
    // stops growing past the cap.
    if state.typed_chars > 0 {
        let bullet_w = regular.measure_width("●", INPUT_FONT_PX);
        let dots_region_x = box_x + caps_region_w;
        let dots_region_w = (INPUT_W as i32 - caps_region_w - INPUT_PAD).max(0);
        let max_dots = if bullet_w > 0 {
            ((dots_region_w / bullet_w) as usize).max(1)
        } else {
            state.typed_chars
        };
        let visible = state.typed_chars.min(max_dots);
        let dots: String = "●".repeat(visible);
        let dots_w = regular.measure_width(&dots, INPUT_FONT_PX);
        let dots_x = dots_region_x + ((dots_region_w - dots_w) / 2);
        let dots_y = box_y + (INPUT_H as i32 * 2 / 3);
        regular.render(
            &dots, INPUT_FONT_PX, dots_x, dots_y, text_color, 0xff, canvas, canvas_w, canvas_h,
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

    let wordmark_target_w = (pw / 3).clamp(360, 900) as u32;
    let center_x = px + pw / 2;
    let center_y = py + (ph as f32 * 0.85) as i32;
    wordmark.blit_centered(
        canvas,
        canvas_w,
        canvas_h,
        center_x,
        center_y,
        wordmark_target_w,
    );

    paint_power_menu(
        canvas, canvas_w, canvas_h, output_rect, &state.power_menu,
        regular, bold, theme,
    );
}

#[allow(clippy::too_many_arguments)]
fn paint_power_menu(
    canvas: &mut [u8],
    canvas_w: u32,
    canvas_h: u32,
    rect: &OutputRect,
    state: &crate::PowerMenuState,
    regular: &FontFace,
    bold: &FontFace,
    theme: &Theme,
) {
    let text_color = rgb(theme.text);
    let base_color = rgb(theme.base);
    let accent_color = rgb(theme.accent);

    let (cx, cy) = power::button_center(rect);
    let btn_x = cx - power::BTN_SIZE / 2;
    let btn_y = cy - power::BTN_SIZE / 2;
    let btn_size = power::BTN_SIZE as u32;

    let (fill_alpha, border_alpha) = if state.open {
        (0xee, 0xff)
    } else {
        (0xb0, 0xcc)
    };
    draw_rounded_box(
        canvas, canvas_w, canvas_h, btn_x, btn_y,
        btn_size, btn_size, btn_size / 2, 2,
        base_color, fill_alpha,
        accent_color, border_alpha,
    );

    let glyph_str = POWER_GLYPH.to_string();
    let (xmin, ymin, w_px, h_px) = regular.glyph_bbox(POWER_GLYPH, power::GLYPH_PX);
    let glyph_x = cx - xmin - (w_px as i32) / 2;
    let glyph_baseline = cy + ymin + (h_px as i32) / 2;
    regular.render(
        &glyph_str, power::GLYPH_PX, glyph_x, glyph_baseline,
        accent_color, 0xff, canvas, canvas_w, canvas_h,
    );

    if !state.open {
        return;
    }

    let items = crate::PowerAction::all();
    if items.is_empty() {
        return;
    }
    let (mx, my) = power::menu_origin(rect);
    let menu_w = power::MENU_W;
    let menu_h = (power::ITEM_H * items.len() as i32) as u32;

    draw_rounded_box(
        canvas, canvas_w, canvas_h, mx, my,
        menu_w, menu_h, power::MENU_RADIUS, 1,
        base_color, 0xee,
        accent_color, 0xcc,
    );

    let pointer_row = state.pointer.and_then(|(px, py)| {
        if px < mx as f32 || px >= (mx + menu_w as i32) as f32 {
            return None;
        }
        let local_y = py - my as f32;
        if local_y < 0.0 {
            return None;
        }
        let idx = (local_y / power::ITEM_H as f32) as i32;
        if idx >= 0 && (idx as usize) < items.len() {
            Some(idx as usize)
        } else {
            None
        }
    });

    let icon_px = power::LABEL_PX;
    for (i, action) in items.iter().enumerate() {
        let row_y = my + (i as i32) * power::ITEM_H;
        let highlighted = pointer_row == Some(i)
            || (state.kb_active && state.selected == i);
        if highlighted {
            draw_rounded_box(
                canvas, canvas_w, canvas_h,
                mx + 4, row_y + 2,
                menu_w - 8, (power::ITEM_H - 4) as u32,
                6, 0,
                accent_color, 0x33,
                accent_color, 0x00,
            );
        }
        let label = action.label();
        let face: &FontFace = if highlighted { bold } else { regular };
        let label_color = if highlighted { accent_color } else { text_color };

        let row_cy = row_y + power::ITEM_H / 2;

        let icon_ch = menu_icon_for(*action);
        let icon_str = icon_ch.to_string();
        let icon_w = regular.measure_width(&icon_str, icon_px);
        let (_ixmin, icon_ymin, _iw_px, icon_h) = regular.glyph_bbox(icon_ch, icon_px);
        let icon_x = mx + 14;
        let icon_baseline = row_cy + icon_ymin + (icon_h as i32) / 2;
        regular.render(
            &icon_str, icon_px, icon_x, icon_baseline,
            label_color, 0xff, canvas, canvas_w, canvas_h,
        );

        let label_x = icon_x + icon_w + 12;
        if let Some(first_ch) = label.chars().next() {
            let (_lxmin, l_ymin, _lw_px, l_h) = face.glyph_bbox(first_ch, power::LABEL_PX);
            let label_baseline = row_cy + l_ymin + (l_h as i32) / 2;
            face.render(
                label, power::LABEL_PX, label_x, label_baseline,
                label_color, 0xff, canvas, canvas_w, canvas_h,
            );
        }
    }
}

fn menu_icon_for(action: crate::PowerAction) -> char {
    use crate::PowerAction::*;
    match action {
        Suspend => SLEEP_GLYPH,
        Hibernate => HIBERNATE_GLYPH,
        Restart => RESTART_GLYPH,
        Shutdown => POWER_GLYPH,
    }
}
