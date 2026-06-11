use std::time::{Duration, Instant};

use shedos_prompt_ui::primitives::{blend_pixel, draw_rounded_box};
use shedos_prompt_ui::text::FontFace;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Lock,
    Suspend,
    Restart,
    Shutdown,
}

impl Action {
    pub fn all() -> [Action; 4] {
        [Action::Lock, Action::Suspend, Action::Restart, Action::Shutdown]
    }
    pub fn label(self) -> &'static str {
        match self {
            Action::Lock => "Lock screen",
            Action::Suspend => "Sleep",
            Action::Restart => "Reboot",
            Action::Shutdown => "Shut down",
        }
    }
    pub fn glyph(self) -> char {
        match self {
            Action::Lock => '\u{F023}',
            Action::Suspend => '\u{F186}',
            Action::Restart => '\u{F021}',
            Action::Shutdown => '\u{F011}',
        }
    }
    pub fn confirm_question(self) -> &'static str {
        match self {
            Action::Lock | Action::Suspend => "",
            Action::Restart => "Reboot?",
            Action::Shutdown => "Shut down?",
        }
    }
    pub fn confirm_button(self) -> &'static str {
        match self {
            Action::Lock | Action::Suspend => "",
            Action::Restart => "Reboot",
            Action::Shutdown => "Shut down",
        }
    }
    pub fn action_color(self) -> (u8, u8, u8) {
        match self {
            Action::Lock => palette().blue,
            Action::Suspend => palette().mauve,
            Action::Restart => palette().yellow,
            Action::Shutdown => palette().coral,
        }
    }
}

// Colors come from the live theme (greeter.toml via prompt-ui's
// Theme); the load_or_default fallback is the Catppuccin Mocha set
// the mockups were approved with. The roles keep the mockups' names.
pub struct Palette {
    pub text: (u8, u8, u8),
    pub muted: (u8, u8, u8),
    pub card_bg: (u8, u8, u8),
    pub card_border: (u8, u8, u8),
    pub row_bg: (u8, u8, u8),
    pub dark_text: (u8, u8, u8),
    pub backdrop_rgb: (u8, u8, u8),
    pub blue: (u8, u8, u8),
    pub mauve: (u8, u8, u8),
    pub yellow: (u8, u8, u8),
    pub coral: (u8, u8, u8),
}

fn rgb(argb: u32) -> (u8, u8, u8) {
    (
        ((argb >> 16) & 0xff) as u8,
        ((argb >> 8) & 0xff) as u8,
        (argb & 0xff) as u8,
    )
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
            row_bg: rgb(t.surface0),
            dark_text: rgb(t.base),
            backdrop_rgb: rgb(t.base),
            blue: rgb(t.accent),
            mauve: rgb(t.accent_secondary),
            yellow: rgb(t.yellow),
            coral: rgb(t.red),
        }
    })
}

const BACKDROP_ALPHA: u8 = 0xb3; // 0.7

// Card dimensions. Menu and confirm cards are the same width so the
// swap looks like one stage with new content, not two different cards.
const CARD_W: u32 = 280;
const CARD_RADIUS: u32 = 14;
const CARD_BORDER_PX: u32 = 1;
const CARD_PAD_X: i32 = 22;
const CARD_PAD_Y: i32 = 22;

const POWER_LABEL_PX: f32 = 13.0;
const POWER_LABEL_GAP: i32 = 14;

const ROW_HEIGHT: i32 = 36;
const ROW_GAP: i32 = 8;
const ROW_RADIUS: u32 = 8;
const ROW_PAD_X: i32 = 12;
const ROW_LABEL_PX: f32 = 12.0;
const ROW_GLYPH_LABEL_GAP: i32 = 10;

const CONFIRM_PAD_Y: i32 = 26;
const CONFIRM_GLYPH_PX: f32 = 42.0;
const CONFIRM_GLYPH_GAP: i32 = 14;
const QUESTION_PX: f32 = 15.0;
const QUESTION_GAP: i32 = 6;
const CONSEQUENCE_PX: f32 = 12.0;
const CONSEQUENCE_GAP: i32 = 18;
const BUTTON_H: i32 = 32;
const BUTTON_RADIUS: u32 = 8;
const BUTTON_GAP: i32 = 8;
const BUTTON_PX: f32 = 13.0;

