//! Phase 1 Increment 4 slice A: deterministic broad phase and exact geometry
//! queries.
//!
//! These tests are property tests over randomized small worlds. They are the
//! gate `PHASE_1_PLAN.md` Increment 4 states for this slice: indexed candidates
//! are compared with an all-pairs reference, and exact query results are
//! compared with a brute-force reference.
//!
//! The broad phase is [`BroadPhase`]: a uniform grid rebuilt from body shapes
//! and keyed by [`AgentId`]. The exact queries are
//! [`body_clearance_m`] and [`bodies_intersect`]. Everything below is an
//! independent brute-force reference written from the shape geometry, never a
//! call back into the code under test, so a bug cannot hide behind a shared
//! implementation.
//!
//! # Declared tolerances and ordering
//!
//! - [`QUERY_TOLERANCE_M`] is the absolute metre tolerance for comparing a
//!   query result with its reference. The world spans a few metres and `f64`
//!   carries about 15 significant digits, so `1e-9 m` is far above rounding
//!   noise and far below any physical scale the queries resolve.
//! - [`CONTACT_BAND_M`] is the band around contact within which the sign of
//!   the clearance is not compared: at exact contact the two algorithms may
//!   disagree which side of zero a value falls on. The band keeps that
//!   measure-zero set out of the assertion without weakening the disjoint and
//!   overlapping cases.
//! - Candidate order is ascending `AgentId`; candidate pairs are ascending
//!   `(first, second)` with `first < second`; no candidate or pair repeats.
//!   Every randomized assertion below checks that order and uniqueness.

use std::collections::BTreeSet;

use glam::DVec2;
use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::{Rng, SeedableRng};

use hekate_sim::{AgentId, BodyShape, BroadPhase, bodies_intersect, body_clearance_m};

/// Absolute tolerance in metres for a query-versus-reference comparison.
const QUERY_TOLERANCE_M: f64 = 1e-9;

/// Band around contact within which the clearance sign is not compared.
const CONTACT_BAND_M: f64 = 1e-7;

/// Fixed seeds for the randomized worlds; the sweep is reproducible.
const SEEDS: [u64; 12] = [1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233];

/// Bodies per randomized world.
const WORLD_BODIES: usize = 16;

/// Half-extent of the randomized world, in metres.
const WORLD_HALF_EXTENT_M: f64 = 8.0;

/// Draw a uniform `f64` in `[low, high)`.
fn uniform(rng: &mut ChaCha20Rng, low: f64, high: f64) -> f64 {
    let unit = (rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
    low + unit * (high - low)
}

/// One randomized body of either shape.
fn random_body(rng: &mut ChaCha20Rng) -> BodyShape {
    let centre = DVec2::new(
        uniform(rng, -WORLD_HALF_EXTENT_M, WORLD_HALF_EXTENT_M),
        uniform(rng, -WORLD_HALF_EXTENT_M, WORLD_HALF_EXTENT_M),
    );
    if rng.next_u64() & 1 == 0 {
        BodyShape::Circle {
            centre,
            radius_m: uniform(rng, 0.2, 1.5),
        }
    } else {
        BodyShape::Box {
            centre,
            heading_rad: uniform(rng, 0.0, std::f64::consts::TAU),
            length_m: uniform(rng, 0.6, 3.0),
            width_m: uniform(rng, 0.3, 1.5),
        }
    }
}

/// A randomized world of uniquely identified bodies.
fn random_world(seed: u64) -> Vec<(AgentId, BodyShape)> {
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    (0..WORLD_BODIES)
        .map(|index| (AgentId::from_index(index), random_body(&mut rng)))
        .collect()
}

/// Four corners of an oriented box in ring order, in world metres.
fn box_corners(body: &BodyShape) -> [DVec2; 4] {
    let BodyShape::Box {
        centre,
        heading_rad,
        length_m,
        width_m,
    } = *body
    else {
        panic!("box_corners called on a circle body");
    };
    let (sin, cos) = heading_rad.sin_cos();
    let forward = DVec2::new(cos, sin) * (length_m * 0.5);
    let left = DVec2::new(-sin, cos) * (width_m * 0.5);
    [
        centre + forward + left,
        centre + forward - left,
        centre - forward - left,
        centre - forward + left,
    ]
}

/// Distance in metres from a point to a line segment.
fn point_segment_distance(point: DVec2, start: DVec2, end: DVec2) -> f64 {
    let segment = end - start;
    let length_sq = segment.length_squared();
    let fraction = if length_sq > 0.0 {
        ((point - start).dot(segment) / length_sq).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (point - (start + segment * fraction)).length()
}

/// Whether a point lies inside a convex quadrilateral, by the cross-product
/// sign test. Independent of the separating-axis test.
fn point_in_convex_polygon(point: DVec2, polygon: &[DVec2; 4]) -> bool {
    let mut sign = 0.0;
    for index in 0..4 {
        let start = polygon[index];
        let end = polygon[(index + 1) % 4];
        let cross = (end - start).perp_dot(point - start);
        if cross.abs() > 1e-12 {
            let this = cross.signum();
            if sign == 0.0 {
                sign = this;
            } else if this != sign {
                return false;
            }
        }
    }
    true
}

/// Whether two segments properly cross or touch.
fn segments_intersect(a0: DVec2, a1: DVec2, b0: DVec2, b1: DVec2) -> bool {
    let d1 = (a1 - a0).perp_dot(b0 - a0);
    let d2 = (a1 - a0).perp_dot(b1 - a0);
    let d3 = (b1 - b0).perp_dot(a0 - b0);
    let d4 = (b1 - b0).perp_dot(a1 - b0);
    ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0))
}

