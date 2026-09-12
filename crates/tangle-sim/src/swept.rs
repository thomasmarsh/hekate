//! Swept body bounds and time-of-impact casts over one tick of linear motion.
//!
//! # Model card
//!
//! `PHASE_1_PLAN.md` Increment 4 ("Swept candidate bounds and time-of-impact
//! shape casts for tunneling protection") needs a query that notices two bodies
//! crossing between two ticks. The static queries in [`crate::query`] cannot: a
//! fast body leaves one side of a thin body and appears on the other with no
//! overlap at either tick endpoint, so a test that samples only the endpoints
//! misses the crossing. This module owns that query. The per-agent sweep bound
//! the broad phase indexes lives on [`SweptBody`], and the swept candidate query
//! over it is [`SweptBroadPhase`](crate::index::SweptBroadPhase).
//!
//! ## State
//!
//! A [`SweptBody`] is a body shape at the tick start plus a linear displacement
//! in metres across the tick. The kernel integrates position only, so a body
//! keeps its heading and extents for the tick: a cast solves translation only,
//! and rotation within a tick is out of scope. Both bodies of a cast move over
//! the same tick, and only their relative displacement enters the query.
//!
//! ## Contact semantics
//!
//! [`time_of_impact`] reports the first tick fraction at which the two bodies
//! enter the contact band, that is the first fraction at which the signed
//! clearance [`body_clearance_m`] is at or below [`CONTACT_EPSILON_M`]. Reading
//! the outcomes:
//!
//! - a first touch reports the time, a `clearance_m` at or just above zero, and
//!   the unit normal of the touching surfaces;
//! - start overlap, where the bodies already overlap at the tick start, reports
//!   `time = 0.0` with a negative `clearance_m` and the minimum-translation
//!   normal: the bodies are already in contact at the only time the cast can
//!   report;
//! - a graze whose closest approach stays above the band reports no hit, so a
//!   near miss of more than a nanometre is not a contact;
//! - parallel or coincident motion never closes a gap, so bodies that keep a
//!   constant clearance report no hit however long the tick;
//! - a crossing pair whose segments intersect between the endpoints reports the
//!   first entry into the band, which is the whole point of the swept query.
//!
//! The normal points from the first body toward the second and is a unit
//! vector. A cast is symmetric in its arguments: swapping them keeps the time
//! and the clearance and mirrors the normal.
//!
//! ## Algorithm
//!
//! The signed clearance between two convex bodies translating linearly is a
//! convex function of the tick fraction. Each body's shape is a convex set and
//! their separation at time `t` is the distance from `t` times the relative
//! displacement to the Minkowski difference of the two start shapes, a convex
//! set; the distance from a point to a convex set is convex, and composing it
//! with a linear map keeps it convex. Two consequences make the cast exact and
//! tunneling-free:
//!
//! - the clearance rate is non-decreasing over the tick, so "is the clearance
//!   still closing?" is a monotone predicate and one bisection locates the tick
//!   fraction where the bodies stop closing, which is the first contact when a
//!   contact exists;
//! - the fractions at which the clearance is inside the band form a single
//!   interval, so "has the clearance entered the band?" is monotone too and a
//!   second bisection locates the first entry.
//!
//! A body can therefore not tunnel through another between two ticks: the cast
//! reads the whole interval, not a sample of it, so an arbitrarily narrow
//! crossing window between the endpoints is still found. The clearance rate used
//! is the exact derivative of the convex clearance, the relative displacement
//! projected onto [`body_contact_normal`], which is zero where the bodies
//! already overlap.
//!
//! ## Tolerances and tie-breaks
//!
//! [`TOI_TIME_TOLERANCE`] is the declared resolution of a reported time: each
//! bisection stops when its bracket is no wider, so a reported time is within
//! one part in `1e12` of a tick of the first band entry. A hit is reported only
//! when the clearance at the located time is inside the contact band, and no hit
//! is reported when the closest approach the cast locates exceeds the band by
//! more than the relative displacement times that tolerance, which is `1e-9 m`
//! for any relative displacement below `1e3 m` per tick.
//!
//! Every tie-break belongs to [`body_contact_normal`] and [`body_clearance_m`],
//! which the cast reuses rather than restates, so a swept query and a static
//! query agree exactly on what touching means and which direction separates.
//!
//! ## Determinism
//!
//! A cast is a pure function of its two bodies: a fixed sequence of exact
//! clearance and normal evaluations with no random draws, no iteration over a
//! hash map, and no global state. The same pair of swept bodies always reports
//! the same time, clearance, and normal.

use glam::DVec2;

