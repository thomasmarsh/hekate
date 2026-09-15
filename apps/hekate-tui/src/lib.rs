//! Character-cell terminal viewer for Hekate.
//!
//! This crate is the terminal backend on the shared presentation contract. It
//! never reads or mutates the kernel directly: [`TuiSession`] advances one
//! [`hekate_sim::Simulation`] only in whole fixed steps through the shared
//! `PresentationController`, and [`CellBackend`] renders the resulting
//! backend-agnostic `SceneFrame` into a colored character grid written to any
//! [`std::io::Write`] sink.
//!
//! Backend selection is capability-gated: [`capability`] probes the terminal
//! and only ever selects the opt-in Kitty backend behind the bounded lifecycle
//! mandated by `DEC-002-terminal-backend-strategy`, and [`fallback`] restores
//! the terminal and continues in character cells if it fails.
//!
//! `hekate-tui` is the only crate permitted to depend on a terminal library
//! (`crossterm`); the shared layer and the kernel stay terminal-free.

mod backend;
pub mod capability;
pub mod fallback;
mod grid;
mod hud;
mod kitty;
mod palette;
mod pixel;
mod raster;
mod session;
pub mod terminal;

pub use backend::{CellBackend, RunInfo};
pub use capability::{
    BackendKind, BackendRequest, CapabilityResponder, CapabilityVerdict, Detection,
    EnvironmentHints, TerminalResponder,
};
pub use fallback::{BackendFallback, BackendPair};
pub use grid::{Cell, CellGrid};
pub use kitty::{
    EncodedFrame, KittyBackend, MAX_CHUNK, Multiplexer, SCREEN_MAX_SEQUENCE, apc, base64_encode,
    delete_image, double_esc, encode, screen_wrap, tmux_wrap,
};
pub use palette::{ColorDepth, Rgb, ansi256_color, to_ansi16, to_ansi256};
pub use pixel::{PixelRasterizer, RgbaImage};
pub use raster::{
    BACKGROUND, BODY_COLOR, CELL_ASPECT, PATH_COLOR, PORTAL_COLOR, Rasterizer, SELECTED_COLOR,
    VECTOR_COLOR,
};
pub use session::{SessionBackend, TuiSession};
pub use terminal::{RealTerminal, TerminalModes};