const OPEN_BACKDROP_MS: u32 = 150;
const OPEN_ROW_OFFSET_MS: u32 = 30;
const OPEN_FADE_MS: u32 = 120;
const SWAP_MS: u32 = 150;
const DISMISS_MS: u32 = 120;

#[derive(Debug, Clone, Copy)]
pub enum Phase {
    Opening,
    Menu,
    SwapToConfirm,
    Confirming,
    SwapToMenu,
    Dismissing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmFocus {
    Cancel,
    Commit,
}

#[derive(Debug)]
pub struct PowerState {
    pub phase: Phase,
    pub action: Option<Action>,
    pub focus_menu: usize,
    pub focus_confirm: ConfirmFocus,
    pub opened_at: Instant,
    pub phase_started_at: Instant,
}

impl PowerState {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            phase: Phase::Opening,
            action: None,
            focus_menu: 0,
            focus_confirm: ConfirmFocus::Cancel,
            opened_at: now,
            phase_started_at: now,
        }
    }

    pub fn focused_action(&self) -> Action {
        Action::all()[self.focus_menu.min(Action::all().len() - 1)]
    }

    pub fn move_menu(&mut self, delta: i32) {
        if matches!(self.phase, Phase::Menu | Phase::Opening) {
            let n = Action::all().len() as i32;
            let f = self.focus_menu as i32;
            self.focus_menu = ((f + delta).rem_euclid(n)) as usize;
        }
    }

    pub fn toggle_confirm(&mut self) {
        if matches!(self.phase, Phase::Confirming | Phase::SwapToConfirm) {
            self.focus_confirm = match self.focus_confirm {
                ConfirmFocus::Cancel => ConfirmFocus::Commit,
                ConfirmFocus::Commit => ConfirmFocus::Cancel,
            };
        }
    }

    pub fn enter_confirming(&mut self, action: Action) {
        self.action = Some(action);
        self.focus_confirm = ConfirmFocus::Cancel;
        self.phase = Phase::SwapToConfirm;
        self.phase_started_at = Instant::now();
    }

    pub fn enter_menu(&mut self) {
        self.action = None;
        self.phase = Phase::SwapToMenu;
        self.phase_started_at = Instant::now();
    }

    pub fn enter_dismiss(&mut self) {
        self.phase = Phase::Dismissing;
        self.phase_started_at = Instant::now();
    }

    pub fn tick(&mut self, now: Instant) -> bool {
        let dt = now.duration_since(self.phase_started_at).as_millis() as u32;
        match self.phase {
            Phase::Opening if dt >= 4 * OPEN_ROW_OFFSET_MS + OPEN_FADE_MS => {
                self.phase = Phase::Menu;
                true
            }
            Phase::SwapToConfirm if dt >= SWAP_MS => {
                self.phase = Phase::Confirming;
                true
            }
            Phase::SwapToMenu if dt >= SWAP_MS => {
                self.phase = Phase::Menu;
                true
            }
            _ => false,
        }
    }

    pub fn is_settled(&self) -> bool {
        matches!(self.phase, Phase::Menu | Phase::Confirming)
    }

    pub fn is_dismissing(&self) -> bool {
        matches!(self.phase, Phase::Dismissing)
    }

    pub fn dismiss_done(&self, now: Instant) -> bool {
        matches!(self.phase, Phase::Dismissing)
            && now.duration_since(self.phase_started_at) >= Duration::from_millis(DISMISS_MS as u64)
    }
}

fn ease_out_cubic(t: f32) -> f32 {
    let inv = 1.0 - t.clamp(0.0, 1.0);
    1.0 - inv * inv * inv
}

fn anim(now: Instant, started: Instant, delay_ms: u32, duration_ms: u32) -> f32 {
    let elapsed = now.saturating_duration_since(started).as_millis() as i64 - delay_ms as i64;
    if elapsed <= 0 {
        0.0
    } else if elapsed >= duration_ms as i64 {
        1.0
    } else {
        ease_out_cubic(elapsed as f32 / duration_ms as f32)
    }
}

// ---------- layout helpers ----------

fn menu_card_height() -> i32 {
    let rows = Action::all().len() as i32;
    CARD_PAD_Y
        + POWER_LABEL_PX as i32
        + POWER_LABEL_GAP
        + ROW_HEIGHT * rows
        + ROW_GAP * (rows - 1)
        + CARD_PAD_Y
}

