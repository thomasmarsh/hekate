//! Deterministic hitch and trailer-pose integration for an articulated chain.
//!
//! An `AgentFamily::ArticulatedWheeled` agent's lead (tractor) segment is a
//! plain [`crate::agent::AgentStore`] vehicle: its `position`/`heading_rad`
//! integrate through the exact same [`crate::sim::Simulation`] path a
//! `WheeledBox` uses, so nothing here ever writes those columns. Every
//! trailing segment's pose is additional per-agent state, owned by one
//! [`ArticulatedState`] and driven once per tick, after the lead segment has
//! already advanced, by [`ArticulatedState::advance`].
//!
//! # Kinematic model
//!
//! Each segment `i` in chain order exposes two fixed body-frame points, both
//! derived from its own sampled length and (for a trailing segment) its own
//! sampled hitch/kingpin setback:
//!
//! - its **rear reference point**, half its length behind its centre along
//!   its own heading — the point the *next* segment down the chain hitches
//!   to. For the lead segment this is where its (unauthored) fifth wheel is
//!   assumed to sit: the physical rear of the tractor body.
//! - for a trailing segment only, its own **kingpin**, `hitch_offset_m`
//!   behind its own front edge — the point that must coincide, at every
//!   instant, with the rear reference point of the segment ahead of it (a
//!   physical pin joint, so the two points are never merely close).
//!
//! This is exactly the classical single-axle "kingpin trailer" kinematic
//! model (e.g. the on-axle hitching case of Altafini's N-trailer systems):
//! segment `i`'s rear reference point is treated as a non-holonomic
//! "axle" that moves with no lateral slip — its velocity is tangent to the
//! segment's own heading — while its kingpin, a fixed `L_i = length_i -
//! hitch_offset_i` ahead of that axle along the same heading, is rigidly
//! towed by the segment ahead. Writing `v_h` for the instantaneous velocity
//! of the towing rear-reference point (computed by finite difference from its
//! position one tick ago and its position now, both exact functions of the
//! stored/updated poses) and `θ` for the trailing segment's heading, the
//! no-slip constraint gives the standard trailer heading-rate equation
//!
//! ```text
//! θ' = (v_h · perp(θ)) / L_i
//! ```
//!
//! where `perp(θ) = (-sin θ, cos θ)`. Each tick integrates this once with
//! explicit Euler (`θ_new = θ_old + θ' * dt`) and then **exactly** repins the
//! segment's centre from its new heading and the segment ahead's exact new
//! rear-reference point, so the two segments' joint never drifts apart —
//! only the heading carries first-order integration error, and that error is
//! the same order as every other fixed-step kinematic integration this crate
//! already performs. See `crates/hekate-sim/tests/inc3_articulated.rs` for the
//! straight-path exact check and the constant-radius steady-state off-tracking
//! check this model predicts.
//!
//! Every draw this module reads was already sampled once at admission (see
//! [`crate::profile::sample_articulated_chain_profile`]); nothing here reads
//! an RNG, so two runs from the same seed produce byte-identical segment
//! poses.

use glam::DVec2;

/// Minimum hitch arm length used in the heading-rate denominator.
///
/// `L_i = length_i - hitch_offset_i` is expected to be a genuine positive
/// drawbar length for any realistic authored chain; this only guards against a
/// degenerate authored geometry (`hitch_offset_m` at or beyond the segment's
/// own length) turning a division into `NaN`/`inf`. It never fires for a
/// checked-in fixture.
const MIN_HITCH_ARM_M: f64 = 1e-3;

/// One segment's sampled, immutable-for-the-run geometry.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ArticulatedSegmentGeometry {
    /// Sampled segment length in metres.
    pub(crate) length_m: f64,
    /// Sampled segment width in metres.
    pub(crate) width_m: f64,
    /// Sampled hitch/kingpin setback from this segment's own front edge in
    /// metres: `None` for the lead segment, `Some` for every trailing one.
    pub(crate) hitch_offset_m: Option<f64>,
}

/// One segment's world pose: its centre and heading, exactly as
/// [`crate::snapshot::BodySegmentSample`] reports it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SegmentPose {
    pub(crate) position: DVec2,
    pub(crate) heading_rad: f64,
}

