//! Bevy side of the Tangle viewer.
//!
//! [`CurrentFrame`] is the Bevy graphics backend: it implements
//! [`tangle_present::RendererBackend`], capturing each projected [`SceneFrame`]
//! so the ECS render systems draw from the shared presentation contract rather
//! than from kernel state. The ECS is responsible for actually presenting the
//! captured frame to the window.

use bevy::prelude::Resource;
use tangle_present::{BackendCapabilities, BackendResult, RendererBackend, SceneFrame};

/// The most recent frame the shared presentation layer projected, as captured
/// by the Bevy backend's [`RendererBackend::draw`].
#[derive(Resource, Default)]
pub struct CurrentFrame(Option<SceneFrame>);

impl CurrentFrame {
    /// Wrap an already projected frame, so the first update already has one.
    pub fn new(frame: SceneFrame) -> Self {
        Self(Some(frame))
    }

    /// The captured frame, if one has been drawn.
    pub const fn get(&self) -> Option<&SceneFrame> {
        self.0.as_ref()
    }

    /// Reflect a selection change applied to the controller after this frame
    /// was drawn, so the highlight appears on the same tick as the click.
    pub fn set_selection(&mut self, selection: Option<usize>) {
        if let Some(frame) = self.0.as_mut() {
            frame.status.selection = selection;
        }
    }
}

impl RendererBackend for CurrentFrame {
    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::GPU
    }

    fn resize(&mut self, _width: u32, _height: u32) -> BackendResult {
        // The Bevy camera and window own sizing; nothing to do here.
        Ok(())
    }

    fn draw(&mut self, frame: &SceneFrame) -> BackendResult {
        // Capture the frame; the ECS systems read it back as the present stage.
        self.0 = Some(frame.clone());
        Ok(())
    }

    fn present(&mut self) -> BackendResult {
        // Window presentation is driven by Bevy's render schedule.
        Ok(())
    }
}
