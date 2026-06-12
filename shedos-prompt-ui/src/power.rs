//! Power-menu state, geometry, and hit-testing for the greeter and
//! lock prompt surfaces. Layout constants live here so render and
//! hit-test agree without sharing literals.

use crate::OutputRect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerAction {
    Suspend,
    Restart,
    Shutdown,
}

impl PowerAction {
    pub fn label(self) -> &'static str {
        match self {
            Self::Suspend => "Sleep",
            Self::Restart => "Restart",
            Self::Shutdown => "Shut down",
        }
    }

    pub fn all() -> &'static [PowerAction] {
        &[
            PowerAction::Suspend,
            PowerAction::Restart,
            PowerAction::Shutdown,
        ]
    }
}

#[derive(Debug, Clone, Default)]
pub struct PowerMenuState {
    pub open: bool,
    pub selected: usize,
    /// False until F12 or an arrow key engages keyboard navigation;
    /// stops a row from pre-highlighting after a mouse-open.
    pub kb_active: bool,
    pub pointer: Option<(f32, f32)>,
}

impl PowerMenuState {
    pub fn item_count(&self) -> usize {
        PowerAction::all().len()
    }
    pub fn current(&self) -> Option<PowerAction> {
        PowerAction::all().get(self.selected).copied()
    }
    pub fn select_next(&mut self) {
        let n = self.item_count();
        if n == 0 {
            return;
        }
        self.selected = (self.selected + 1) % n;
    }
    pub fn select_prev(&mut self) {
        let n = self.item_count();
        if n == 0 {
            return;
        }
        self.selected = if self.selected == 0 {
            n - 1
        } else {
            self.selected - 1
        };
    }
    pub fn clamp_selection(&mut self) {
        let n = self.item_count();
        if n == 0 {
            self.selected = 0;
        } else if self.selected >= n {
            self.selected = n - 1;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerHit {
    None,
    ToggleButton,
    Item(PowerAction),
}

pub const BTN_SIZE: i32 = 44;
pub const BTN_MARGIN: i32 = 48;
pub const MENU_W: u32 = 200;
pub const ITEM_H: i32 = 40;
pub const MENU_GAP: i32 = 10;
pub const MENU_RADIUS: u32 = 10;
pub const GLYPH_PX: f32 = 22.0;
pub const LABEL_PX: f32 = 18.0;

pub fn button_center(rect: &OutputRect) -> (i32, i32) {
    let bx = rect.x + rect.w - BTN_MARGIN - BTN_SIZE / 2;
    let by = rect.y + BTN_MARGIN + BTN_SIZE / 2;
    (bx, by)
}

pub fn menu_origin(rect: &OutputRect) -> (i32, i32) {
    let (bx, by) = button_center(rect);
    let right = bx + BTN_SIZE / 2;
    let menu_x = right - MENU_W as i32;
    let menu_y = by + BTN_SIZE / 2 + MENU_GAP;
    (menu_x, menu_y)
}

/// Map a click at canvas coordinate `(x, y)` to a power hit. Checks
/// every rect so the click is recognised on any monitor's mirror.
pub fn hit_test(state: &PowerMenuState, outputs: &[OutputRect], x: f32, y: f32) -> PowerHit {
    for rect in outputs {
        if let Some(hit) = hit_test_rect(state, rect, x, y) {
            return hit;
        }
    }
    PowerHit::None
}

fn hit_test_rect(state: &PowerMenuState, rect: &OutputRect, x: f32, y: f32) -> Option<PowerHit> {
    let (cx, cy) = button_center(rect);
    let r = (BTN_SIZE / 2) as f32;
    let dx = x - cx as f32;
    let dy = y - cy as f32;
    if dx * dx + dy * dy <= r * r {
        return Some(PowerHit::ToggleButton);
    }
    if state.open {
        let (mx, my) = menu_origin(rect);
        let menu_w = MENU_W as i32;
        let items = PowerAction::all();
        for (i, action) in items.iter().enumerate() {
            let top = my + (i as i32) * ITEM_H;
            if x >= mx as f32
                && x < (mx + menu_w) as f32
                && y >= top as f32
                && y < (top + ITEM_H) as f32
            {
                return Some(PowerHit::Item(*action));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect() -> OutputRect {
        OutputRect { x: 0, y: 0, w: 1920, h: 1080 }
    }

    #[test]
    fn actions_are_suspend_restart_shutdown() {
        assert_eq!(
            PowerAction::all(),
            &[PowerAction::Suspend, PowerAction::Restart, PowerAction::Shutdown]
        );
    }

    #[test]
    fn button_hit_on_top_right() {
        let st = PowerMenuState::default();
        let (cx, cy) = button_center(&rect());
        assert_eq!(hit_test(&st, &[rect()], cx as f32, cy as f32), PowerHit::ToggleButton);
    }

    #[test]
    fn miss_when_outside_and_closed() {
        let st = PowerMenuState::default();
        assert_eq!(hit_test(&st, &[rect()], 100.0, 100.0), PowerHit::None);
    }

    #[test]
    fn menu_first_item_hit_when_open() {
        let st = PowerMenuState { open: true, ..Default::default() };
        let (mx, my) = menu_origin(&rect());
        let hit = hit_test(&st, &[rect()], (mx + MENU_W as i32 / 2) as f32, (my + ITEM_H / 2) as f32);
        assert_eq!(hit, PowerHit::Item(PowerAction::Suspend));
    }

    #[test]
    fn menu_ignored_when_closed() {
        let st = PowerMenuState::default();
        let (mx, my) = menu_origin(&rect());
        let hit = hit_test(&st, &[rect()], (mx + MENU_W as i32 / 2) as f32, (my + ITEM_H / 2) as f32);
        assert_eq!(hit, PowerHit::None);
    }

    #[test]
    fn select_wraps() {
        let mut st = PowerMenuState::default();
        assert_eq!(st.selected, 0);
        st.select_next();
        assert_eq!(st.selected, 1);
        st.select_next();
        assert_eq!(st.selected, 2);
        st.select_next();
        assert_eq!(st.selected, 0, "wraps past the last action");
        st.select_prev();
        assert_eq!(st.selected, 2, "wraps backward to the last action");
    }
}
