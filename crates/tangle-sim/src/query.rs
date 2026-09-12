//! Exact narrow-phase geometry queries over the body shapes the kernel models.
//!
//! # Model card
//!
//! `PHASE_1_PLAN.md` Increment 4 ("Exact box/box, circle/circle, and box/circle
//! distance and intersection queries") needs one place that answers "how far
//! apart are these two bodies, and do they overlap?" for the two shapes the
//! simulator uses. A vehicle is an oriented box, a pedestrian a circle; this
//! module owns those shapes and the exact query between them. The uniform-grid
//! broad phase that feeds candidate pairs to these queries lives in
//! [`crate::index`].
//!
//! ## State
//!
//! [`BodyShape`] is the smallest description of a body needed for a static
//! query: a circle has a centre and a radius, an oriented box has a centre, a
//! heading, and a length and width along its heading and left axes. Both shapes
//! are convex and closed. The kernel's [`AgentStore`](crate::agent::AgentStore)
//! maps onto them through [`agent_body`]: a pedestrian's radius is half its
//! reported body length, matching the crossing-occupancy test, and a vehicle's
//! box is its reported length and width at its heading.
//!
//! ## Query semantics
//!
//! [`body_clearance_m`] returns the signed surface clearance in metres between
//! two closed shapes:
//!
//! - positive when the shapes are disjoint, and equal to the exact distance
//!   between them;
//! - negative when they overlap, with magnitude the exact minimum translation
//!   distance that separates them (the penetration depth);
//! - zero when they touch.
//!
//! [`bodies_intersect`] is exactly `body_clearance_m(first, second) < 0.0`.
//! Both queries are symmetric in their arguments.
//!
//! Circle/circle and circle/box are computed analytically. Box/box uses the
//! separating-axis test: two convex boxes are disjoint exactly when one of
//! their four face-normal axes separates them, in which case the distance is
//! the closest vertex-to-box distance; otherwise the penetration depth is the
//! least projection overlap.
//!
//! ## Tolerance
//!
//! The projection arithmetic of the separating-axis test can read a touching
//! pair a few ulps negative. A least projection overlap no larger than
//! [`CONTACT_EPSILON_M`] therefore reads as touching (`0.0`) rather than a
//! sub-nanometre penetration. That absorbs only floating-point noise at exact
//! contact, never a physical overlap, and it is the only tolerance the queries
//! apply.
//!
//! ## Determinism
//!
//! Every query is a pure function of its two shapes: no random draws, no
//! iteration order, and no global state, so the same shapes always give the
//! same clearance and the same intersection result.

use glam::DVec2;

use crate::agent::{AgentMode, AgentStore};

/// A sub-nanometre band at exact box/box contact, in metres.
///
/// A projection overlap no larger than this reads as touching (`0.0`); see the
/// module tolerance note.
pub const CONTACT_EPSILON_M: f64 = 1e-9;

/// An axis-aligned bounding box in world metres.
///
/// `min` is componentwise no greater than `max`; [`Aabb::new`] enforces that,
/// so a caller may construct one from any two corners.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    /// Lower corner, in metres.
    pub min: DVec2,
    /// Upper corner, in metres.
    pub max: DVec2,
}

impl Aabb {
    /// The box spanning `min` and `max`, ordering the corners componentwise.
    pub fn new(min: DVec2, max: DVec2) -> Self {
        Self {
            min: min.min(max),
            max: min.max(max),
        }
    }

    /// The box of the given half-extent around `centre`, in metres.
    pub fn from_centre_half_extent(centre: DVec2, half_extent: DVec2) -> Self {
        Self {
            min: centre - half_extent,
            max: centre + half_extent,
        }
    }

    /// The box centre, in metres.
    pub fn centre(&self) -> DVec2 {
        (self.min + self.max) * 0.5
    }

    /// The half-extent from the centre to a corner, in metres.
    pub fn half_extent(&self) -> DVec2 {
        (self.max - self.min) * 0.5
    }

    /// Whether the two closed boxes share at least one point.
    pub fn overlaps(&self, other: &Aabb) -> bool {
        self.min.x <= other.max.x
            && other.min.x <= self.max.x
            && self.min.y <= other.max.y
            && other.min.y <= self.max.y
    }

    /// The box grown by `margin` metres on every side.
    pub fn expand(&self, margin: f64) -> Aabb {
        let margin = DVec2::splat(margin);
        Aabb {
            min: self.min - margin,
            max: self.max + margin,
        }
    }
}