fn confirm_card_height() -> i32 {
    CONFIRM_PAD_Y
        + CONFIRM_GLYPH_PX as i32
        + CONFIRM_GLYPH_GAP
        + QUESTION_PX as i32
        + QUESTION_GAP
        + CONSEQUENCE_PX as i32
        + CONSEQUENCE_GAP
        + BUTTON_H
        + CONFIRM_PAD_Y
}

fn menu_card_rect(canvas_w: u32, canvas_h: u32) -> (i32, i32, u32, u32) {
    let w = CARD_W;
    let h = menu_card_height() as u32;
    let x = (canvas_w as i32 - w as i32) / 2;
    let y = (canvas_h as i32 - h as i32) / 2;
    (x, y, w, h)
}

fn confirm_card_rect(canvas_w: u32, canvas_h: u32) -> (i32, i32, u32, u32) {
    let w = CARD_W;
    let h = confirm_card_height() as u32;
    let x = (canvas_w as i32 - w as i32) / 2;
    let y = (canvas_h as i32 - h as i32) / 2;
    (x, y, w, h)
}

fn menu_row_rect(canvas_w: u32, canvas_h: u32, row: usize) -> (i32, i32, u32, u32) {
    let (cx, cy, cw, _) = menu_card_rect(canvas_w, canvas_h);
    let inner_x = cx + CARD_PAD_X;
    let inner_w = cw as i32 - CARD_PAD_X * 2;
    let label_y = cy + CARD_PAD_Y + POWER_LABEL_PX as i32;
    let rows_top = label_y + POWER_LABEL_GAP;
    let y = rows_top + row as i32 * (ROW_HEIGHT + ROW_GAP);
    (inner_x, y, inner_w as u32, ROW_HEIGHT as u32)
}

fn confirm_button_rect(
    canvas_w: u32,
    canvas_h: u32,
    which: ConfirmFocus,
) -> (i32, i32, u32, u32) {
    let (cx, cy, cw, _) = confirm_card_rect(canvas_w, canvas_h);
    let inner_x = cx + CARD_PAD_X;
    let inner_w = cw as i32 - CARD_PAD_X * 2;
    let btn_w = (inner_w - BUTTON_GAP) / 2;
    let btn_y = cy + CARD_PAD_Y
        + CONFIRM_GLYPH_PX as i32
        + CONFIRM_GLYPH_GAP
        + QUESTION_PX as i32
        + QUESTION_GAP
        + CONSEQUENCE_PX as i32
        + CONSEQUENCE_GAP;
    let x = match which {
        ConfirmFocus::Cancel => inner_x,
        ConfirmFocus::Commit => inner_x + btn_w + BUTTON_GAP,
    };
    (x, btn_y, btn_w as u32, BUTTON_H as u32)
}

pub fn hit_test_menu(canvas_w: u32, canvas_h: u32, x: f32, y: f32) -> Option<usize> {
    for row in 0..Action::all().len() {
        let (rx, ry, rw, rh) = menu_row_rect(canvas_w, canvas_h, row);
        if x >= rx as f32 && x < (rx + rw as i32) as f32 && y >= ry as f32 && y < (ry + rh as i32) as f32 {
            return Some(row);
        }
    }
    None
}

pub fn is_inside_menu_card(canvas_w: u32, canvas_h: u32, x: f32, y: f32) -> bool {
    let (cx, cy, cw, ch) = menu_card_rect(canvas_w, canvas_h);
    x >= cx as f32 && x < (cx + cw as i32) as f32 && y >= cy as f32 && y < (cy + ch as i32) as f32
}

pub fn is_inside_confirm_card(canvas_w: u32, canvas_h: u32, x: f32, y: f32) -> bool {
    let (cx, cy, cw, ch) = confirm_card_rect(canvas_w, canvas_h);
    x >= cx as f32 && x < (cx + cw as i32) as f32 && y >= cy as f32 && y < (cy + ch as i32) as f32
}

pub fn hit_test_confirm(canvas_w: u32, canvas_h: u32, x: f32, y: f32) -> Option<ConfirmFocus> {
    for which in [ConfirmFocus::Cancel, ConfirmFocus::Commit] {
        let (bx, by, bw, bh) = confirm_button_rect(canvas_w, canvas_h, which);
        if x >= bx as f32
            && x < (bx + bw as i32) as f32
            && y >= by as f32
            && y < (by + bh as i32) as f32
        {
            return Some(which);
        }
    }
    None
}