use crate::query::{Aabb, BodyShape, CONTACT_EPSILON_M, body_clearance_m, body_contact_normal};

/// Resolution of a reported time of impact, as a fraction of one tick.
///
/// A cast bisects the tick until its bracket is no wider than this, so a
/// reported time is within one part in `1e12` of a tick of the first tick
/// fraction at which the bodies enter the contact band. At the kernel's step
/// that is a fraction of a nanosecond of simulated time.
pub const TOI_TIME_TOLERANCE: f64 = 1e-12;

/// One body over one tick: its shape at the tick start and its linear
/// displacement across the tick.
///
/// The shape keeps its heading and extents for the whole tick; see the module
/// card. [`SweptBody::swept_bounds`] is the per-agent sweep bound the swept
/// broad phase indexes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SweptBody {
    /// The body shape at the tick start.
    pub shape: BodyShape,
    /// The body's displacement across the tick, in metres.
    pub displacement_m: DVec2,
}

impl SweptBody {
    /// The shape at `fraction` of the tick: the start shape translated by that
    /// fraction of the displacement.
    pub fn shape_at(&self, fraction: f64) -> BodyShape {
        self.shape.translated(self.displacement_m * fraction)
    }

    /// The shape at the tick end, exactly `shape_at(1.0)`.
    pub fn end_shape(&self) -> BodyShape {
        self.shape_at(1.0)
    }

    /// The tight axis-aligned box around everything the body covers this tick,
    /// in world metres.
    ///
    /// The body keeps its orientation and translates linearly, so each axis of
    /// its enclosing box varies linearly over the tick: the extremes are the
    /// tick endpoints, and the box of the two endpoint boxes is the tight bound
    /// of the whole swept volume. It therefore contains the boxes at the tick
    /// start and at the tick end exactly.
    pub fn swept_bounds(&self) -> Aabb {
        let start = self.shape.bounds();
        let end = self.end_shape().bounds();
        Aabb {
            min: start.min.min(end.min),
            max: start.max.max(end.max),
        }
    }

    /// The distance in metres from the start centre to the farthest point the
    /// body can reach this tick: its circumradius plus the displacement
    /// magnitude.
    ///
    /// The swept broad phase widens a query by the largest reach among the
    /// bodies it indexes, because a body's start centre may sit up to this far
    /// outside a region its swept volume still reaches into.
    pub fn swept_reach_m(&self) -> f64 {
        self.shape.circumradius_m() + self.displacement_m.length()
    }
}

/// The first contact of two bodies over one tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeOfImpact {
    /// Tick fraction at first contact, in `[0, 1]`.
    pub time: f64,
    /// Unit contact normal at `time`, pointing from the first body toward the
    /// second.
    pub normal: DVec2,
    /// Signed surface clearance in metres at `time`: zero or slightly positive
    /// at a first touch, negative when the bodies already overlap at the tick
    /// start.
    pub clearance_m: f64,
}

/// The first time within one tick at which two moving bodies enter the contact
/// band, or `None` when they stay clear of it for the whole tick.
/// The first time within one tick at which two moving bodies enter the contact
/// band, or `None` when they stay clear of it for the whole tick.
///
/// This is [`band_entry`] with the contact band
/// [`CONTACT_EPSILON_M`](crate::CONTACT_EPSILON_M), so a swept and a static
/// query agree exactly on what touching means.
///
/// See the module card for the contact semantics, the algorithm, and the
/// declared tolerances.
pub fn time_of_impact(first: &SweptBody, second: &SweptBody) -> Option<TimeOfImpact> {
    band_entry(first, second, CONTACT_EPSILON_M)
}

