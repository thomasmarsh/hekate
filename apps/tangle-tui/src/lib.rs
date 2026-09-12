//! Character-cell terminal viewer for Tangle.
//!
//! This crate is the terminal backend on the shared presentation contract. It
//! never reads or mutates the kernel directly: [`TuiSession`] advances one
//! [`tangle_sim::Simulation`] only in whole fixed steps through the shared
//! `PresentationController`, and [`CellBackend`] renders the resulting
//! backend-agnostic `SceneFrame` into a colored character grid written to any
//! [`std::io::Write`] sink.
//!
//! `tangle-tui` is the only crate permitted to depend on a terminal library
//! (`crossterm`); the shared layer and the kernel stay terminal-free.

mod backend;
mod grid;
mod palette;
mod raster;
mod session;

pub use backend::{CellBackend, RunInfo};
pub use grid::{Cell, CellGrid};
pub use palette::{ColorDepth, Rgb, ansi256_color, to_ansi16, to_ansi256};
pub use raster::{
    BACKGROUND, BODY_COLOR, CELL_ASPECT, PATH_COLOR, PORTAL_COLOR, Rasterizer, SELECTED_COLOR,
    VECTOR_COLOR,
};
pub use session::TuiSession;