// ---------- painting ----------

pub fn paint(
    canvas: &mut [u8],
    canvas_w: u32,
    canvas_h: u32,
    state: &PowerState,
    regular: &FontFace,
    bold: &FontFace,
    now: Instant,
) {
    let bdrop = backdrop_alpha_now(state, now);
    fill_backdrop(canvas, canvas_w, canvas_h, bdrop);

    let menu_a = menu_alpha_now(state, now);
    let confirm_a = confirm_alpha_now(state, now);

    if menu_a > 0.001 {
        paint_menu(canvas, canvas_w, canvas_h, state, regular, bold, now, menu_a);
    }
    if confirm_a > 0.001 {
        if let Some(action) = state.action {
            paint_confirming(
                canvas, canvas_w, canvas_h, action, state.focus_confirm, regular, bold,
                confirm_a,
            );
        }
    }
}

fn backdrop_alpha_now(state: &PowerState, now: Instant) -> u8 {
    match state.phase {
        Phase::Opening => {
            let t = anim(now, state.opened_at, 0, OPEN_BACKDROP_MS);
            (BACKDROP_ALPHA as f32 * t) as u8
        }
        Phase::Dismissing => {
            let t = anim(now, state.phase_started_at, 0, DISMISS_MS);
            (BACKDROP_ALPHA as f32 * (1.0 - t)) as u8
        }
        _ => BACKDROP_ALPHA,
    }
}

fn menu_alpha_now(state: &PowerState, now: Instant) -> f32 {
    match state.phase {
        Phase::Opening | Phase::Menu => 1.0,
        Phase::SwapToConfirm => 1.0 - anim(now, state.phase_started_at, 0, SWAP_MS),
        Phase::SwapToMenu => anim(now, state.phase_started_at, 0, SWAP_MS),
        Phase::Confirming => 0.0,
        Phase::Dismissing => {
            if state.action.is_none() || matches!(state.action, Some(Action::Lock)) {
                1.0 - anim(now, state.phase_started_at, 0, DISMISS_MS)
            } else {
                0.0
            }
        }
    }
}

fn confirm_alpha_now(state: &PowerState, now: Instant) -> f32 {
    match state.phase {
        Phase::SwapToConfirm => anim(now, state.phase_started_at, 0, SWAP_MS),
        Phase::Confirming => 1.0,
        Phase::SwapToMenu => 1.0 - anim(now, state.phase_started_at, 0, SWAP_MS),
        Phase::Dismissing => match state.action {
            Some(Action::Restart) | Some(Action::Shutdown) => {
                1.0 - anim(now, state.phase_started_at, 0, DISMISS_MS)
            }
            _ => 0.0,
        },
        _ => 0.0,
    }
}

fn fill_backdrop(canvas: &mut [u8], w: u32, h: u32, alpha: u8) {
    // wl_shm Argb8888 with straight alpha: write RGB unmultiplied,
    // alpha into the alpha byte. Compositor blends against what's
    // behind (the wallpaper).
    let r = palette().backdrop_rgb.0;
    let g = palette().backdrop_rgb.1;
    let b = palette().backdrop_rgb.2;
    let pixels = (w as usize) * (h as usize);
    for i in 0..pixels {
        let off = i * 4;
        canvas[off] = b;
        canvas[off + 1] = g;
        canvas[off + 2] = r;
        canvas[off + 3] = alpha;
    }
}

fn scale_alpha(group: f32, base: u8) -> u8 {
    (group.clamp(0.0, 1.0) * base as f32) as u8
}

