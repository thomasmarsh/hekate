//! Phase 1 Increment 4 slice B: swept candidate bounds and time-of-impact
//! casts.
//!
//! `PHASE_1_PLAN.md` Increment 4 gates this slice on "swept fixtures detect
//! crossing bodies that do not overlap at either tick endpoint". The fixtures
//! below are closed-form crossings computed by hand, and the property tests
//! compare the swept queries against a uniform-sampling reference written from
//! the clearance query, never a call back into the cast under test.
//!
//! The swept query is [`time_of_impact`] over [`SweptBody`] pairs; the swept
//! broad phase is [`SweptBroadPhase`], whose per-agent bound is
//! [`SweptBody::swept_bounds`].
//!
//! # Declared tolerances and tie-breaks
//!
//! - [`TOI_TOLERANCE`] is the absolute tick-fraction tolerance for comparing a
//!   cast's reported time with an analytic or reference time. A tick is a small
//!   slice of simulated time and the cast's own declared resolution is far
//!   finer, so `1e-9` of a tick is far above the arithmetic noise of either
//!   side and far below any physical scale.
//! - [`NORMAL_TOLERANCE`] is the component tolerance for comparing a reported
//!   contact normal with an expected unit direction. Normals are read off the
//!   separating geometry, so they carry the floating-point cancellation of a
//!   near-touching feature pair; `1e-6` covers that without accepting a
//!   different direction.
//! - [`CONTACT_BAND_M`] is the contact band: a hit is a hit exactly when the
//!   clearance at the reported time is at or below it, and a pair whose closest
//!   approach stays above it is a near miss.
//! - [`SAMPLES`] is the uniform sample grid of the reference. A reference
//!   absence proof is only used when the sampled minimum clearance, lowered by
//!   the relative displacement over half a sample, is still above the band.
//! - Ordering is explicit throughout: swept candidates are ascending and unique
//!   by [`AgentId`], swept candidate pairs are ascending `(first, second)` with
//!   `first < second`, and the contact-normal tie-breaks are the ones
//!   `body_contact_normal` documents. The randomized assertions below check
//!   that order and uniqueness.

use std::collections::BTreeSet;

use glam::DVec2;
use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::{Rng, SeedableRng};

use hekate_sim::{
    AgentId, BodyShape, BroadPhase, CONTACT_EPSILON_M, SweptBody, SweptBroadPhase,
    bodies_intersect, body_clearance_m, time_of_impact,
};

/// Absolute tick-fraction tolerance for a cast-versus-analytic time comparison.
const TOI_TOLERANCE: f64 = 1e-9;

/// Component tolerance for a contact-normal comparison.
const NORMAL_TOLERANCE: f64 = 1e-6;

/// The contact band, in metres: the clearance at or below which a cast reports
/// a hit.
const CONTACT_BAND_M: f64 = CONTACT_EPSILON_M;

/// Uniform samples the reference grid takes over one tick.
const SAMPLES: usize = 4096;

/// Fixed seeds for the randomized worlds; the sweep is reproducible.
const SEEDS: [u64; 8] = [1, 2, 3, 5, 8, 13, 21, 34];

/// Bodies per randomized world.
const WORLD_BODIES: usize = 8;

/// Half-extent of the randomized world, in metres.
const WORLD_HALF_EXTENT_M: f64 = 6.0;

/// Largest randomized displacement over one tick, in metres per tick.
const DISPLACEMENT_M: f64 = 2.5;

