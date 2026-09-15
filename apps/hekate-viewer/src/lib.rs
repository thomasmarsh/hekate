//! Bevy side of the Hekate viewer.
//!
//! [`CurrentFrame`] is the Bevy graphics backend: it implements
//! [`hekate_present::RendererBackend`], capturing each projected [`SceneFrame`]
//! so the ECS render systems draw from the shared presentation contract rather
//! than from kernel state. The ECS is responsible for actually presenting the
//! captured frame to the window.

use bevy::prelude::{Quat, Resource, Transform, Vec3};
use glam::DVec2;
use hekate_present::{
    BackendCapabilities, BackendResult, BodyShape, RendererBackend, SceneBody, SceneFrame,
};

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

/// Shared unit mesh a rendered body shape draws with.
///
/// Both meshes are unit-sized and scaled by the shape's world transform, so a
/// body reuses one mesh handle instead of allocating per agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyMesh {
    /// A unit rectangle scaled to the shape's length and width.
    Box,
    /// A unit-diameter circle scaled to the shape's diameter.
    Circle,
}

/// One body shape resolved to the shared mesh and world transform a Bevy
/// entity renders it with.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyVisual {
    /// Shared mesh this shape draws with.
    pub mesh: BodyMesh,
    /// World transform: translation in the world plane, a heading rotation, and
    /// a length-by-width scale.
    pub transform: Transform,
}

/// Resolve a scene body to the visuals the viewer renders.
///
/// The shape decision is the backend-independent [`SceneBody::shapes`], so the
/// viewer draws a vehicle box, a pedestrian circle, and a narrow wheeled capsule
/// by the body's reported kind, and one entity per ordered segment, without a
/// branch on mode or scenario. One shared shape contributes one visual, except a
/// capsule: its straight part and its cap radius scale independently, so a
/// scaled unit mesh would draw elliptical caps. A capsule therefore draws the
/// rectangle its segment spans and a circle on each cap, the exact shapes the
/// viewer already has meshes for.
pub fn body_visuals(body: &SceneBody) -> Vec<BodyVisual> {
    let mut visuals = Vec::new();
    for shape in body.shapes() {
        match shape {
            BodyShape::Box {
                center,
                heading_rad,
                length_m,
                width_m,
            } => visuals.push(box_visual(center, heading_rad, length_m, width_m)),
            BodyShape::Circle { center, radius_m } => {
                visuals.push(circle_visual(center, radius_m));
            }
            BodyShape::Capsule {
                center,
                heading_rad,
                length_m,
                radius_m,
            } => {
                visuals.push(box_visual(center, heading_rad, length_m, radius_m * 2.0));
                let (sin, cos) = heading_rad.sin_cos();
                let forward = DVec2::new(cos, sin) * (length_m * 0.5);
                visuals.push(circle_visual(center + forward, radius_m));
                visuals.push(circle_visual(center - forward, radius_m));
            }
        }
    }
    visuals
}

/// A box visual: the unit rectangle scaled to the shape's length and width.
fn box_visual(center: DVec2, heading_rad: f64, length_m: f64, width_m: f64) -> BodyVisual {
    BodyVisual {
        mesh: BodyMesh::Box,
        transform: Transform {
            translation: Vec3::new(center.x as f32, center.y as f32, 1.0),
            rotation: Quat::from_rotation_z(heading_rad as f32),
            scale: Vec3::new(length_m as f32, width_m as f32, 1.0),
        },
    }
}