fn paint_menu(
    canvas: &mut [u8],
    canvas_w: u32,
    canvas_h: u32,
    state: &PowerState,
    regular: &FontFace,
    bold: &FontFace,
    now: Instant,
    group_alpha: f32,
) {
    let (cx, cy, cw, ch) = menu_card_rect(canvas_w, canvas_h);

    // The whole card and its contents fade together for swap/dismiss;
    // for the open animation, each element has its own per-element
    // delay and we still pass group_alpha = 1.
    let card_alpha = scale_alpha(group_alpha, 0xff);
    draw_rounded_box(
        canvas, canvas_w, canvas_h, cx, cy, cw, ch, CARD_RADIUS, CARD_BORDER_PX,
        palette().card_bg, card_alpha, palette().card_border, card_alpha,
    );

    // POWER label.
    let label = "Power";
    let label_w = regular.measure_width(label, POWER_LABEL_PX);
    let label_x = (canvas_w as i32 - label_w) / 2;
    let label_y = cy + CARD_PAD_Y + POWER_LABEL_PX as i32;
    let label_open = match state.phase {
        Phase::Opening => anim(now, state.opened_at, 0, OPEN_FADE_MS),
        _ => 1.0,
    };
    let label_alpha = scale_alpha(group_alpha * label_open, 0xb3);
    regular.render(
        label, POWER_LABEL_PX, label_x, label_y, palette().text, label_alpha, canvas, canvas_w, canvas_h,
    );

    // Rows.
    for (row, action) in Action::all().iter().enumerate() {
        let row_delay = OPEN_ROW_OFFSET_MS * (row as u32 + 2);
        let row_open = match state.phase {
            Phase::Opening => anim(now, state.opened_at, row_delay, OPEN_FADE_MS),
            _ => 1.0,
        };
        let alpha = scale_alpha(group_alpha * row_open, 0xff);
        paint_menu_row(
            canvas,
            canvas_w,
            canvas_h,
            *action,
            row,
            row == state.focus_menu,
            regular,
            bold,
            alpha,
        );
    }
}

fn paint_menu_row(
    canvas: &mut [u8],
    canvas_w: u32,
    canvas_h: u32,
    action: Action,
    row: usize,
    focused: bool,
    regular: &FontFace,
    bold: &FontFace,
    alpha: u8,
) {
    let (rx, ry, rw, rh) = menu_row_rect(canvas_w, canvas_h, row);

    let (fill, label_color, glyph_color, weight_bold) = match action {
        Action::Shutdown => (palette().coral, palette().dark_text, palette().dark_text, true),
        _ => (palette().row_bg, palette().text, palette().text, false),
    };
    let border = if focused { palette().text } else { fill };
    let border_thick = if focused { 1 } else { 0 };

    draw_rounded_box(
        canvas, canvas_w, canvas_h, rx, ry, rw, rh, ROW_RADIUS, border_thick,
        fill, alpha, border, alpha,
    );

    let face: &FontFace = if weight_bold { bold } else { regular };
    let glyph_str = action.glyph().to_string();
    let glyph_w = face.measure_width(&glyph_str, ROW_LABEL_PX);
    let label = action.label();
    let label_w = face.measure_width(label, ROW_LABEL_PX);
    let content_w = glyph_w + ROW_GLYPH_LABEL_GAP + label_w;

    // Align glyph + label centered inside the row.
    let cx = rx + (rw as i32) / 2;
    let start_x = cx - content_w / 2;
    let baseline_y = ry + (ROW_HEIGHT + ROW_LABEL_PX as i32) / 2;

    face.render(
        &glyph_str,
        ROW_LABEL_PX,
        start_x,
        baseline_y,
        glyph_color,
        alpha,
        canvas,
        canvas_w,
        canvas_h,
    );
    face.render(
        label,
        ROW_LABEL_PX,
        start_x + glyph_w + ROW_GLYPH_LABEL_GAP,
        baseline_y,
        label_color,
        alpha,
        canvas,
        canvas_w,
        canvas_h,
    );

    // Padding-x is honoured by the row's own outer rect (rx); no extra adjustment needed.
    let _ = ROW_PAD_X;
}