/// Whether two convex boxes intersect, by containment-or-edge-crossing.
/// Independent of the separating-axis test the crate uses.
fn boxes_intersect(first: &BodyShape, second: &BodyShape) -> bool {
    let a = box_corners(first);
    let b = box_corners(second);
    if a.iter().any(|&corner| point_in_convex_polygon(corner, &b)) {
        return true;
    }
    if b.iter().any(|&corner| point_in_convex_polygon(corner, &a)) {
        return true;
    }
    for i in 0..4 {
        for j in 0..4 {
            if segments_intersect(a[i], a[(i + 1) % 4], b[j], b[(j + 1) % 4]) {
                return true;
            }
        }
    }
    false
}

/// Signed distance in metres from a point to a convex quadrilateral: negative
/// when the point is inside, the nearest-edge distance otherwise.
fn signed_point_to_box(point: DVec2, body: &BodyShape) -> f64 {
    let corners = box_corners(body);
    let mut nearest_edge = f64::INFINITY;
    for index in 0..4 {
        nearest_edge = nearest_edge.min(point_segment_distance(
            point,
            corners[index],
            corners[(index + 1) % 4],
        ));
    }
    if point_in_convex_polygon(point, &corners) {
        -nearest_edge
    } else {
        nearest_edge
    }
}

/// Brute-force signed clearance in metres between two bodies, matching the
/// crate's documented semantics but computed independently.
fn reference_clearance_m(first: &BodyShape, second: &BodyShape) -> f64 {
    match (first, second) {
        (
            BodyShape::Circle {
                centre: a,
                radius_m: ra,
            },
            BodyShape::Circle {
                centre: b,
                radius_m: rb,
            },
        ) => (*b - *a).length() - ra - rb,
        (BodyShape::Circle { centre, radius_m }, BodyShape::Box { .. }) => {
            signed_point_to_box(*centre, second) - radius_m
        }
        (BodyShape::Box { .. }, BodyShape::Circle { centre, radius_m }) => {
            signed_point_to_box(*centre, first) - radius_m
        }
        (BodyShape::Box { .. }, BodyShape::Box { .. }) => {
            // Minimum vertex-to-edge distance over both boxes. For disjoint
            // convex polygons this equals the polygon distance; for overlapping
            // boxes it is not meaningful, so callers use it only when disjoint.
            let a = box_corners(first);
            let b = box_corners(second);
            let mut best = f64::INFINITY;
            for i in 0..4 {
                for j in 0..4 {
                    best = best.min(point_segment_distance(a[i], b[j], b[(j + 1) % 4]));
                    best = best.min(point_segment_distance(b[i], a[j], a[(j + 1) % 4]));
                }
            }
            best
        }
    }
}