/// One hitch's exceeded-limit state edge, returned by [`ArticulatedState::advance`].
///
/// `hitch_index` is the trailing segment's own index in the chain (`1` for the
/// hitch between the lead and the first trailer, and so on), matching
/// [`ArticulatedState::trailers`]'s zero-based indexing plus one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HitchLimitTransition {
    pub(crate) hitch_index: u32,
    pub(crate) angle_rad: f64,
    pub(crate) limit_rad: f64,
    /// `true` when the hitch angle just exceeded the limit, `false` when it
    /// just returned inside it.
    pub(crate) exceeding: bool,
}

/// The rear reference point of a segment: half its length behind its centre
/// along its own heading. See the module docs for what this point means for
/// the lead segment versus a trailing one.
fn rear_reference(position: DVec2, heading_rad: f64, length_m: f64) -> DVec2 {
    position - DVec2::from_angle(heading_rad) * (length_m * 0.5)
}

/// A trailing segment's own kingpin: `hitch_offset_m` behind its front edge.
fn kingpin(position: DVec2, heading_rad: f64, length_m: f64, hitch_offset_m: f64) -> DVec2 {
    position + DVec2::from_angle(heading_rad) * (length_m * 0.5 - hitch_offset_m)
}

/// Recover a segment's centre from its kingpin position and its own heading:
/// the exact inverse of [`kingpin`], used to repin a trailing segment to the
/// segment ahead's exact new rear-reference point every tick.
fn centre_from_kingpin(
    kingpin_position: DVec2,
    heading_rad: f64,
    length_m: f64,
    hitch_offset_m: f64,
) -> DVec2 {
    kingpin_position - DVec2::from_angle(heading_rad) * (length_m * 0.5 - hitch_offset_m)
}

/// Wrap an angle in radians to `(-pi, pi]`.
///
/// Mirrors the private helper of the same name in `crate::steering`/`crate::sim`:
/// each module keeps its own copy rather than sharing a `pub(crate)` export,
/// consistent with the existing duplication between those two.
fn wrap_pi(angle_rad: f64) -> f64 {
    let mut wrapped = (angle_rad + std::f64::consts::PI) % std::f64::consts::TAU;
    if wrapped <= 0.0 {
        wrapped += std::f64::consts::TAU;
    }
    wrapped - std::f64::consts::PI
}

/// Per-agent runtime state for one admitted `ArticulatedWheeled` agent.
///
/// `None` in `AgentStore::articulated` for every other agent. The lead
/// segment's own pose is never duplicated here at rest — [`Self::advance`]
/// takes it as an argument each tick, reading it fresh from
/// `AgentStore::position`/`heading_rad` — except for `previous_lead`, the one
/// snapshot this state must keep so the next tick can finite-difference the
/// lead's own rear-reference velocity.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ArticulatedState {
    /// Every segment's sampled geometry, in chain order, lead segment first.
    segments: Vec<ArticulatedSegmentGeometry>,
    /// Sampled maximum articulation angle in radians, shared by every hitch
    /// (the compiled body carries one range for the whole chain).
    articulation_limit_rad: f64,
    /// The lead segment's pose as of the end of the previous tick (or at
    /// admission, before the first tick).
    previous_lead: SegmentPose,
    /// Every trailing segment's current pose, in chain order: `trailers[0]`
    /// is `segments[1]`, and so on.
    trailers: Vec<SegmentPose>,
    /// Whether each hitch (`trailers[i]`'s hitch to the segment ahead of it)
    /// was exceeding `articulation_limit_rad` as of the last check, so
    /// [`Self::advance`] can report only the edges.
    exceeding: Vec<bool>,
}

impl ArticulatedState {
    /// Lay out a newly admitted chain: every trailing segment aligned with the
    /// lead segment's own entry heading, nose to tail, each segment's kingpin
    /// pinned exactly to the rear reference point of the segment ahead of it.
    ///
    /// This is the only place a trailing segment's heading is set without
    /// going through [`Self::advance`]'s heading-rate integration: the chain
    /// enters the world already formed, not folding into shape over time, so
    /// its initial condition is the aligned configuration every fixture and
    /// analytic reference in this slice assumes.
    pub(crate) fn spawn(
        segments: Vec<ArticulatedSegmentGeometry>,
        articulation_limit_rad: f64,
        lead_position: DVec2,
        lead_heading_rad: f64,
    ) -> Self {
        let lead_length_m = segments.first().map_or(0.0, |segment| segment.length_m);
        let mut driver_rear = rear_reference(lead_position, lead_heading_rad, lead_length_m);
        let mut trailers = Vec::with_capacity(segments.len().saturating_sub(1));
        for segment in segments.iter().skip(1) {
            let hitch_offset_m = segment.hitch_offset_m.unwrap_or(0.0);
            let position = centre_from_kingpin(
                driver_rear,
                lead_heading_rad,
                segment.length_m,
                hitch_offset_m,
            );
            trailers.push(SegmentPose {
                position,
                heading_rad: lead_heading_rad,
            });
            driver_rear = rear_reference(position, lead_heading_rad, segment.length_m);
        }
        let exceeding = vec![false; trailers.len()];
        Self {
            segments,
            articulation_limit_rad,
            previous_lead: SegmentPose {
                position: lead_position,
                heading_rad: lead_heading_rad,
            },
            trailers,
            exceeding,
        }
    }