fn paint_confirming(
    canvas: &mut [u8],
    canvas_w: u32,
    canvas_h: u32,
    action: Action,
    focus: ConfirmFocus,
    regular: &FontFace,
    bold: &FontFace,
    group_alpha: f32,
) {
    let (cx, cy, cw, ch) = confirm_card_rect(canvas_w, canvas_h);
    let alpha = scale_alpha(group_alpha, 0xff);
    draw_rounded_box(
        canvas, canvas_w, canvas_h, cx, cy, cw, ch, CARD_RADIUS, CARD_BORDER_PX,
        palette().card_bg, alpha, palette().card_border, alpha,
    );

    let inner_x = cx + CARD_PAD_X;
    let inner_w = cw as i32 - CARD_PAD_X * 2;
    let canvas_cx = inner_x + inner_w / 2;

    // Big glyph centered.
    let glyph_str = action.glyph().to_string();
    let glyph_w = regular.measure_width(&glyph_str, CONFIRM_GLYPH_PX);
    let glyph_baseline = cy + CARD_PAD_Y + CONFIRM_GLYPH_PX as i32;
    regular.render(
        &glyph_str,
        CONFIRM_GLYPH_PX,
        canvas_cx - glyph_w / 2,
        glyph_baseline,
        action.action_color(),
        alpha,
        canvas,
        canvas_w,
        canvas_h,
    );

    // Question.
    let question = action.confirm_question();
    let q_w = bold.measure_width(question, QUESTION_PX);
    let q_baseline = glyph_baseline + CONFIRM_GLYPH_GAP + QUESTION_PX as i32;
    bold.render(
        question,
        QUESTION_PX,
        canvas_cx - q_w / 2,
        q_baseline,
        palette().text,
        alpha,
        canvas,
        canvas_w,
        canvas_h,
    );

    // Consequence.
    let conseq = "Unsaved work in any app will be lost.";
    let c_w = regular.measure_width(conseq, CONSEQUENCE_PX);
    let c_baseline = q_baseline + QUESTION_GAP + CONSEQUENCE_PX as i32;
    regular.render(
        conseq,
        CONSEQUENCE_PX,
        canvas_cx - c_w / 2,
        c_baseline,
        palette().muted,
        alpha,
        canvas,
        canvas_w,
        canvas_h,
    );

    // Buttons.
    let (cancel_x, cancel_y, cancel_w, cancel_h) =
        confirm_button_rect(canvas_w, canvas_h, ConfirmFocus::Cancel);
    let (commit_x, commit_y, commit_w, commit_h) =
        confirm_button_rect(canvas_w, canvas_h, ConfirmFocus::Commit);

    // Cancel: palette().row_bg fill, palette().text label.
    let cancel_focused = focus == ConfirmFocus::Cancel;
    draw_rounded_box(
        canvas, canvas_w, canvas_h,
        cancel_x, cancel_y, cancel_w, cancel_h, BUTTON_RADIUS,
        if cancel_focused { 1 } else { 0 },
        palette().row_bg, alpha,
        palette().text, alpha,
    );
    let cancel_text = "Cancel";
    let ct_w = regular.measure_width(cancel_text, BUTTON_PX);
    let ct_x = cancel_x + (cancel_w as i32 - ct_w) / 2;
    let ct_y = cancel_y + (BUTTON_H + BUTTON_PX as i32) / 2;
    regular.render(
        cancel_text,
        BUTTON_PX,
        ct_x,
        ct_y,
        palette().text,
        alpha,
        canvas,
        canvas_w,
        canvas_h,
    );

    // Commit: action-color fill, palette().dark_text label, bold weight.
    let commit_focused = focus == ConfirmFocus::Commit;
    draw_rounded_box(
        canvas, canvas_w, canvas_h,
        commit_x, commit_y, commit_w, commit_h, BUTTON_RADIUS,
        if commit_focused { 1 } else { 0 },
        action.action_color(), alpha,
        palette().text, alpha,
    );
    let commit_text = action.confirm_button();
    let bt_w = bold.measure_width(commit_text, BUTTON_PX);
    let bt_x = commit_x + (commit_w as i32 - bt_w) / 2;
    let bt_y = commit_y + (BUTTON_H + BUTTON_PX as i32) / 2;
    bold.render(
        commit_text,
        BUTTON_PX,
        bt_x,
        bt_y,
        palette().dark_text,
        alpha,
        canvas,
        canvas_w,
        canvas_h,
    );

    // Borders / underlines from blend_pixel aren't needed; the
    // draw_rounded_box border above renders a 1px outline on focused
    // buttons.
    let _ = blend_pixel;
}

pub fn dispatch_action(action: Action) -> std::io::Result<std::process::Child> {
    use std::process::Command;
    match action {
        Action::Lock => Command::new("loginctl").arg("lock-session").spawn(),
        Action::Suspend => Command::new("systemctl").arg("suspend").spawn(),
        Action::Restart => Command::new("systemctl").arg("reboot").spawn(),
        Action::Shutdown => Command::new("systemctl").arg("poweroff").spawn(),
    }
}
