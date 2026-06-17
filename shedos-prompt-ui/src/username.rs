//! Username dropdown for the greeter and lock prompt — a floating
//! overlay sibling of the power menu (power.rs).

use crate::users::User;
use crate::OutputRect;

#[derive(Debug, Clone, Default)]
pub struct UsernameMenuState {
    pub users: Vec<User>,
    pub selected: usize,
    pub open: bool,
    /// Set once an arrow key is pressed, so the list doesn't pre-highlight
    /// a row after a mouse-open.
    pub kb_active: bool,
    pub pointer: Option<(f32, f32)>,
}

impl UsernameMenuState {
    pub fn item_count(&self) -> usize {
        self.users.len()
    }
    pub fn current(&self) -> Option<&User> {
        self.users.get(self.selected)
    }
    pub fn selected_name(&self) -> Option<&str> {
        self.current().map(|u| u.name.as_str())
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
        self.selected = if self.selected == 0 { n - 1 } else { self.selected - 1 };
    }
    pub fn clamp_selection(&mut self) {
        let n = self.item_count();
        if n == 0 {
            self.selected = 0;
        } else if self.selected >= n {
            self.selected = n - 1;
        }
    }
    pub fn select_by_name(&mut self, name: &str) {
        if let Some(idx) = self.users.iter().position(|u| u.name == name) {
            self.selected = idx;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsernameHit {
    None,
    Field,
    Item(usize),
}

pub const FIELD_W: u32 = 300;
pub const FIELD_H: i32 = 36;
pub const ITEM_H: i32 = 36;
pub const MENU_RADIUS: u32 = 10;
pub const LABEL_PX: f32 = 18.0;

/// Field top-left, centered above the password box (drawn at 0.58 of the
/// rect height).
pub fn field_origin(rect: &OutputRect) -> (i32, i32) {
    let fx = rect.x + (rect.w - FIELD_W as i32) / 2;
    let box_y = rect.y + (rect.h as f32 * 0.58) as i32;
    let fy = box_y - FIELD_H - 16;
    (fx, fy)
}

pub fn menu_origin(rect: &OutputRect) -> (i32, i32) {
    let (fx, fy) = field_origin(rect);
    (fx, fy + FIELD_H + 4)
}

pub fn hit_test(
    state: &UsernameMenuState,
    outputs: &[OutputRect],
    x: f32,
    y: f32,
) -> UsernameHit {
    for rect in outputs {
        if let Some(hit) = hit_test_rect(state, rect, x, y) {
            return hit;
        }
    }
    UsernameHit::None
}

fn hit_test_rect(
    state: &UsernameMenuState,
    rect: &OutputRect,
    x: f32,
    y: f32,
) -> Option<UsernameHit> {
    let (fx, fy) = field_origin(rect);
    let fw = FIELD_W as i32;
    if x >= fx as f32 && x < (fx + fw) as f32 && y >= fy as f32 && y < (fy + FIELD_H) as f32 {
        return Some(UsernameHit::Field);
    }
    if state.open {
        let (mx, my) = menu_origin(rect);
        for i in 0..state.users.len() {
            let top = my + (i as i32) * ITEM_H;
            if x >= mx as f32
                && x < (mx + fw) as f32
                && y >= top as f32
                && y < (top + ITEM_H) as f32
            {
                return Some(UsernameHit::Item(i));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::users::User;

    fn rect() -> OutputRect {
        OutputRect { x: 0, y: 0, w: 1920, h: 1080 }
    }

    fn three_users() -> Vec<User> {
        vec![
            User { name: "alice".into(), uid: 1000 },
            User { name: "bob".into(), uid: 1001 },
            User { name: "carol".into(), uid: 1002 },
        ]
    }

    #[test]
    fn field_hit_when_closed() {
        let st = UsernameMenuState { users: three_users(), ..Default::default() };
        let (fx, fy) = field_origin(&rect());
        let hit = hit_test(&st, &[rect()], (fx + FIELD_W as i32 / 2) as f32, (fy + FIELD_H / 2) as f32);
        assert_eq!(hit, UsernameHit::Field);
    }

    #[test]
    fn miss_outside_field_when_closed() {
        let st = UsernameMenuState { users: three_users(), ..Default::default() };
        assert_eq!(hit_test(&st, &[rect()], 5.0, 5.0), UsernameHit::None);
    }

    #[test]
    fn first_item_hit_when_open() {
        let st = UsernameMenuState { users: three_users(), open: true, ..Default::default() };
        let (mx, my) = menu_origin(&rect());
        let hit = hit_test(&st, &[rect()], (mx + FIELD_W as i32 / 2) as f32, (my + ITEM_H / 2) as f32);
        assert_eq!(hit, UsernameHit::Item(0));
    }

    #[test]
    fn list_ignored_when_closed() {
        let st = UsernameMenuState { users: three_users(), ..Default::default() };
        let (mx, my) = menu_origin(&rect());
        let hit = hit_test(&st, &[rect()], (mx + FIELD_W as i32 / 2) as f32, (my + ITEM_H / 2) as f32);
        assert_eq!(hit, UsernameHit::None);
    }

    #[test]
    fn select_wraps_both_ways() {
        let mut st = UsernameMenuState { users: three_users(), ..Default::default() };
        assert_eq!(st.selected, 0);
        st.select_next();
        st.select_next();
        assert_eq!(st.selected, 2);
        st.select_next();
        assert_eq!(st.selected, 0, "wraps forward");
        st.select_prev();
        assert_eq!(st.selected, 2, "wraps backward");
    }

    #[test]
    fn select_by_name_highlights_regardless_of_position() {
        let mut st = UsernameMenuState { users: three_users(), ..Default::default() };
        st.select_by_name("carol");
        assert_eq!(st.selected, 2);
        assert_eq!(st.selected_name(), Some("carol"));
        st.select_by_name("missing");
        assert_eq!(st.selected, 2, "unknown name leaves selection unchanged");
    }

    #[test]
    fn selected_name_none_when_empty() {
        let st = UsernameMenuState::default();
        assert_eq!(st.selected_name(), None);
        let mut st = st;
        st.select_next();
        assert_eq!(st.selected, 0, "no-op on empty list");
    }
}