/// A convex body shape: an oriented box (a vehicle) or a circle (a pedestrian).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BodyShape {
    /// A circle body, centred at `centre` with `radius_m`.
    Circle {
        /// Centre in world metres.
        centre: DVec2,
        /// Radius in metres.
        radius_m: f64,
    },
    /// An oriented box body, centred at `centre`.
    Box {
        /// Centre in world metres.
        centre: DVec2,
        /// Heading of the length axis in radians.
        heading_rad: f64,
        /// Full length along the heading axis, in metres.
        length_m: f64,
        /// Full width along the left axis, in metres.
        width_m: f64,
    },
}

impl BodyShape {
    /// The body centre, in world metres.
    pub fn centre(&self) -> DVec2 {
        match *self {
            Self::Circle { centre, .. } | Self::Box { centre, .. } => centre,
        }
    }

    /// The tight axis-aligned box around the body, in world metres.
    pub fn bounds(&self) -> Aabb {
        match *self {
            Self::Circle { centre, radius_m } => {
                Aabb::from_centre_half_extent(centre, DVec2::splat(radius_m))
            }
            Self::Box {
                centre,
                heading_rad,
                length_m,
                width_m,
            } => {
                let (sin, cos) = heading_rad.sin_cos();
                let half_extent = DVec2::new(
                    (cos.abs() * length_m + sin.abs() * width_m) * 0.5,
                    (sin.abs() * length_m + cos.abs() * width_m) * 0.5,
                );
                Aabb::from_centre_half_extent(centre, half_extent)
            }
        }
    }

    /// The radius of the smallest centred circle that contains the body, in
    /// metres.
    ///
    /// The broad phase widens a query by the largest circumradius among the
    /// indexed bodies, because a body's centre may sit up to this far outside a
    /// region its extents still reach into.
    pub fn circumradius_m(&self) -> f64 {
        match *self {
            Self::Circle { radius_m, .. } => radius_m,
            Self::Box {
                length_m, width_m, ..
            } => (length_m * length_m + width_m * width_m).sqrt() * 0.5,
        }
    }
}

/// The body shape of one agent slot, from the shared store.
///
/// A pedestrian is a circle of half its reported body length, matching the
/// crossing-occupancy test; a vehicle is its reported length and width at its
/// heading. Both modes use the same mapping, so one broad phase covers them.
pub(crate) fn agent_body(agents: &AgentStore, index: usize) -> BodyShape {
    let centre = agents.position[index];
    match agents.mode[index] {
        AgentMode::Pedestrian => BodyShape::Circle {
            centre,
            radius_m: agents.body_length_m[index] * 0.5,
        },
        AgentMode::Vehicle => BodyShape::Box {
            centre,
            heading_rad: agents.heading_rad[index],
            length_m: agents.body_length_m[index],
            width_m: agents.body_width_m[index],
        },
    }
}

/// The signed surface clearance between two bodies, in metres.
///
/// Positive when disjoint and equal to the exact distance, negative when
/// overlapping and equal to `-penetration_depth`, and zero when touching. The
/// result is symmetric: `body_clearance_m(a, b) == body_clearance_m(b, a)`.
pub fn body_clearance_m(first: &BodyShape, second: &BodyShape) -> f64 {
    match (first, second) {
        (
            BodyShape::Circle {
                centre: first_centre,
                radius_m: first_radius,
            },
            BodyShape::Circle {
                centre: second_centre,
                radius_m: second_radius,
            },
        ) => (*second_centre - *first_centre).length() - *first_radius - *second_radius,
        (BodyShape::Circle { centre, radius_m }, box_body @ BodyShape::Box { .. }) => {
            circle_box_clearance_m(*centre, *radius_m, box_body)
        }
        (box_body @ BodyShape::Box { .. }, BodyShape::Circle { centre, radius_m }) => {
            circle_box_clearance_m(*centre, *radius_m, box_body)
        }
        (first @ BodyShape::Box { .. }, second @ BodyShape::Box { .. }) => {
            box_box_clearance_m(first, second)
        }
    }
}

/// Whether two bodies overlap, within the query tolerance.
///
/// Exactly `body_clearance_m(first, second) < 0.0`, so a pair touching within
/// [`CONTACT_EPSILON_M`] counts as touching rather than overlapping.
pub fn bodies_intersect(first: &BodyShape, second: &BodyShape) -> bool {
    body_clearance_m(first, second) < 0.0
}

/// A body-local box frame: centre, unit axes, and half extents.
struct BoxFrame {
    centre: DVec2,
    forward: DVec2,
    left: DVec2,
    half_length: f64,
    half_width: f64,
}

