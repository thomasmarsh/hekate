//! Backend-independent input commands.
//!
//! Every backend translates its device events into a [`ViewCommand`] stream.
//! The presentation controller applies commands identically, so backend choice
//! cannot change the playback clock or which kernel ticks are visited.

use glam::DVec2;

use crate::clock::Speed;
use crate::scene::Overlay;

/// Which seed a restart should use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartMode {
    /// Re-run the same scenario and seed.
    SameSeed,
    /// Re-run the same scenario with the next seed.
    NextSeed,
}

/// A normalized viewer command.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ViewCommand {
    /// Flip pause and play.
    TogglePause,
    /// Set pause explicitly.
    SetPaused(bool),
    /// Advance exactly one tick on the next frame.
    SingleStep,
    /// Select a playback speed.
    SetSpeed(Speed),
    /// Restart the simulation with the same or next seed.
    Restart(RestartMode),
    /// Move the viewport centre by a world-space delta.
    Pan(DVec2),
    /// Multiply the viewport zoom by a factor.
    Zoom(f64),
    /// Select the body nearest a world-space point, if one is close enough.
    SelectNearest(DVec2),
    /// Clear the current selection.
    ClearSelection,
    /// Flip one debug overlay.
    ToggleOverlay(Overlay),
}