    /// Every trailing segment's current pose, in chain order.
    pub(crate) fn trailers(&self) -> &[SegmentPose] {
        &self.trailers
    }

    /// Every segment's sampled geometry, in chain order, lead segment first.
    pub(crate) fn segments(&self) -> &[ArticulatedSegmentGeometry] {
        &self.segments
    }

    /// Drive every trailing segment's pose from the lead segment's just-
    /// integrated pose, and report each hitch's exceeded-limit edge.
    ///
    /// `lead_position`/`lead_heading_rad` are the lead segment's pose *after*
    /// this tick's ordinary `advance_physics` call, read fresh from
    /// `AgentStore`; this never reads or writes that store itself, so the
    /// tractor's own integration stays exactly the `WheeledBox` path.
    pub(crate) fn advance(
        &mut self,
        lead_position: DVec2,
        lead_heading_rad: f64,
        dt: f64,
    ) -> Vec<HitchLimitTransition> {
        let mut transitions = Vec::new();
        if self.segments.is_empty() {
            return transitions;
        }
        let lead_length_m = self.segments[0].length_m;
        // The point that drives the next segment down the chain, before and
        // after this tick, starting with the lead segment itself, and the
        // heading of the segment ahead the next hitch angle is measured
        // against.
        let mut driver_prev = rear_reference(
            self.previous_lead.position,
            self.previous_lead.heading_rad,
            lead_length_m,
        );
        let mut driver_new = rear_reference(lead_position, lead_heading_rad, lead_length_m);
        let mut ahead_heading_rad = lead_heading_rad;

        for (offset, segment) in self.segments.iter().enumerate().skip(1) {
            let trailer_index = offset - 1;
            let hitch_offset_m = segment.hitch_offset_m.unwrap_or(0.0);
            let arm_m = (segment.length_m - hitch_offset_m).max(MIN_HITCH_ARM_M);

            let previous_pose = self.trailers[trailer_index];
            let v_h = (driver_new - driver_prev) / dt;
            let perp = DVec2::new(
                -previous_pose.heading_rad.sin(),
                previous_pose.heading_rad.cos(),
            );
            let heading_rate = v_h.dot(perp) / arm_m;
            let heading_rad = previous_pose.heading_rad + heading_rate * dt;
            // Repin exactly to the driver's new rear-reference point: the
            // physical joint never drifts, only the heading carries the
            // explicit-Euler integration error.
            let position =
                centre_from_kingpin(driver_new, heading_rad, segment.length_m, hitch_offset_m);
            debug_assert!(
                kingpin(position, heading_rad, segment.length_m, hitch_offset_m)
                    .distance(driver_new)
                    < 1e-6,
                "the repinned kingpin must coincide exactly with the driver's rear reference"
            );
            self.trailers[trailer_index] = SegmentPose {
                position,
                heading_rad,
            };

            let relative_angle_rad = wrap_pi(ahead_heading_rad - heading_rad).abs();
            let was_exceeding = self.exceeding[trailer_index];
            let is_exceeding = relative_angle_rad > self.articulation_limit_rad;
            if is_exceeding != was_exceeding {
                transitions.push(HitchLimitTransition {
                    hitch_index: offset as u32,
                    angle_rad: relative_angle_rad,
                    limit_rad: self.articulation_limit_rad,
                    exceeding: is_exceeding,
                });
            }
            self.exceeding[trailer_index] = is_exceeding;

            // This segment becomes the driver for the next one in the chain.
            driver_prev = rear_reference(
                previous_pose.position,
                previous_pose.heading_rad,
                segment.length_m,
            );
            driver_new = rear_reference(position, heading_rad, segment.length_m);
            ahead_heading_rad = heading_rad;
        }

        self.previous_lead = SegmentPose {
            position: lead_position,
            heading_rad: lead_heading_rad,
        };
        transitions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry(
        length_m: f64,
        width_m: f64,
        hitch_offset_m: Option<f64>,
    ) -> ArticulatedSegmentGeometry {
        ArticulatedSegmentGeometry {
            length_m,
            width_m,
            hitch_offset_m,
        }
    }

    fn tractor_semitrailer() -> Vec<ArticulatedSegmentGeometry> {
        vec![geometry(6.0, 2.5, None), geometry(13.6, 2.55, Some(1.2))]
    }

    /// At spawn, every trailing segment is aligned with the lead heading and
    /// its kingpin sits exactly on the segment ahead's rear reference point.
    #[test]
    fn spawn_lays_out_an_aligned_chain_with_exact_joints() {
        let state = ArticulatedState::spawn(tractor_semitrailer(), 0.9, DVec2::new(10.0, 0.0), 0.0);
        assert_eq!(state.trailers().len(), 1);
        let trailer = state.trailers()[0];
        assert!((trailer.heading_rad - 0.0).abs() < 1e-12);
        // Lead rear reference: (10 - 3, 0) = (7, 0). Trailer kingpin must sit
        // there: centre = kingpin - (len/2 - offset) along heading (0) =
        // (7 - (6.8 - 1.2), 0) = (1.4, 0).
        assert!((trailer.position.x - 1.4).abs() < 1e-9);
        assert!((trailer.position.y - 0.0).abs() < 1e-9);
    }

    /// Driving the lead straight ahead at a constant heading never rotates the
    /// trailer and keeps its offset behind the lead exactly constant: the
    /// exact, closed-form straight-path reference this slice's fixture test
    /// checks against the full simulation.
    #[test]
    fn a_straight_lead_keeps_the_trailer_aligned_and_at_a_constant_offset() {
        let mut state = ArticulatedState::spawn(tractor_semitrailer(), 0.9, DVec2::ZERO, 0.0);
        let dt = 0.05;
        let speed_mps = 8.0;
        let mut lead_position = DVec2::ZERO;
        for _ in 0..200 {
            lead_position += DVec2::new(speed_mps * dt, 0.0);
            let transitions = state.advance(lead_position, 0.0, dt);
            assert!(transitions.is_empty());
        }
        let trailer = state.trailers()[0];
        assert!(
            (trailer.heading_rad - 0.0).abs() < 1e-9,
            "heading drifted: {}",
            trailer.heading_rad
        );
        // The offset behind the lead must stay exactly the spawn offset
        // (10 - 1.4 = 8.6 at spawn with lead at (10,0); here lead starts at 0
        // so the trailer starts at -8.6 and must track 8.6 m behind forever).
        let expected_x = lead_position.x - 8.6;
        assert!(
            (trailer.position.x - expected_x).abs() < 1e-9,
            "trailer.x={} expected={}",
            trailer.position.x,
            expected_x
        );
        assert!(trailer.position.y.abs() < 1e-9);
    }

    /// A hitch angle beyond the compiled limit reports exactly one rising edge
    /// and, once it recovers, exactly one falling edge.
    #[test]
    fn a_hitch_beyond_the_limit_reports_one_rising_and_one_falling_edge() {
        // A tiny articulation limit so an ordinary turn trips it immediately.
        let mut state = ArticulatedState::spawn(tractor_semitrailer(), 0.01, DVec2::ZERO, 0.0);
        let dt = 0.05;
        // Snap the lead through a hard turn in one tick, which the trailer's
        // heading cannot instantly follow, opening a large hitch angle.
        let mut lead_position = DVec2::new(0.4, 0.0);
        let lead_heading_rad = 1.2;
        let transitions = state.advance(lead_position, lead_heading_rad, dt);
        assert!(
            transitions.iter().any(|t| t.exceeding),
            "a hard turn must trip the limit: {transitions:?}"
        );
        // Continue straight ahead on the new heading long enough for the
        // trailer to rotate into alignment and the angle to fall back inside
        // the limit.
        let mut saw_falling = false;
        for _ in 0..2000 {
            lead_position += DVec2::from_angle(lead_heading_rad) * (8.0 * dt);
            let transitions = state.advance(lead_position, lead_heading_rad, dt);
            if transitions.iter().any(|t| !t.exceeding) {
                saw_falling = true;
                break;
            }
        }
        assert!(
            saw_falling,
            "the hitch angle never recovered inside the limit"
        );
    }
}