/// The frame of a box body. Panics if called on a non-box.
fn box_frame(body: &BodyShape) -> BoxFrame {
    let BodyShape::Box {
        centre,
        heading_rad,
        length_m,
        width_m,
    } = *body
    else {
        unreachable!("box_frame is only called for a box body");
    };
    let (sin, cos) = heading_rad.sin_cos();
    BoxFrame {
        centre,
        forward: DVec2::new(cos, sin),
        left: DVec2::new(-sin, cos),
        half_length: length_m * 0.5,
        half_width: width_m * 0.5,
    }
}

/// The four corners of a box frame in ring order, in world metres.
fn box_corners(frame: &BoxFrame) -> [DVec2; 4] {
    let along = frame.forward * frame.half_length;
    let across = frame.left * frame.half_width;
    [
        frame.centre + along + across,
        frame.centre + along - across,
        frame.centre - along - across,
        frame.centre - along + across,
    ]
}

/// Distance in metres from a point to the closed box frame.
fn point_to_box_distance(point: DVec2, frame: &BoxFrame) -> f64 {
    let offset = point - frame.centre;
    let local = DVec2::new(offset.dot(frame.forward), offset.dot(frame.left));
    let clamped = DVec2::new(
        local.x.clamp(-frame.half_length, frame.half_length),
        local.y.clamp(-frame.half_width, frame.half_width),
    );
    (local - clamped).length()
}

/// Signed clearance in metres between a circle and a box.
fn circle_box_clearance_m(centre: DVec2, radius_m: f64, box_body: &BodyShape) -> f64 {
    let frame = box_frame(box_body);
    let offset = centre - frame.centre;
    let local = DVec2::new(offset.dot(frame.forward), offset.dot(frame.left));
    let clamped = DVec2::new(
        local.x.clamp(-frame.half_length, frame.half_length),
        local.y.clamp(-frame.half_width, frame.half_width),
    );
    let outside_distance = (local - clamped).length();
    if outside_distance > 0.0 {
        // The centre is outside the box, so the shapes overlap when the radius
        // reaches the nearest surface.
        outside_distance - radius_m
    } else {
        // The centre is inside the box, so they overlap by the radius plus the
        // centre's distance to the nearest face: the minimum translation that
        // carries the circle clear.
        let to_face = (frame.half_length - local.x.abs()).min(frame.half_width - local.y.abs());
        -(radius_m + to_face)
    }
}

/// The interval a box frame spans when projected onto `axis`.
fn box_projection(frame: &BoxFrame, axis: DVec2) -> (f64, f64) {
    let centre = frame.centre.dot(axis);
    let radius = frame.half_length * frame.forward.dot(axis).abs()
        + frame.half_width * frame.left.dot(axis).abs();
    (centre - radius, centre + radius)
}

/// Signed clearance in metres between two boxes, by separating-axis test.
fn box_box_clearance_m(first: &BodyShape, second: &BodyShape) -> f64 {
    let first_frame = box_frame(first);
    let second_frame = box_frame(second);
    let mut least_overlap = f64::INFINITY;
    for axis in [
        first_frame.forward,
        first_frame.left,
        second_frame.forward,
        second_frame.left,
    ] {
        let (first_min, first_max) = box_projection(&first_frame, axis);
        let (second_min, second_max) = box_projection(&second_frame, axis);
        let overlap = first_max.min(second_max) - first_min.max(second_min);
        if overlap < 0.0 {
            // A separating axis exists, so the boxes are disjoint and the exact
            // distance is the closest vertex-to-box distance.
            return box_box_distance_m(&first_frame, &second_frame);
        }
        least_overlap = least_overlap.min(overlap);
    }
    if least_overlap <= CONTACT_EPSILON_M {
        0.0
    } else {
        -least_overlap
    }
}

/// Exact distance in metres between two disjoint boxes, via vertex-to-box
/// clamping. For disjoint convex polygons the closest pair always has a vertex
/// of one polygon as an endpoint, so both directions are checked.
fn box_box_distance_m(first: &BoxFrame, second: &BoxFrame) -> f64 {
    let mut nearest = f64::INFINITY;
    for corner in box_corners(first) {
        nearest = nearest.min(point_to_box_distance(corner, second));
    }
    for corner in box_corners(second) {
        nearest = nearest.min(point_to_box_distance(corner, first));
    }
    nearest
}

#[cfg(test)]
mod tests {
    use super::*;

    fn circle(x: f64, y: f64, radius_m: f64) -> BodyShape {
        BodyShape::Circle {
            centre: DVec2::new(x, y),
            radius_m,
        }
    }