/// The first time within one tick at which two moving bodies enter the band of
/// signed clearances `<= band_m`, or `None` when they stay clear of it.
///
/// The signed clearance between two convex bodies translating linearly is
/// convex in the tick fraction, so the set of fractions inside a band is a
/// single interval and the same bisection that finds the contact of
/// [`time_of_impact`] finds a wider band. `band_m` must be non-negative; the
/// contact band [`CONTACT_EPSILON_M`](crate::CONTACT_EPSILON_M) is the
/// narrowest useful value, and a wider band is how a caller asks whether a pair
/// came within a threshold, such as the near-miss band of [`crate::safety`].
///
/// A start clearance already inside the band reports `time = 0.0`.
pub fn band_entry(first: &SweptBody, second: &SweptBody, band_m: f64) -> Option<TimeOfImpact> {
    let band_m = band_m.max(0.0);
    let start_clearance = body_clearance_m(&first.shape, &second.shape);
    if start_clearance <= band_m {
        return Some(contact(first, second, 0.0, start_clearance));
    }
    if clearance_at(first, second, 1.0) <= band_m {
        // The tick ends inside the band, so the band interval ends at or before
        // the tick end and the band predicate is false-then-true on the tick.
        let time = first_fraction(0.0, 1.0, |fraction| {
            clearance_at(first, second, fraction) <= band_m
        });
        return Some(contact(
            first,
            second,
            time,
            clearance_at(first, second, time),
        ));
    }
    // Clear of the band at both tick endpoints, so an entry needs the clearance
    // rate to turn non-negative inside the tick. The rate is non-decreasing, so
    // its sign is the monotone predicate to bisect: it is negative exactly while
    // the bodies are still closing.
    let relative_m = second.displacement_m - first.displacement_m;
    if clearance_rate(first, second, relative_m, 0.0) >= 0.0
        || clearance_rate(first, second, relative_m, 1.0) < 0.0
    {
        // Either the bodies already stop closing at the tick start, so the
        // clearance can never fall, or they are still closing at the tick end,
        // so the tick's lowest clearance is the one it ends with. Both are
        // above the band, so there is no entry this tick.
        return None;
    }
    let turning = first_fraction(0.0, 1.0, |fraction| {
        clearance_rate(first, second, relative_m, fraction) >= 0.0
    });
    let closest = clearance_at(first, second, turning);
    if closest > band_m {
        return None;
    }
    let time = first_fraction(0.0, turning, |fraction| {
        clearance_at(first, second, fraction) <= band_m
    });
    Some(contact(
        first,
        second,
        time,
        clearance_at(first, second, time),
    ))
}

/// The contact record at `time`, with the normal of the touching shapes.
fn contact(first: &SweptBody, second: &SweptBody, time: f64, clearance_m: f64) -> TimeOfImpact {
    let first_shape = first.shape_at(time);
    let second_shape = second.shape_at(time);
    TimeOfImpact {
        time,
        normal: body_contact_normal(&first_shape, &second_shape),
        clearance_m,
    }
}

/// The signed clearance in metres between two swept bodies at `fraction` of the
/// tick.
fn clearance_at(first: &SweptBody, second: &SweptBody, fraction: f64) -> f64 {
    body_clearance_m(&first.shape_at(fraction), &second.shape_at(fraction))
}

/// The rate in metres per tick at which the clearance between two swept bodies
/// changes at `fraction`.
///
/// The signed clearance is a convex function of the tick fraction, and this is
/// its derivative: the relative displacement of the second body projected onto
/// the contact normal, whose sign says whether the bodies are still closing.
/// Where the bodies already overlap or touch that convex function is flat at
/// zero, so the reported rate is zero as well. Either way the value is a
/// subgradient of the convex clearance, so the sequence over the tick is
/// non-decreasing and its sign is a monotone predicate.
fn clearance_rate(first: &SweptBody, second: &SweptBody, relative_m: DVec2, fraction: f64) -> f64 {
    let first_shape = first.shape_at(fraction);
    let second_shape = second.shape_at(fraction);
    if body_clearance_m(&first_shape, &second_shape) <= 0.0 {
        0.0
    } else {
        relative_m.dot(body_contact_normal(&first_shape, &second_shape))
    }
}

/// The first fraction in `[low, high]` at which a monotone predicate holds.
///
/// The predicate must be false at `low`, true at `high`, and false-then-true
/// across the interval. Both callers bisect a convex predicate whose true set is
/// a single interval, so that holds. The search stops when the bracket is no
/// wider than [`TOI_TIME_TOLERANCE`] or when floating point can no longer split
/// it, and returns the fraction that satisfies the predicate.
fn first_fraction(mut low: f64, mut high: f64, predicate: impl Fn(f64) -> bool) -> f64 {
    while high - low > TOI_TIME_TOLERANCE {
        let midpoint = 0.5 * (low + high);
        if midpoint <= low || midpoint >= high {
            break;
        }
        if predicate(midpoint) {
            high = midpoint;
        } else {
            low = midpoint;
        }
    }
    high
}

#[cfg(test)]
mod tests {
    use super::*;

    fn circle(centre: DVec2, radius_m: f64) -> BodyShape {
        BodyShape::Circle { centre, radius_m }
    }

    fn moving(shape: BodyShape, displacement_m: DVec2) -> SweptBody {
        SweptBody {
            shape,
            displacement_m,
        }
    }

    fn still(shape: BodyShape) -> SweptBody {
        moving(shape, DVec2::ZERO)
    }