/// A circle visual: the unit-diameter circle scaled to the shape's diameter.
fn circle_visual(center: DVec2, radius_m: f64) -> BodyVisual {
    BodyVisual {
        mesh: BodyMesh::Circle,
        transform: Transform {
            translation: Vec3::new(center.x as f32, center.y as f32, 1.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::new((radius_m * 2.0) as f32, (radius_m * 2.0) as f32, 1.0),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use hekate_model::BodyKind;
    use hekate_sim::{AgentMode, BodySegmentSample};

    /// A scene body with no segments, for the single-envelope shapes.
    fn body(
        id: usize,
        body_kind: BodyKind,
        position: DVec2,
        length_m: f64,
        width_m: f64,
    ) -> SceneBody {
        SceneBody {
            id,
            position,
            heading_rad: 0.0,
            length_m,
            width_m,
            mode: AgentMode::Vehicle,
            body_kind,
            segments: Vec::new(),
            speed_mps: Some(0.0),
            path: None,
            path_distance_m: None,
            route: None,
            profile: None,
            decision: None,
        }
    }

    /// The viewer's shape selection follows the body's kind and ordered
    /// segments, with no branch on mode or scenario: a vehicle box draws a box
    /// scaled to its extent, a pedestrian circle draws a circle scaled to its
    /// diameter, and a segmented body draws one entity per segment at the
    /// segment's own pose.
    #[test]
    fn a_body_selects_its_mesh_from_kind_and_draws_a_segment_each() {
        let vehicle = body(0, BodyKind::Box, DVec2::new(1.0, 2.0), 4.5, 1.8);
        let visuals = body_visuals(&vehicle);
        assert_eq!(visuals.len(), 1);
        assert_eq!(visuals[0].mesh, BodyMesh::Box);
        assert_eq!(visuals[0].transform.translation, Vec3::new(1.0, 2.0, 1.0));
        assert_eq!(visuals[0].transform.scale, Vec3::new(4.5, 1.8, 1.0));

        let pedestrian = body(1, BodyKind::Circle, DVec2::ZERO, 0.5, 0.5);
        let visuals = body_visuals(&pedestrian);
        assert_eq!(visuals.len(), 1);
        assert_eq!(visuals[0].mesh, BodyMesh::Circle);
        assert_eq!(visuals[0].transform.scale, Vec3::new(0.5, 0.5, 1.0));

        let mut articulated = body(2, BodyKind::ArticulatedChain, DVec2::ZERO, 6.0, 2.0);
        articulated.segments = vec![
            BodySegmentSample {
                position: DVec2::new(0.0, 0.0),
                heading_rad: 0.0,
            },
            BodySegmentSample {
                position: DVec2::new(3.0, 0.0),
                heading_rad: 0.1,
            },
        ];
        let visuals = body_visuals(&articulated);
        assert_eq!(visuals.len(), 2);
        assert!(visuals.iter().all(|visual| visual.mesh == BodyMesh::Box));
        assert_eq!(visuals[0].transform.translation, Vec3::new(0.0, 0.0, 1.0));
        assert_eq!(visuals[0].transform.scale, Vec3::new(3.0, 2.0, 1.0));
        assert_eq!(visuals[1].transform.translation, Vec3::new(3.0, 0.0, 1.0));
    }

    /// A narrow wheeled capsule draws its straight part and both caps: the box
    /// between the cap centres and one circle per cap, so the drawn envelope is
    /// the capsule rather than the rectangle that bounds it.
    #[test]
    fn a_capsule_body_draws_its_straight_part_and_both_caps() {
        let capsule = body(3, BodyKind::Capsule, DVec2::new(1.0, 2.0), 1.8, 0.7);
        let visuals = body_visuals(&capsule);
        assert_eq!(visuals.len(), 3);

        assert_eq!(visuals[0].mesh, BodyMesh::Box);
        assert_eq!(visuals[0].transform.translation, Vec3::new(1.0, 2.0, 1.0));
        // The straight part is the reported length by the diameter.
        assert_eq!(visuals[0].transform.scale, Vec3::new(1.8, 0.7, 1.0));

        // Each cap sits half the straight length from the centre, with the
        // half-width as its radius. The centres are compared with a tolerance
        // because the viewer carries world metres as `f32`.
        for (visual, offset) in visuals[1..].iter().zip([0.9_f32, -0.9_f32]) {
            assert_eq!(visual.mesh, BodyMesh::Circle);
            assert!(
                (visual.transform.translation.x - (1.0 + offset)).abs() < 1e-6,
                "cap x is {} for offset {offset}",
                visual.transform.translation.x
            );
            assert_eq!(visual.transform.translation.y, 2.0);
            assert_eq!(visual.transform.scale, Vec3::new(0.7, 0.7, 1.0));
        }
    }
}
