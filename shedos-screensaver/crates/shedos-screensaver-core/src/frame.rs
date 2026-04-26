use crate::color::Color;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CellAttrs(pub u8);

impl CellAttrs {
    pub const NONE: Self = Self(0);
    pub const BOLD: Self = Self(1 << 0);
    pub const DIM: Self = Self(1 << 1);
    pub const ITALIC: Self = Self(1 << 2);
    pub const UNDERLINE: Self = Self(1 << 3);
    pub const REVERSE: Self = Self(1 << 4);

    pub fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    pub fn with(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub ch: char,
    pub fg: Color,
    pub bg: Color,
    pub attrs: CellAttrs,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: ' ',
            fg: Color::TEXT,
            bg: Color::BASE,
            attrs: CellAttrs::NONE,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Frame {
    rows: u16,
    cols: u16,
    cells: Vec<Cell>,
}

impl Frame {
    pub fn new(rows: u16, cols: u16) -> Self {
        let len = rows as usize * cols as usize;
        Self { rows, cols, cells: vec![Cell::default(); len] }
    }

    pub fn rows(&self) -> u16 {
        self.rows
    }

    pub fn cols(&self) -> u16 {
        self.cols
    }

    fn idx(&self, row: u16, col: u16) -> Option<usize> {
        if row < self.rows && col < self.cols {
            Some(row as usize * self.cols as usize + col as usize)
        } else {
            None
        }
    }

    pub fn get(&self, row: u16, col: u16) -> Option<&Cell> {
        self.idx(row, col).map(|i| &self.cells[i])
    }

    pub fn set(&mut self, row: u16, col: u16, cell: Cell) {
        if let Some(i) = self.idx(row, col) {
            self.cells[i] = cell;
        }
    }

    pub fn set_glyph(&mut self, row: u16, col: u16, ch: char, fg: Color) {
        if let Some(i) = self.idx(row, col) {
            self.cells[i].ch = ch;
            self.cells[i].fg = fg;
        }
    }

    pub fn fill(&mut self, cell: Cell) {
        self.cells.iter_mut().for_each(|c| *c = cell);
    }

    pub fn clear(&mut self) {
        self.fill(Cell::default());
    }

    pub fn cells(&self) -> impl Iterator<Item = (u16, u16, &Cell)> {
        let cols = self.cols;
        self.cells
            .iter()
            .enumerate()
            .map(move |(i, c)| ((i / cols as usize) as u16, (i % cols as usize) as u16, c))
    }

    /// Resize, preserving content within the overlap and clearing the rest.
    pub fn resize(&mut self, rows: u16, cols: u16) {
        if rows == self.rows && cols == self.cols {
            return;
        }
        let mut new = Frame::new(rows, cols);
        let r_max = self.rows.min(rows);
        let c_max = self.cols.min(cols);
        for r in 0..r_max {
            for c in 0..c_max {
                if let Some(cell) = self.get(r, c).copied() {
                    new.set(r, c, cell);
                }
            }
        }
        *self = new;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_frame_is_clear() {
        let f = Frame::new(3, 5);
        assert_eq!(f.rows(), 3);
        assert_eq!(f.cols(), 5);
        assert_eq!(f.get(0, 0).unwrap().ch, ' ');
        assert_eq!(f.get(2, 4).unwrap().ch, ' ');
        assert!(f.get(3, 0).is_none());
        assert!(f.get(0, 5).is_none());
    }

    #[test]
    fn set_and_get_roundtrip() {
        let mut f = Frame::new(2, 2);
        f.set_glyph(1, 1, 'X', Color::rgb(255, 0, 0));
        let c = f.get(1, 1).unwrap();
        assert_eq!(c.ch, 'X');
        assert_eq!(c.fg, Color::rgb(255, 0, 0));
    }

    #[test]
    fn out_of_bounds_set_is_silent() {
        let mut f = Frame::new(2, 2);
        f.set_glyph(99, 99, 'X', Color::TEXT);
        // No panic, no change.
        assert_eq!(f.get(0, 0).unwrap().ch, ' ');
    }

    #[test]
    fn resize_grows_clearing_extra() {
        let mut f = Frame::new(2, 2);
        f.set_glyph(0, 0, 'A', Color::TEXT);
        f.resize(3, 3);
        assert_eq!(f.rows(), 3);
        assert_eq!(f.cols(), 3);
        assert_eq!(f.get(0, 0).unwrap().ch, 'A');
        assert_eq!(f.get(2, 2).unwrap().ch, ' ');
    }

    #[test]
    fn resize_shrinks_truncating() {
        let mut f = Frame::new(3, 3);
        f.set_glyph(0, 0, 'A', Color::TEXT);
        f.set_glyph(2, 2, 'B', Color::TEXT);
        f.resize(2, 2);
        assert_eq!(f.get(0, 0).unwrap().ch, 'A');
        assert!(f.get(2, 2).is_none());
    }

    #[test]
    fn cells_iterator_visits_in_row_major_order() {
        let mut f = Frame::new(2, 3);
        f.set_glyph(0, 0, 'A', Color::TEXT);
        f.set_glyph(1, 2, 'F', Color::TEXT);
        let visited: Vec<(u16, u16, char)> = f.cells().map(|(r, c, cell)| (r, c, cell.ch)).collect();
        assert_eq!(visited[0], (0, 0, 'A'));
        assert_eq!(visited[5], (1, 2, 'F'));
    }

    #[test]
    fn cell_attrs_combine() {
        let a = CellAttrs::BOLD.with(CellAttrs::ITALIC);
        assert!(a.contains(CellAttrs::BOLD));
        assert!(a.contains(CellAttrs::ITALIC));
        assert!(!a.contains(CellAttrs::DIM));
    }
}