/// Brute-force intersection predicate, independent of the crate's queries.
fn reference_intersects(first: &BodyShape, second: &BodyShape) -> bool {
    match (first, second) {
        (
            BodyShape::Circle {
                centre: a,
                radius_m: ra,
            },
            BodyShape::Circle {
                centre: b,
                radius_m: rb,
            },
        ) => (*b - *a).length() <= ra + rb,
        (BodyShape::Circle { centre, radius_m }, BodyShape::Box { .. }) => {
            signed_point_to_box(*centre, second) <= *radius_m
        }
        (BodyShape::Box { .. }, BodyShape::Circle { centre, radius_m }) => {
            signed_point_to_box(*centre, first) <= *radius_m
        }
        (BodyShape::Box { .. }, BodyShape::Box { .. }) => boxes_intersect(first, second),
    }
}

/// All unordered pairs `i < j` whose enclosing boxes overlap, as a set of id
/// pairs.
fn reference_overlapping_bounds(bodies: &[(AgentId, BodyShape)]) -> BTreeSet<(u32, u32)> {
    let mut pairs = BTreeSet::new();
    for i in 0..bodies.len() {
        for j in (i + 1)..bodies.len() {
            if bodies[i].1.bounds().overlaps(&bodies[j].1.bounds()) {
                pairs.insert((bodies[i].0.get(), bodies[j].0.get()));
            }
        }
    }
    pairs
}

#[test]
fn exact_queries_match_the_brute_force_reference() {
    for seed in SEEDS {
        let bodies = random_world(seed);
        for i in 0..bodies.len() {
            for j in (i + 1)..bodies.len() {
                let (first, second) = (&bodies[i].1, &bodies[j].1);
                let clearance = body_clearance_m(first, second);
                let reference = reference_clearance_m(first, second);
                let context = format!(
                    "seed {seed} pair ({}, {}) clearance {clearance} reference {reference}",
                    bodies[i].0.get(),
                    bodies[j].0.get()
                );

                // Symmetric in the arguments.
                assert!(
                    (clearance - body_clearance_m(second, first)).abs() <= QUERY_TOLERANCE_M,
                    "asymmetric clearance: {context}"
                );

                match (first, second) {
                    (BodyShape::Box { .. }, BodyShape::Box { .. }) => {
                        // The reference distance is only meaningful when the
                        // boxes are disjoint through a separating axis.
                        if clearance > CONTACT_BAND_M {
                            assert!(
                                (clearance - reference).abs() <= QUERY_TOLERANCE_M,
                                "disjoint box/box distance mismatch: {context}"
                            );
                        }
                    }
                    _ => {
                        // Circle/circle and circle/box references are exact in
                        // every configuration.
                        assert!(
                            (clearance - reference).abs() <= QUERY_TOLERANCE_M,
                            "exact query mismatch: {context}"
                        );
                    }
                }

                if clearance.abs() > CONTACT_BAND_M {
                    assert_eq!(
                        bodies_intersect(first, second),
                        reference_intersects(first, second),
                        "intersection disagreement: {context}"
                    );
                    assert_eq!(
                        bodies_intersect(first, second),
                        clearance < 0.0,
                        "clearance sign and intersection disagree: {context}"
                    );
                }
            }
        }
    }
}

