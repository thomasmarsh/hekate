//! Reusable, Bevy-free pieces of the Tangle viewer.
//!
//! The binary owns the Bevy app and rendering presentation. Everything that can
//! be reasoned about without a window or GPU — loading a scenario and deciding
//! how many whole kernel steps a frame of wall time buys — lives here so it can
//! be tested headlessly.

pub mod presentation;
pub mod scenario;

pub use presentation::{MAX_TICKS_PER_FRAME, PresentationClock, Speed};
pub use scenario::{LoadError, load_scenario};