    fn box_body(x: f64, y: f64, heading_rad: f64, length_m: f64, width_m: f64) -> BodyShape {
        BodyShape::Box {
            centre: DVec2::new(x, y),
            heading_rad,
            length_m,
            width_m,
        }
    }

    #[test]
    fn circle_circle_clearance_is_exact_and_signed() {
        // Unit circles three metres apart: one metre of surface clearance.
        assert!(
            (body_clearance_m(&circle(0.0, 0.0, 1.0), &circle(3.0, 0.0, 1.0)) - 1.0).abs() < 1e-12
        );
        // Overlapping by half a metre.
        assert!(
            (body_clearance_m(&circle(0.0, 0.0, 1.0), &circle(1.5, 0.0, 1.0)) + 0.5).abs() < 1e-12
        );
        // Concentric circles penetrate by the sum of the radii.
        assert!(
            (body_clearance_m(&circle(0.0, 0.0, 0.7), &circle(0.0, 0.0, 0.3)) + 1.0).abs() < 1e-12
        );
        // Touching exactly is not an intersection.
        assert!(!bodies_intersect(
            &circle(0.0, 0.0, 1.0),
            &circle(2.0, 0.0, 1.0)
        ));
    }

    #[test]
    fn circle_box_clearance_is_exact_and_signed() {
        // A 5 m by 3 m axis-aligned box: half length 2.5, half width 1.5.
        let body = box_body(0.0, 0.0, 0.0, 5.0, 3.0);
        // Circle to the right, its surface half a metre clear of the box face.
        assert!((body_clearance_m(&circle(3.5, 0.0, 0.5), &body) - 0.5).abs() < 1e-12);
        // Circle centred inside the box: penetration is radius plus the centre's
        // distance to the nearest face (1.5 m on the width axis).
        assert!((body_clearance_m(&circle(0.0, 0.0, 0.5), &body) + 2.0).abs() < 1e-12);
        // Symmetric in the arguments.
        let clear = circle(2.0, 2.0, 0.25);
        assert!((body_clearance_m(&clear, &body) - body_clearance_m(&body, &clear)).abs() < 1e-12);
    }

    #[test]
    fn box_box_clearance_is_exact_and_signed() {
        // Two 2 m squares on the x axis.
        let first = box_body(0.0, 0.0, 0.0, 2.0, 2.0);
        let far = box_body(3.0, 0.0, 0.0, 2.0, 2.0);
        assert!((body_clearance_m(&first, &far) - 1.0).abs() < 1e-12);
        let overlapping = box_body(1.5, 0.0, 0.0, 2.0, 2.0);
        assert!((body_clearance_m(&first, &overlapping) + 0.5).abs() < 1e-12);
        // Exact contact reads as touching, not penetration.
        let touching = box_body(2.0, 0.0, 0.0, 2.0, 2.0);
        assert!(body_clearance_m(&first, &touching).abs() < 1e-12);
        assert!(!bodies_intersect(&first, &touching));
    }

    #[test]
    fn box_box_separating_axis_handles_rotation() {
        // A long thin box crossing a perpendicular one at the origin overlaps,
        // even though neither axis-aligned projection separates them trivially.
        let horizontal = box_body(0.0, 0.0, 0.0, 4.0, 0.5);
        let vertical = box_body(0.0, 0.0, std::f64::consts::FRAC_PI_2, 4.0, 0.5);
        assert!(bodies_intersect(&horizontal, &vertical));
        // Rotating the horizontal body to the left axis clears the origin.
        let offset = box_body(0.0, 3.0, 0.0, 4.0, 0.5);
        assert!(body_clearance_m(&horizontal, &offset) > 0.0);
    }

    #[test]
    fn bounds_enclose_the_body() {
        let circle_body = circle(1.0, -2.0, 0.5);
        let bounds = circle_body.bounds();
        assert_eq!(bounds.min, DVec2::new(0.5, -2.5));
        assert_eq!(bounds.max, DVec2::new(1.5, -1.5));
        assert!((circle_body.circumradius_m() - 0.5).abs() < 1e-12);

        // A 4 by 2 box rotated 30 degrees: the extent uses the rotated corners.
        let rotated = box_body(0.0, 0.0, 0.3, 4.0, 2.0);
        let (sin, cos) = 0.3_f64.sin_cos();
        let expected_x = (cos.abs() * 4.0 + sin.abs() * 2.0) * 0.5;
        assert!((rotated.bounds().max.x - expected_x).abs() < 1e-12);
        assert!(
            (rotated.circumradius_m() - (4.0_f64 * 4.0 + 2.0 * 2.0).sqrt() * 0.5).abs() < 1e-12
        );
    }
}
