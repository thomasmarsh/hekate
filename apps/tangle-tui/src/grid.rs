//! A character-cell surface and its ANSI serialization.
//!
//! A [`CellGrid`] is the terminal analogue of a pixel buffer: one glyph plus a
//! foreground and background color per cell. [`CellGrid::write_ansi`] emits it
//! through any [`Write`] sink, coalescing identical color runs so a mostly flat
//! frame stays cheap to transmit, and degrading to 256- or 16-color output when
//! the terminal cannot render truecolor.

use std::io::{self, Write};

use crate::palette::{ColorDepth, Rgb};

/// One character cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    /// The glyph drawn in the cell.
    pub ch: char,
    /// Foreground (glyph) color.
    pub fg: Rgb,
    /// Background color.
    pub bg: Rgb,
}

impl Cell {
    /// A space on `bg` with `fg` as its foreground.
    pub const fn new(ch: char, fg: Rgb, bg: Rgb) -> Self {
        Self { ch, fg, bg }
    }

    /// A blank cell on `bg`.
    pub const fn blank(bg: Rgb) -> Self {
        Self::new(' ', bg, bg)
    }
}

/// A rectangular grid of [`Cell`]s in row-major order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellGrid {
    width: u32,
    height: u32,
    cells: Vec<Cell>,
}

impl CellGrid {
    /// A grid of `width` by `height` cells, every one set to `fill`.
    pub fn new(width: u32, height: u32, fill: Cell) -> Self {
        Self {
            width,
            height,
            cells: vec![fill; (width as usize) * (height as usize)],
        }
    }

    /// Grid width in columns.
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Grid height in rows.
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// The cell at `col`, `row`, or `None` when out of bounds.
    pub fn get(&self, col: i64, row: i64) -> Option<&Cell> {
        self.index(col, row).map(|index| &self.cells[index])
    }

    /// Overwrite the cell at `col`, `row`. Out-of-bounds writes are ignored,
    /// so callers can clip lines without checking each endpoint.
    pub fn put(&mut self, col: i64, row: i64, cell: Cell) {
        if let Some(index) = self.index(col, row) {
            self.cells[index] = cell;
        }
    }

    /// Fill every cell with `fill`.
    pub fn fill(&mut self, fill: Cell) {
        self.cells.fill(fill);
    }

    fn index(&self, col: i64, row: i64) -> Option<usize> {
        if col < 0 || row < 0 || col >= i64::from(self.width) || row >= i64::from(self.height) {
            return None;
        }
        Some(row as usize * self.width as usize + col as usize)
    }

    /// Serialize the grid as full-width ANSI lines separated by `\r\n` and with
    /// no trailing newline, so a caller can append its own row after it.
    pub fn write_ansi<W: Write>(&self, out: &mut W, depth: ColorDepth) -> io::Result<()> {
        let mut active: Option<(Rgb, Rgb)> = None;
        for row in 0..self.height {
            if row > 0 {
                out.write_all(b"\r\n")?;
            }
            for col in 0..self.width {
                let cell = &self.cells[row as usize * self.width as usize + col as usize];
                if active != Some((cell.fg, cell.bg)) {
                    write!(
                        out,
                        "\x1b[{};{}m",
                        depth.foreground(cell.fg),
                        depth.background(cell.bg)
                    )?;
                    active = Some((cell.fg, cell.bg));
                }
                let mut buffer = [0u8; 4];
                out.write_all(cell.ch.encode_utf8(&mut buffer).as_bytes())?;
            }
        }
        Ok(())
    }

    /// The grid serialized to a [`String`] at `depth`, for tests and goldens.
    pub fn to_ansi(&self, depth: ColorDepth) -> String {
        let mut bytes = Vec::new();
        self.write_ansi(&mut bytes, depth)
            .expect("writing to a Vec cannot fail");
        String::from_utf8(bytes).expect("ANSI output is UTF-8")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blank_grid_serializes_to_its_width_and_height() {
        let mut grid = CellGrid::new(3, 2, Cell::blank(Rgb::BLACK));
        grid.put(1, 0, Cell::new('x', Rgb::WHITE, Rgb::BLACK));
        let text = grid.to_ansi(ColorDepth::Ansi16);
        // Two rows, one line break, and no trailing newline.
        assert_eq!(text.matches("\r\n").count(), 1);
        assert!(!text.ends_with("\r\n"));
        assert!(text.contains('x'));
    }

    #[test]
    fn out_of_bounds_writes_are_clipped() {
        let mut grid = CellGrid::new(2, 2, Cell::blank(Rgb::BLACK));
        grid.put(-1, 0, Cell::new('a', Rgb::WHITE, Rgb::BLACK));
        grid.put(0, 5, Cell::new('b', Rgb::WHITE, Rgb::BLACK));
        grid.put(2, 0, Cell::new('c', Rgb::WHITE, Rgb::BLACK));
        assert!(grid.get(-1, 0).is_none());
        assert!(grid.get(0, 5).is_none());
        assert!(grid.get(2, 0).is_none());
    }

    #[test]
    fn color_runs_are_coalesced() {
        let grid = CellGrid::new(4, 1, Cell::new('#', Rgb::WHITE, Rgb::BLACK));
        let text = grid.to_ansi(ColorDepth::Truecolor);
        // One SGR prefix for the whole run, not one per cell.
        assert_eq!(text.matches("\x1b[").count(), 1);
    }

    #[test]
    fn lower_depths_never_emit_truecolor_sequences() {
        let grid = CellGrid::new(1, 1, Cell::new('#', Rgb::new(1, 2, 3), Rgb::BLACK));
        let ansi256 = grid.to_ansi(ColorDepth::Ansi256);
        let ansi16 = grid.to_ansi(ColorDepth::Ansi16);
        assert!(ansi256.contains("38;5;"));
        assert!(!ansi256.contains("38;2;"));
        assert!(!ansi16.contains("38;2;"));
        assert!(!ansi16.contains("38;5;"));
    }
}
