//! Bevy-free, terminal-free presentation layer shared by every Tangle renderer.
//!
//! The kernel owns the authoritative clock and simulation state. This crate
//! owns everything between it and a renderer: reading a scenario from disk, the
//! presentation clock, the viewport, selection and overlays, and the pure
//! projection of kernel snapshots into a backend-agnostic [`SceneFrame`]. A
//! renderer implements [`RendererBackend`] and consumes that frame; it never
//! reads or mutates the simulation directly, so no backend can change which
//! ticks the kernel visits.
//!
//! The one-way dependency direction is enforced in CI by
//! `scripts/check-dependency-direction.sh`: this crate must not depend on Bevy,
//! a window, a terminal library, or an application crate, and the kernel crates
//! must not depend on it.

mod backend;
mod clock;
mod command;
mod controller;
mod scenario;
mod scene;

pub use backend::{BackendCapabilities, BackendResult, RendererBackend};
pub use clock::{MAX_TICKS_PER_FRAME, PresentationClock, Speed};
pub use command::{RestartMode, ViewCommand};
pub use controller::{Applied, PresentationController};
pub use scenario::{LoadError, load_scenario};
pub use scene::{
    BodyKind, DEFAULT_BODY_LENGTH_M, DEFAULT_BODY_WIDTH_M, FrameStatus, Overlay, Overlays,
    SELECT_RADIUS_PIXELS, SceneBody, SceneFrame, SceneGeometry, ScenePath, ScenePortal, Viewport,
};