/// Draw a uniform `f64` in `[low, high)`.
fn uniform(rng: &mut ChaCha20Rng, low: f64, high: f64) -> f64 {
    let unit = (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
    low + unit * (high - low)
}

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

fn swept(shape: BodyShape, displacement_m: DVec2) -> SweptBody {
    SweptBody {
        shape,
        displacement_m,
    }
}

fn still(shape: BodyShape) -> SweptBody {
    swept(shape, DVec2::ZERO)
}

/// The signed clearance in metres between two swept bodies at `fraction` of the
/// tick, taken from the static query exactly as a caller would.
fn clearance_at(first: &SweptBody, second: &SweptBody, fraction: f64) -> f64 {
    body_clearance_m(&first.shape_at(fraction), &second.shape_at(fraction))
}

/// The relative displacement of the second body, in metres per tick.
fn relative_m(first: &SweptBody, second: &SweptBody) -> DVec2 {
    second.displacement_m - first.displacement_m
}

/// The first band entry the uniform sample grid witnesses, refined by a plain
/// bisection between the last clear sample and the first sample inside the
/// band.
///
/// The refinement is sound because the band entry set of two convex bodies
/// translating linearly is a single interval, so the clear-then-inside
/// predicate is monotone across that bracket. `None` means no sample is inside
/// the band.
fn reference_entry(first: &SweptBody, second: &SweptBody) -> Option<f64> {
    let width = 1.0 / SAMPLES as f64;
    let mut clear_sample = 0.0;
    for index in 0..=SAMPLES {
        let sample = index as f64 * width;
        if clearance_at(first, second, sample) <= CONTACT_BAND_M {
            if index == 0 {
                return Some(0.0);
            }
            let (mut low, mut high) = (clear_sample, sample);
            for _ in 0..80 {
                let middle = 0.5 * (low + high);
                if clearance_at(first, second, middle) <= CONTACT_BAND_M {
                    high = middle;
                } else {
                    low = middle;
                }
            }
            return Some(high);
        }
        clear_sample = sample;
    }
    None
}

/// A lower bound on the tick's minimum clearance, in metres, from the sample
/// grid.
///
/// The clearance between two bodies is 1-Lipschitz in each body's translation,
/// so between two samples half a grid step apart it can change by at most the
/// relative displacement times that half step. When this bound is above the
/// band, the tick provably holds no contact.
fn sampled_clearance_lower_bound_m(first: &SweptBody, second: &SweptBody) -> f64 {
    let width = 1.0 / SAMPLES as f64;
    let mut minimum = f64::INFINITY;
    for index in 0..=SAMPLES {
        minimum = minimum.min(clearance_at(first, second, index as f64 * width));
    }
    minimum - relative_m(first, second).length() * 0.5 * width
}

/// One randomized body of either shape.
fn random_body(rng: &mut ChaCha20Rng) -> BodyShape {
    let centre = DVec2::new(
        uniform(rng, -WORLD_HALF_EXTENT_M, WORLD_HALF_EXTENT_M),
        uniform(rng, -WORLD_HALF_EXTENT_M, WORLD_HALF_EXTENT_M),
    );
    if rng.next_u64() & 1 == 0 {
        circle(centre.x, centre.y, uniform(rng, 0.2, 1.0))
    } else {
        box_body(
            centre.x,
            centre.y,
            uniform(rng, 0.0, std::f64::consts::TAU),
            uniform(rng, 0.6, 2.5),
            uniform(rng, 0.3, 1.2),
        )
    }
}

/// One randomized swept body: a random shape with a random displacement.
fn random_swept_body(rng: &mut ChaCha20Rng) -> SweptBody {
    swept(
        random_body(rng),
        DVec2::new(
            uniform(rng, -DISPLACEMENT_M, DISPLACEMENT_M),
            uniform(rng, -DISPLACEMENT_M, DISPLACEMENT_M),
        ),
    )
}

/// A randomized world of uniquely identified swept bodies.
fn random_world(seed: u64) -> Vec<(AgentId, SweptBody)> {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    (0..WORLD_BODIES)
        .map(|index| (AgentId::from_index(index), random_swept_body(&mut rng)))
        .collect()
}

/// The thin wall of the box fixtures: a `width_m` thick box across the y axis.
fn thin_wall(width_m: f64) -> BodyShape {
    box_body(0.0, 0.0, 0.0, width_m, 4.0)
}

/// A 4 m by 2 m box at `centre_x` travelling along +x by `displacement_x`.
fn crossing_box(centre_x: f64, displacement_x: f64) -> SweptBody {
    swept(
        box_body(centre_x, 0.0, 0.0, 4.0, 2.0),
        DVec2::new(displacement_x, 0.0),
    )
}

#[test]
fn a_fast_box_crosses_a_thin_box_with_no_endpoint_overlap() {
    // The 4 m box starts half a metre clear of the wall and ends past it, so
    // neither tick endpoint overlaps. It is 0.29 of the tick away from first
    // contact: the box's front face is at -3 + 10t and the wall's left face at
    // -0.1.
    let fast = crossing_box(-5.0, 10.0);
    let wall = still(thin_wall(0.2));
    assert!(!bodies_intersect(&fast.shape_at(0.0), &wall.shape_at(0.0)));
    assert!(!bodies_intersect(&fast.shape_at(1.0), &wall.shape_at(1.0)));
    assert!(bodies_intersect(&fast.shape_at(0.5), &wall.shape_at(0.5)));

    let hit = time_of_impact(&fast, &wall).expect("the crossing box enters the wall");
    assert!(
        (hit.time - 0.29).abs() <= TOI_TOLERANCE,
        "first contact at {}",
        hit.time
    );
    assert!((hit.normal - DVec2::X).length() <= NORMAL_TOLERANCE);
    assert!(hit.clearance_m <= CONTACT_BAND_M);
    // The wall is 0.2 m thick and the box crosses it entirely, so the window is
    // narrow relative to the tick: a sample of the endpoints alone misses it.
    let exit = 0.31;
    assert!(
        hit.time < exit,
        "the cast reports the first entry, not the exit"
    );

    // The same pair is a swept candidate pair and no static candidate pair.
    let mut static_phase = BroadPhase::default();
    static_phase.rebuild(&[
        (AgentId::from_index(0), fast.shape),
        (AgentId::from_index(1), wall.shape),
    ]);
    let mut pairs = Vec::new();
    static_phase.candidate_pairs(&mut pairs);
    assert!(
        pairs.is_empty(),
        "the start-bound index cannot pair the crossing"
    );
    let mut swept_phase = SweptBroadPhase::default();
    swept_phase.rebuild(&[
        (AgentId::from_index(0), fast),
        (AgentId::from_index(1), wall),
    ]);
    swept_phase.candidate_pairs(&mut pairs);
    assert_eq!(
        pairs,
        vec![(AgentId::from_index(0), AgentId::from_index(1))]
    );
}

#[test]
fn a_fast_circle_crosses_a_circle_with_no_endpoint_overlap() {
    // The first circle runs from x = -6 to x = 6 through a still circle at the
    // origin; the two are clear at both tick endpoints. First touch is where
    // 6 - 12t = 2, at a third of the tick.
    let fast = swept(circle(-6.0, 0.0, 1.0), DVec2::new(12.0, 0.0));
    let waiting = still(circle(0.0, 0.0, 1.0));
    assert!(!bodies_intersect(
        &fast.shape_at(0.0),
        &waiting.shape_at(0.0)
    ));
    assert!(!bodies_intersect(
        &fast.shape_at(1.0),
        &waiting.shape_at(1.0)
    ));
    assert!(bodies_intersect(
        &fast.shape_at(0.5),
        &waiting.shape_at(0.5)
    ));

    let hit = time_of_impact(&fast, &waiting).expect("the crossing circle touches");
    assert!(
        (hit.time - 1.0 / 3.0).abs() <= TOI_TOLERANCE,
        "first contact at {}",
        hit.time
    );
    assert!((hit.normal - DVec2::X).length() <= NORMAL_TOLERANCE);
    assert!(hit.clearance_m <= CONTACT_BAND_M);
}

#[test]
fn crossing_diagonal_paths_touch_at_the_analytic_time_and_normal() {
    // One circle runs along +x through the origin while another runs along +y;
    // their segments cross at the origin. The centres are at (-3 + 6t, 0) and
    // (0, -3 + 6t), so the squared distance is 2(3 - 6t)^2 and contact at unit
    // radii is where 3 - 6t = 1/sqrt(2).
    let along_x = swept(circle(-3.0, 0.0, 0.5), DVec2::new(6.0, 0.0));
    let along_y = swept(circle(0.0, -3.0, 0.5), DVec2::new(0.0, 6.0));
    let expected_time = (3.0 - std::f64::consts::FRAC_1_SQRT_2) / 6.0;
    // At contact the first centre is at (-0.7071, 0) and the second at
    // (0, -0.7071), so the normal runs from the first toward the second.
    let expected_normal = DVec2::new(1.0, -1.0).normalize();

    assert!(!bodies_intersect(
        &along_x.shape_at(0.0),
        &along_y.shape_at(0.0)
    ));
    assert!(!bodies_intersect(
        &along_x.shape_at(1.0),
        &along_y.shape_at(1.0)
    ));

    let hit = time_of_impact(&along_x, &along_y).expect("the crossing paths touch");
    assert!(
        (hit.time - expected_time).abs() <= TOI_TOLERANCE,
        "first contact at {} not {expected_time}",
        hit.time
    );
    assert!(
        (hit.normal - expected_normal).length() <= NORMAL_TOLERANCE,
        "contact normal {:?}",
        hit.normal
    );
    assert!(hit.clearance_m <= CONTACT_BAND_M);
}

#[test]
fn a_crossing_is_found_however_narrow_the_window() {
    // The crossing box starts with its front face one gap ahead of the wall's
    // left face, so the analytic first contact is gap / displacement whatever
    // the wall's thickness and however narrow the overlap afterwards.
    const GAP_M: f64 = 0.5;
    for width_m in [0.4, 0.2, 0.1, 0.05, 0.02] {
        for displacement_m in [5.0, 10.0, 25.0, 100.0] {
            let fast = crossing_box(-(2.0 + width_m * 0.5 + GAP_M), displacement_m);
            let wall = still(thin_wall(width_m));
            assert!(
                !bodies_intersect(&fast.shape_at(0.0), &wall.shape_at(0.0))
                    && !bodies_intersect(&fast.shape_at(1.0), &wall.shape_at(1.0)),
                "width {width_m} displacement {displacement_m} must not overlap at the endpoints"
            );
            let expected_time = GAP_M / displacement_m;
            let hit = time_of_impact(&fast, &wall).unwrap_or_else(|| {
                panic!("width {width_m} displacement {displacement_m} must be detected")
            });
            assert!(
                (hit.time - expected_time).abs() <= TOI_TOLERANCE,
                "width {width_m} displacement {displacement_m}: {} not {expected_time}",
                hit.time
            );
            assert!((hit.normal - DVec2::X).length() <= NORMAL_TOLERANCE);
        }
    }
}

#[test]
fn start_overlap_touching_and_parallel_motion_read_as_documented() {
    // Already overlapping at the tick start: time zero, a negative clearance,
    // and the direction that separates with the least motion.
    let first = still(circle(0.0, 0.0, 1.0));
    let second = swept(circle(1.5, 0.0, 1.0), DVec2::new(1.0, 0.0));
    let hit = time_of_impact(&first, &second).expect("start overlap is a contact");
    assert_eq!(hit.time, 0.0);
    assert!((hit.clearance_m + 0.5).abs() <= TOI_TOLERANCE);
    assert!((hit.normal - DVec2::X).length() <= NORMAL_TOLERANCE);

    // Exactly touching at the tick start is a contact at time zero.
    let touching = swept(circle(2.0, 0.0, 1.0), DVec2::new(3.0, 0.0));
    let hit = time_of_impact(&first, &touching).expect("exact touching is a contact");
    assert_eq!(hit.time, 0.0);
    assert!(hit.clearance_m.abs() <= TOI_TOLERANCE);

    // Parallel motion, identical motion, and bodies that never close all keep
    // the clearance they start with.
    let lead = swept(circle(0.0, 0.0, 1.0), DVec2::new(4.0, 0.0));
    assert_eq!(
        time_of_impact(&lead, &swept(circle(0.0, 3.0, 1.0), DVec2::new(4.0, 0.0))),
        None
    );
    // A crossing of unmatched speeds: the mover runs along -y past a point the
    // lead reaches at half a tick, and the closest approach stays 0.42 m clear.
    assert_eq!(
        time_of_impact(&lead, &swept(circle(2.0, 3.0, 1.0), DVec2::new(0.0, -1.0))),
        None
    );
    assert_eq!(
        time_of_impact(&still(circle(0.0, 0.0, 1.0)), &still(circle(5.0, 0.0, 1.0))),
        None
    );
}

#[test]
fn swept_candidates_match_the_all_pairs_swept_bound_reference() {
    for seed in SEEDS {
        let bodies = random_world(seed);
        let mut phase = SweptBroadPhase::default();
        phase.rebuild(&bodies);

        let mut pairs = Vec::new();
        phase.candidate_pairs(&mut pairs);
        assert!(
            pairs.windows(2).all(|window| window[0] < window[1]),
            "pairs must be ascending and unique: seed {seed}"
        );

        let mut reference = BTreeSet::new();
        for i in 0..bodies.len() {
            for j in (i + 1)..bodies.len() {
                if bodies[i]
                    .1
                    .swept_bounds()
                    .overlaps(&bodies[j].1.swept_bounds())
                {
                    reference.insert((bodies[i].0.get(), bodies[j].0.get()));
                }
            }
        }
        let indexed: BTreeSet<(u32, u32)> = pairs
            .iter()
            .map(|(first, second)| (first.get(), second.get()))
            .collect();
        assert_eq!(
            indexed, reference,
            "the swept pair set must equal the all-pairs swept-bound reference: seed {seed}"
        );

        // Every body whose swept bound overlaps the query is returned, in
        // ascending id order without repeats.
        let query = bodies[seed as usize % bodies.len()].1.swept_bounds();
        let mut candidates = Vec::new();
        phase.candidates_overlapping(query, &mut candidates);
        assert!(
            candidates.windows(2).all(|window| window[0] < window[1]),
            "candidates must be ascending and unique: seed {seed}"
        );
        for (id, body) in &bodies {
            if body.swept_bounds().overlaps(&query) {
                assert!(
                    candidates.contains(id),
                    "body {} whose swept bound overlaps the query must be a candidate: seed {seed}",
                    id.get()
                );
            }
        }
    }
}

#[test]
fn swept_candidate_pairs_cover_every_pair_that_touches_in_the_tick() {
    for seed in SEEDS {
        let bodies = random_world(seed);

        // The all-pairs reference: every pair that touches during the tick.
        let mut reference = BTreeSet::new();
        for i in 0..bodies.len() {
            for j in (i + 1)..bodies.len() {
                if time_of_impact(&bodies[i].1, &bodies[j].1).is_some() {
                    reference.insert((bodies[i].0.get(), bodies[j].0.get()));
                }
            }
        }

        let mut phase = SweptBroadPhase::default();
        phase.rebuild(&bodies);
        let mut pairs = Vec::new();
        phase.candidate_pairs(&mut pairs);
        let indexed: BTreeSet<(u32, u32)> = pairs
            .iter()
            .filter(|(first, second)| {
                time_of_impact(&bodies[first.index()].1, &bodies[second.index()].1).is_some()
            })
            .map(|(first, second)| (first.get(), second.get()))
            .collect();
        assert_eq!(
            indexed, reference,
            "filtering the swept candidates must recover exactly the all-pairs contacts: seed {seed}"
        );

        // The start-bound index is a subset: its pairs are also swept pairs,
        // and its own contacts are covered too.
        let start_bodies: Vec<(AgentId, BodyShape)> =
            bodies.iter().map(|(id, body)| (*id, body.shape)).collect();
        let mut static_phase = BroadPhase::default();
        static_phase.rebuild(&start_bodies);
        let mut static_pairs = Vec::new();
        static_phase.candidate_pairs(&mut static_pairs);
        let swept_pairs: BTreeSet<(u32, u32)> = pairs
            .iter()
            .map(|(first, second)| (first.get(), second.get()))
            .collect();
        for (first, second) in &static_pairs {
            assert!(
                swept_pairs.contains(&(first.get(), second.get())),
                "every start-bound candidate pair must also be a swept pair: seed {seed}"
            );
        }
    }
}

#[test]
fn the_swept_bound_contains_both_endpoint_bounds() {
    for seed in SEEDS {
        for (_, body) in random_world(seed) {
            let swept_bounds = body.swept_bounds();
            for shape in [body.shape, body.end_shape()] {
                let endpoint = shape.bounds();
                assert!(
                    swept_bounds.min.x <= endpoint.min.x && swept_bounds.min.y <= endpoint.min.y
                );
                assert!(
                    swept_bounds.max.x >= endpoint.max.x && swept_bounds.max.y >= endpoint.max.y
                );
            }
            // The reach bound covers the swept volume around the start centre.
            assert!(body.swept_reach_m() >= body.shape.circumradius_m());
        }
    }
}

#[test]
fn casts_agree_with_the_uniform_sampling_reference() {
    let mut witnessed = 0;
    for seed in SEEDS {
        let bodies = random_world(seed);
        for i in 0..bodies.len() {
            for j in (i + 1)..bodies.len() {
                let (first, second) = (&bodies[i].1, &bodies[j].1);
                let context = format!(
                    "seed {seed} pair ({}, {})",
                    bodies[i].0.get(),
                    bodies[j].0.get()
                );
                match reference_entry(first, second) {
                    Some(entry) => {
                        witnessed += 1;
                        let hit = time_of_impact(first, second).unwrap_or_else(|| {
                            panic!("the reference witnessed a contact: {context}")
                        });
                        assert!(
                            (hit.time - entry).abs() <= TOI_TOLERANCE,
                            "cast {} versus reference {entry}: {context}",
                            hit.time
                        );
                        assert!(hit.clearance_m <= CONTACT_BAND_M, "{context}");
                    }
                    None => {
                        // Only assert an absence the reference can prove.
                        let bound = sampled_clearance_lower_bound_m(first, second);
                        assert!(
                            bound > CONTACT_BAND_M,
                            "the reference is inconclusive for {context}: bound {bound}"
                        );
                        assert_eq!(time_of_impact(first, second), None, "{context}");
                    }
                }
            }
        }
    }
    assert!(witnessed > 0, "the randomized worlds must contain contacts");
}

#[test]
fn every_cast_keeps_its_documented_invariants() {
    for seed in SEEDS {
        let bodies = random_world(seed);
        for i in 0..bodies.len() {
            for j in (i + 1)..bodies.len() {
                let (first, second) = (&bodies[i].1, &bodies[j].1);
                let context = format!(
                    "seed {seed} pair ({}, {})",
                    bodies[i].0.get(),
                    bodies[j].0.get()
                );
                let forward = time_of_impact(first, second);
                let backward = time_of_impact(second, first);
                let Some(hit) = forward else {
                    assert!(backward.is_none(), "asymmetric absence: {context}");
                    continue;
                };
                assert!(
                    (0.0..=1.0).contains(&hit.time),
                    "time {} out of range: {context}",
                    hit.time
                );
                assert!(
                    (hit.normal.length() - 1.0).abs() <= 1e-9,
                    "normal must be a unit vector: {context}"
                );
                assert!(hit.clearance_m <= CONTACT_BAND_M, "{context}");
                // The reported time is the first band entry: no earlier sample
                // is inside the band beyond the declared resolution.
                let width = 1.0 / SAMPLES as f64;
                for index in 0..SAMPLES {
                    let sample = index as f64 * width;
                    if sample < hit.time - TOI_TOLERANCE {
                        assert!(
                            clearance_at(first, second, sample) > CONTACT_BAND_M,
                            "sample {sample} before the reported entry {}: {context}",
                            hit.time
                        );
                    }
                }
                let mirror = backward.expect("a cast is symmetric in its arguments");
                assert!(
                    (hit.time - mirror.time).abs() <= TOI_TOLERANCE,
                    "mirrored time: {context}"
                );
                assert!(
                    (hit.normal + mirror.normal).length() <= NORMAL_TOLERANCE,
                    "mirrored normal: {context}"
                );
            }
        }
    }
}