    #[test]
    fn the_swept_bound_covers_both_endpoints_and_is_tight() {
        let body = moving(circle(DVec2::ZERO, 0.5), DVec2::new(3.0, -2.0));
        let bounds = body.swept_bounds();
        assert_eq!(bounds.min, DVec2::new(-0.5, -2.5));
        assert_eq!(bounds.max, DVec2::new(3.5, 0.5));
        // It contains the start and end boxes, and the second endpoint is
        // exactly `shape_at(1.0)`.
        assert_eq!(body.end_shape(), circle(DVec2::new(3.0, -2.0), 0.5));
        for fraction in [0.0, 0.25, 0.5, 1.0] {
            let endpoint = body.shape_at(fraction).bounds();
            assert!(bounds.min.x <= endpoint.min.x && bounds.min.y <= endpoint.min.y);
            assert!(bounds.max.x >= endpoint.max.x && bounds.max.y >= endpoint.max.y);
        }
        assert!((body.swept_reach_m() - (0.5 + 13.0_f64.sqrt())).abs() < 1e-12);
        // A body that does not move has the bound of its own shape.
        assert_eq!(
            still(circle(DVec2::new(2.0, 1.0), 0.25)).swept_bounds(),
            circle(DVec2::new(2.0, 1.0), 0.25).bounds()
        );
    }

    #[test]
    fn start_overlap_reports_time_zero_with_the_separation_normal() {
        let first = still(circle(DVec2::ZERO, 1.0));
        let second = moving(circle(DVec2::new(1.5, 0.0), 1.0), DVec2::new(1.0, 0.0));
        let hit = time_of_impact(&first, &second).expect("start overlap is a contact");
        assert_eq!(hit.time, 0.0);
        assert!((hit.clearance_m + 0.5).abs() < 1e-12);
        assert_eq!(hit.normal, DVec2::X);
    }

    #[test]
    fn touching_at_the_tick_start_is_a_contact_at_zero() {
        let first = still(circle(DVec2::ZERO, 1.0));
        let second = moving(circle(DVec2::new(2.0, 0.0), 1.0), DVec2::new(5.0, 0.0));
        let hit = time_of_impact(&first, &second).expect("exact touching is a contact");
        assert_eq!(hit.time, 0.0);
        assert!(hit.clearance_m.abs() < 1e-12);
        assert_eq!(hit.normal, DVec2::X);
    }

    #[test]
    fn parallel_and_coincident_motion_never_closes_a_gap() {
        // Equal displacement: the clearance never changes.
        let lead = moving(circle(DVec2::ZERO, 1.0), DVec2::new(4.0, 0.0));
        let follower = moving(circle(DVec2::new(0.0, 3.0), 1.0), DVec2::new(4.0, 0.0));
        assert_eq!(time_of_impact(&lead, &follower), None);
        // Motion perpendicular to the line of centres keeps the gap.
        let passing = moving(circle(DVec2::new(-4.0, 3.0), 1.0), DVec2::new(8.0, 0.0));
        assert_eq!(time_of_impact(&lead, &passing), None);
        // One body at rest with a clear neighbour.
        assert_eq!(
            time_of_impact(
                &still(circle(DVec2::ZERO, 1.0)),
                &still(circle(DVec2::new(5.0, 0.0), 1.0))
            ),
            None
        );
    }

    #[test]
    fn a_graze_inside_the_band_is_a_contact_and_above_it_is_not() {
        let first = still(circle(DVec2::ZERO, 1.0));
        let inside = moving(
            circle(DVec2::new(-5.0, 2.0 + 0.5 * CONTACT_EPSILON_M), 1.0),
            DVec2::new(10.0, 0.0),
        );
        let hit = time_of_impact(&first, &inside).expect("a graze inside the band is a contact");
        assert!(hit.clearance_m <= CONTACT_EPSILON_M);
        assert!((hit.time - 0.5).abs() < 1e-4, "graze time {}", hit.time);
        // One micrometre further out, the same crossing is a near miss.
        let outside = moving(
            circle(DVec2::new(-5.0, 2.0 + 1e-6), 1.0),
            DVec2::new(10.0, 0.0),
        );
        assert_eq!(time_of_impact(&first, &outside), None);
    }

    #[test]
    fn a_cast_is_symmetric_in_its_arguments() {
        let first = moving(circle(DVec2::new(-6.0, 0.0), 1.0), DVec2::new(12.0, 0.0));
        let second = still(circle(DVec2::ZERO, 1.0));
        let forward = time_of_impact(&first, &second).expect("crossing circles touch");
        let backward = time_of_impact(&second, &first).expect("crossing circles touch");
        assert!((forward.time - backward.time).abs() < 1e-12);
        assert!((forward.clearance_m - backward.clearance_m).abs() < 1e-12);
        assert!((forward.normal + backward.normal).length() < 1e-12);
    }
}