#[test]
fn broad_phase_candidates_cover_every_overlapping_pair() {
    for seed in SEEDS {
        let bodies = random_world(seed);
        let mut phase = BroadPhase::default();
        phase.rebuild(&bodies);

        let mut pairs = Vec::new();
        phase.candidate_pairs(&mut pairs);
        assert!(
            pairs.windows(2).all(|window| window[0] < window[1]),
            "candidate pairs must be ascending and unique: seed {seed}"
        );

        let reference = reference_overlapping_bounds(&bodies);
        let indexed: BTreeSet<(u32, u32)> = pairs
            .iter()
            .map(|(first, second)| (first.get(), second.get()))
            .collect();
        assert_eq!(
            indexed, reference,
            "indexed bounds-overlapping pair set must equal the all-pairs reference: seed {seed}"
        );

        // Every truly overlapping pair is a candidate, and filtering the
        // candidates by the exact test recovers exactly the all-pairs exact
        // result.
        let mut exact_all = BTreeSet::new();
        for i in 0..bodies.len() {
            for j in (i + 1)..bodies.len() {
                if bodies_intersect(&bodies[i].1, &bodies[j].1) {
                    exact_all.insert((bodies[i].0.get(), bodies[j].0.get()));
                }
            }
        }
        let exact_indexed: BTreeSet<(u32, u32)> = pairs
            .iter()
            .filter(|(first, second)| {
                bodies_intersect(&bodies[first.index()].1, &bodies[second.index()].1)
            })
            .map(|(first, second)| (first.get(), second.get()))
            .collect();
        assert_eq!(
            exact_indexed, exact_all,
            "indexed exact result must equal the all-pairs exact result: seed {seed}"
        );
    }
}

#[test]
fn broad_phase_query_returns_every_reachable_body() {
    for seed in SEEDS {
        let bodies = random_world(seed);
        let mut phase = BroadPhase::default();
        phase.rebuild(&bodies);

        // Use one world body as the query, with a fresh RNG stream.
        let mut rng = ChaCha20Rng::seed_from_u64(seed ^ 0x5eed);
        let query = random_body(&mut rng);
        let bounds = query.bounds();

        let mut candidates = Vec::new();
        phase.candidates_overlapping(bounds, &mut candidates);
        assert!(
            candidates.windows(2).all(|window| window[0] < window[1]),
            "candidates must be ascending and unique: seed {seed}"
        );

        for (id, body) in &bodies {
            if body.bounds().overlaps(&bounds) {
                assert!(
                    candidates.contains(id),
                    "body {} whose bounds overlap the query must be a candidate: seed {seed}",
                    id.get()
                );
            }
        }
    }
}

#[test]
fn candidate_pairs_never_exceed_the_enclosing_box_test() {
    // The broad phase is an enclosing-box filter, so its pair set is
    // sandwiched: every returned pair has overlapping boxes, and every pair
    // the exact geometry intersects is returned. It is deliberately not a
    // near-contact filter: two overlapping boxes can hold bodies a third of a
    // metre apart, and the exact test rejects exactly those candidates, so no
    // assertion here bounds a rejected candidate's clearance.
    for seed in SEEDS {
        let bodies = random_world(seed);
        let mut phase = BroadPhase::default();
        phase.rebuild(&bodies);
        let mut pairs = Vec::new();
        phase.candidate_pairs(&mut pairs);

        let mut returned = BTreeSet::new();
        for &(first, second) in &pairs {
            returned.insert((first.get(), second.get()));
            let first_bounds = bodies[first.index()].1.bounds();
            let second_bounds = bodies[second.index()].1.bounds();
            assert!(
                first_bounds.overlaps(&second_bounds),
                "candidate ({}, {}) must have overlapping enclosing boxes: seed {seed}",
                first.get(),
                second.get()
            );
        }

        // Completeness against the independent brute-force predicate: a pair
        // whose exact bodies intersect can never be missing from the
        // candidates, whatever the box test decides about its neighbours.
        for i in 0..bodies.len() {
            for j in (i + 1)..bodies.len() {
                if reference_intersects(&bodies[i].1, &bodies[j].1) {
                    assert!(
                        returned.contains(&(bodies[i].0.get(), bodies[j].0.get())),
                        "intersecting pair ({}, {}) must be a candidate: seed {seed}",
                        bodies[i].0.get(),
                        bodies[j].0.get()
                    );
                }
            }
        }
    }
}
