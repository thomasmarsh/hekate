//! Deterministic uniform-grid broad phase over every live agent, plus the
//! spatial predicates a candidate query feeds.
//!
//! # Model card
//!
//! `PHASE_1_PLAN.md` Increment 4 ("Deterministic uniform-grid broad phase with
//! stable candidate ordering") needs one deterministic way to ask "which bodies
//! are near this region?" that both modes share, so vehicle yielding to an
//! occupied crossing is not a mode-specific shortcut. This module owns that
//! broad phase: a uniform grid rebuilt each tick from the shared, stable-order
//! [`AgentStore`], a bounding-box candidate query, and the candidate-pair
//! enumeration the exact queries in [`crate::query`] consume.
//!
//! The vehicle and pedestrian body shapes, the exact box/box, circle/circle,
//! and box/circle queries, and the signed-clearance tolerance live in
//! [`crate::query`]; this module only bounds and orders candidates.
//!
//! [`SweptBroadPhase`] is the swept variant of the same grid: it indexes each
//! body's tick-swept bound instead of its start bound, so a fast body that
//! crosses another between two ticks is still a candidate pair. The swept
//! bounds come from [`SweptBody`] and the time-of-impact cast a caller runs on
//! the pairs lives in [`crate::swept`].
//!
//! ## Determinism
//!
//! [`BroadPhase`] stores its cells in a `Vec` sorted by integer cell coordinate
//! and binary-searches that array on a query. Nothing here iterates a hash map,
//! so the candidate set and its order are a pure function of the indexed
//! bodies. Cells and their residents are sorted by [`AgentId`] at rebuild time,
//! and every query sorts its result ascending, so equal cells and exactly equal
//! positions are broken by [`AgentId`]. Every body occupies exactly one cell,
//! so a query never reports the same agent twice and a candidate pair never
//! repeats.
//!
//! ## Query shape
//!
//! [`BroadPhase::candidates_in_aabb`] returns every indexed body whose centre
//! cell overlaps an axis-aligned box, in ascending [`AgentId`] order.
//! [`BroadPhase::candidates_overlapping`] widens that box by the largest
//! circumradius among the indexed bodies, so a body whose centre is outside the
//! box is still returned when its extent reaches in; that is the bound the
//! broad phase guarantees. [`BroadPhase::candidate_pairs`] enumerates every
//! unordered pair whose enclosing axis-aligned boxes overlap, in ascending
//! `(AgentId, AgentId)` order. All three are candidate queries: a caller applies
//! its own exact shape test, such as [`crate::query::bodies_intersect`] or
//! [`circle_overlaps_ring`], to each result.

use glam::DVec2;

use crate::agent::{AgentId, AgentStore};
use crate::query::{self, Aabb, BodyShape};
use crate::swept::SweptBody;

/// Edge length in metres of one uniform-grid cell.
///
/// A property of the index, not of any scenario: the cell is sized so the
/// 4 m pedestrian sense radius and a crossing region span a small number of
/// cells. Increment 4 owns the calibrated cell-size and broad-phase work.
pub(crate) const GRID_CELL_SIZE_M: f64 = 4.0;

/// Integer uniform-grid cell coordinate, in units of the cell size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Cell {
    x: i32,
    y: i32,
}

/// A deterministic uniform-grid broad phase over body shapes keyed by
/// [`AgentId`].
///
/// Rebuild it once per tick from every live body; then any number of candidate
/// queries that tick share one consistent view. The input slice may be in any
/// order: rebuild sorts by [`AgentId`], so the result is a pure function of the
/// indexed bodies and not of insertion order.
#[derive(Debug, Clone)]
pub struct BroadPhase {
    cell_size_m: f64,
    /// Circumradius in metres of the largest indexed body; the uniform bound a
    /// query widens by.
    max_half_extent_m: f64,
    /// Enclosing box of every indexed body, in ascending [`AgentId`] order.
    bodies: Vec<(AgentId, Aabb)>,
    /// Cells in ascending `Cell` order; each holds the ids whose body centre
    /// falls in it, in ascending [`AgentId`] order.
    cells: Vec<(Cell, Vec<AgentId>)>,
}

impl Default for BroadPhase {
    fn default() -> Self {
        Self::new(GRID_CELL_SIZE_M)
    }
}

impl BroadPhase {
    /// A broad phase with the given cell edge length in metres.
    ///
    /// A non-finite or non-positive edge falls back to [`GRID_CELL_SIZE_M`], so
    /// the broad phase is always well defined.
    pub fn new(cell_size_m: f64) -> Self {
        let cell_size_m = if cell_size_m.is_finite() && cell_size_m > 0.0 {
            cell_size_m
        } else {
            GRID_CELL_SIZE_M
        };
        Self {
            cell_size_m,
            max_half_extent_m: 0.0,
            bodies: Vec::new(),
            cells: Vec::new(),
        }
    }

    /// The cell edge length in metres.
    pub fn cell_size_m(&self) -> f64 {
        self.cell_size_m
    }

    /// The largest indexed body circumradius in metres, the query-widening
    /// bound. Zero when no body is indexed.
    pub fn max_half_extent_m(&self) -> f64 {
        self.max_half_extent_m
    }

    /// Number of indexed bodies.
    pub fn len(&self) -> usize {
        self.bodies.len()
    }

    /// Whether no body is indexed.
    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty()
    }

    /// Rebuild from a set of uniquely identified bodies, in any input order.
    ///
    /// A rebuild is a pure function of the bodies, so rebuilding at the start
    /// of a tick gives every query that tick one consistent candidate view.
    pub fn rebuild(&mut self, bodies: &[(AgentId, BodyShape)]) {
        self.max_half_extent_m = bodies
            .iter()
            .map(|(_, body)| body.circumradius_m())
            .fold(0.0, f64::max);

        self.bodies.clear();
        self.bodies
            .extend(bodies.iter().map(|(id, body)| (*id, body.bounds())));
        self.bodies.sort_by_key(|(id, _)| *id);
        debug_assert!(
            self.bodies.windows(2).all(|pair| pair[0].0 < pair[1].0),
            "BroadPhase requires unique AgentIds"
        );

        let mut entries: Vec<(Cell, AgentId)> = bodies
            .iter()
            .map(|(id, body)| (self.cell_of(body.centre()), *id))
            .collect();
        // Sorting by cell then id puts equal cells in ascending id order
        // regardless of input order, so a cell's residents are documented.
        entries.sort_by_key(|(cell, id)| (*cell, *id));
        self.cells.clear();
        for (cell, id) in entries {
            match self.cells.last_mut() {
                Some((last, ids)) if *last == cell => ids.push(id),
                _ => self.cells.push((cell, vec![id])),
            }
        }
    }

    /// The bodies whose centre cell overlaps the axis-aligned `bounds`, in
    /// ascending [`AgentId`] order.
    ///
    /// The result replaces `out`; `out` is only cleared and re-filled, so a
    /// caller can reuse one buffer across a tick. This is the tight candidate
    /// query; a caller that does not already know the candidate extents should
    /// use [`Self::candidates_overlapping`].
    pub fn candidates_in_aabb(&self, bounds: Aabb, out: &mut Vec<AgentId>) {
        out.clear();
        let low = self.cell_of(bounds.min);
        let high = self.cell_of(bounds.max);
        for y in low.y..=high.y {
            for x in low.x..=high.x {
                let cell = Cell { x, y };
                if let Ok(position) = self.cells.binary_search_by_key(&cell, |(cell, _)| *cell) {
                    out.extend_from_slice(&self.cells[position].1);
                }
            }
        }
        // Cells are visited in row-major order, which is not the documented
        // order across cells; the ids within a cell are already ascending.
        out.sort_unstable();
    }

    /// The bodies that may overlap `bounds`, in ascending [`AgentId`] order.
    ///
    /// The query widens `bounds` by the largest indexed body circumradius, so
    /// every body whose extent reaches into `bounds` is returned; a caller
    /// applies the exact shape test to reject the rest.
    pub fn candidates_overlapping(&self, bounds: Aabb, out: &mut Vec<AgentId>) {
        self.candidates_in_aabb(bounds.expand(self.max_half_extent_m), out);
    }

    /// Every unordered candidate pair `(first, second)`, `first < second`,
    /// whose enclosing boxes overlap, in ascending `(AgentId, AgentId)` order.
    ///
    /// The pair set is a superset of every truly overlapping pair; a caller
    /// applies the exact shape test to each pair. Each unordered pair appears
    /// once and no pair repeats.
    pub fn candidate_pairs(&self, out: &mut Vec<(AgentId, AgentId)>) {
        out.clear();
        let mut candidates: Vec<AgentId> = Vec::new();
        for (first, first_bounds) in &self.bodies {
            self.candidates_overlapping(*first_bounds, &mut candidates);
            for second in &candidates {
                if second.index() <= first.index() {
                    continue;
                }
                if let Some(second_bounds) = self.bounds_of(*second)
                    && first_bounds.overlaps(&second_bounds)
                {
                    out.push((*first, *second));
                }
            }
        }
    }

    /// The enclosing box of one indexed body.
    fn bounds_of(&self, id: AgentId) -> Option<Aabb> {
        self.bodies
            .binary_search_by_key(&id, |(indexed, _)| *indexed)
            .ok()
            .map(|position| self.bodies[position].1)
    }

    /// The cell containing a world position.
    fn cell_of(&self, position: DVec2) -> Cell {
        Cell {
            x: (position.x / self.cell_size_m).floor() as i32,
            y: (position.y / self.cell_size_m).floor() as i32,
        }
    }
}

/// A deterministic uniform-grid broad phase over the swept volumes of bodies
/// moving through one tick.
///
/// It composes [`BroadPhase`]: the grid indexes each body's start centre, and
/// the bound stored per body is its tick-swept bound from
/// [`SweptBody::swept_bounds`] rather than its start bound. A query is widened
/// by the largest swept reach among the indexed bodies and every candidate is
/// then filtered by its own swept bound, so
/// [`Self::candidates_overlapping`] returns exactly the bodies whose swept bound
/// overlaps the query box.
///
/// That is the tunnelling protection the static broad phase cannot give: a fast
/// body leaves one side of a thin body and appears on the other with no overlap
/// at either tick endpoint, so a start-bound index never pairs them, while both
/// swept bounds cover the crossing point.
///
/// Ordering matches [`BroadPhase`]: a rebuild is a pure function of the indexed
/// bodies, candidates are ascending and unique by [`AgentId`], and pairs are
/// ascending `(AgentId, AgentId)` with `first < second`, each appearing once.
#[derive(Debug, Clone)]
pub struct SweptBroadPhase {
    grid: BroadPhase,
    /// Tick-swept bound of every indexed body, in ascending [`AgentId`] order.
    swept: Vec<(AgentId, Aabb)>,
    /// Reused start-shape buffer, so a rebuild does not allocate after warmup.
    shapes: Vec<(AgentId, BodyShape)>,
    /// Largest swept reach in metres, the uniform bound a query widens by.
    max_reach_m: f64,
}

impl Default for SweptBroadPhase {
    fn default() -> Self {
        Self::new(GRID_CELL_SIZE_M)
    }
}

impl SweptBroadPhase {
    /// A swept broad phase with the given cell edge length in metres.
    ///
    /// A non-finite or non-positive edge falls back to [`GRID_CELL_SIZE_M`], so
    /// the broad phase is always well defined.
    pub fn new(cell_size_m: f64) -> Self {
        Self {
            grid: BroadPhase::new(cell_size_m),
            swept: Vec::new(),
            shapes: Vec::new(),
            max_reach_m: 0.0,
        }
    }

    /// The cell edge length in metres.
    pub fn cell_size_m(&self) -> f64 {
        self.grid.cell_size_m()
    }

    /// The largest indexed swept reach in metres, the query-widening bound.
    /// Zero when no body is indexed.
    pub fn max_reach_m(&self) -> f64 {
        self.max_reach_m
    }

    /// Number of indexed bodies.
    pub fn len(&self) -> usize {
        self.swept.len()
    }

    /// Whether no body is indexed.
    pub fn is_empty(&self) -> bool {
        self.swept.is_empty()
    }

    /// Rebuild from a set of uniquely identified swept bodies, in any input
    /// order.
    ///
    /// A rebuild is a pure function of the bodies, so rebuilding at the start of
    /// a tick gives every swept query that tick one consistent view.
    pub fn rebuild(&mut self, bodies: &[(AgentId, SweptBody)]) {
        self.max_reach_m = bodies
            .iter()
            .map(|(_, body)| body.swept_reach_m())
            .fold(0.0, f64::max);
        self.shapes.clear();
        self.shapes
            .extend(bodies.iter().map(|(id, body)| (*id, body.shape)));
        self.grid.rebuild(&self.shapes);
        self.swept.clear();
        self.swept
            .extend(bodies.iter().map(|(id, body)| (*id, body.swept_bounds())));
        self.swept.sort_by_key(|(id, _)| *id);
        debug_assert!(
            self.swept.windows(2).all(|pair| pair[0].0 < pair[1].0),
            "SweptBroadPhase requires unique AgentIds"
        );
    }

    /// The tick-swept bound of one indexed body, in world metres.
    pub fn swept_bounds_of(&self, id: AgentId) -> Option<Aabb> {
        self.swept
            .binary_search_by_key(&id, |(indexed, _)| *indexed)
            .ok()
            .map(|position| self.swept[position].1)
    }

    /// The bodies whose tick-swept bound overlaps `bounds`, in ascending
    /// [`AgentId`] order.
    ///
    /// The result replaces `out`; `out` is only cleared and re-filled, so a
    /// caller can reuse one buffer across a tick. The swept query of one body's
    /// whole motion is this query on that body's [`SweptBody::swept_bounds`].
    pub fn candidates_overlapping(&self, bounds: Aabb, out: &mut Vec<AgentId>) {
        self.grid
            .candidates_in_aabb(bounds.expand(self.max_reach_m), out);
        out.retain(|id| {
            self.swept_bounds_of(*id)
                .is_some_and(|swept| swept.overlaps(&bounds))
        });
    }

    /// Every unordered candidate pair `(first, second)`, `first < second`, whose
    /// tick-swept bounds overlap, in ascending `(AgentId, AgentId)` order.
    ///
    /// The pair set is a superset of every pair that touches during the tick: a
    /// caller applies [`time_of_impact`](crate::swept::time_of_impact) or an
    /// exact static query to each pair. Each unordered pair appears once and no
    /// pair repeats.
    pub fn candidate_pairs(&self, out: &mut Vec<(AgentId, AgentId)>) {
        out.clear();
        let mut candidates: Vec<AgentId> = Vec::new();
        for (first, first_bounds) in &self.swept {
            self.candidates_overlapping(*first_bounds, &mut candidates);
            for second in &candidates {
                if second.index() <= first.index() {
                    continue;
                }
                out.push((*first, *second));
            }
        }
    }
}

/// The kernel's AgentStore adapter over [`BroadPhase`].
///
/// It maps each live agent to its body shape and rebuilds the broad phase from
/// the shared, stable-order store, so all candidate queries this tick see every
/// live agent of both modes and no dead slot.
#[derive(Debug, Clone)]
pub(crate) struct SpatialIndex {
    grid: BroadPhase,
    /// Reused body buffer, so a rebuild does not allocate inside the tick loop.
    bodies: Vec<(AgentId, BodyShape)>,
}

impl Default for SpatialIndex {
    fn default() -> Self {
        Self::new(GRID_CELL_SIZE_M)
    }
}

impl SpatialIndex {
    /// A grid with the given cell edge length in metres.
    ///
    /// A non-finite or non-positive edge falls back to [`GRID_CELL_SIZE_M`], so
    /// the index is always well defined.
    pub(crate) fn new(cell_size_m: f64) -> Self {
        Self {
            grid: BroadPhase::new(cell_size_m),
            bodies: Vec::new(),
        }
    }

    /// Rebuild the grid from the live agents of `agents`, in ascending id
    /// order.
    ///
    /// A rebuild is a pure function of agent state, so rebuilding at the start
    /// of a tick gives every query that tick one consistent candidate view.
    pub(crate) fn rebuild(&mut self, agents: &AgentStore) {
        self.bodies.clear();
        for index in 0..agents.len() {
            if !agents.alive[index] {
                continue;
            }
            self.bodies
                .push((AgentId::from_index(index), query::agent_body(agents, index)));
        }
        self.grid.rebuild(&self.bodies);
    }

    /// The live agents whose centre cell overlaps the axis-aligned box
    /// `[min, max]`, in ascending [`AgentId`] order.
    ///
    /// The result replaces `out`; `out` is only cleared and re-filled, so a
    /// caller can reuse one buffer across a tick. The query is a candidate
    /// query, so a caller applies an exact shape test to each returned agent.
    pub(crate) fn candidates_in_aabb(&self, min: DVec2, max: DVec2, out: &mut Vec<AgentId>) {
        self.grid.candidates_in_aabb(Aabb::new(min, max), out);
    }
}

/// Whether a circle body overlaps a closed polygon ring.
///
/// A centre inside the ring overlaps by definition; otherwise the circle
/// overlaps when its radius reaches the nearest edge. Vertices are visited in
/// ring order and the first overlapping edge wins, so the result is
/// deterministic. A ring with fewer than three vertices encloses nothing and
/// never overlaps.
pub(crate) fn circle_overlaps_ring(ring: &[DVec2], centre: DVec2, radius_m: f64) -> bool {
    if ring.len() < 3 {
        return false;
    }
    if point_in_ring(ring, centre) {
        return true;
    }
    for index in 0..ring.len() {
        let start = ring[index];
        let end = ring[(index + 1) % ring.len()];
        if point_segment_distance(centre, start, end) <= radius_m {
            return true;
        }
    }
    false
}

/// Whether a point lies inside a ring, by the even-odd crossing rule.
fn point_in_ring(ring: &[DVec2], point: DVec2) -> bool {
    let mut inside = false;
    let mut previous = ring.len() - 1;
    for index in 0..ring.len() {
        let start = ring[index];
        let end = ring[previous];
        let straddles = (start.y > point.y) != (end.y > point.y);
        if straddles
            && point.x < (end.x - start.x) * (point.y - start.y) / (end.y - start.y) + start.x
        {
            inside = !inside;
        }
        previous = index;
    }
    inside
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::AgentInit;
    use tangle_model::PathId;

    fn agent(mode: crate::agent::AgentMode, at: DVec2) -> AgentInit {
        AgentInit {
            mode,
            path: PathId::from_index(0),
            distance_m: 0.0,
            speed_mps: 0.0,
            position: at,
            heading_rad: 0.0,
            body_length_m: 0.5,
            body_width_m: 0.5,
            direction: 1.0,
            movement: None,
            profile: None,
            narrow_profile: None,
            pedestrian_route: None,
            pedestrian_profile: None,
            route_state: None,
        }
    }

    fn store(positions: &[(crate::agent::AgentMode, DVec2)]) -> AgentStore {
        let mut agents = AgentStore::default();
        for &(mode, at) in positions {
            agents.push(agent(mode, at));
        }
        agents
    }

    use crate::agent::AgentMode;

    #[test]
    fn candidates_cover_both_modes_in_ascending_id_order() {
        // A pedestrian, a vehicle, and a far pedestrian: the grid indexes both
        // modes, so a query returns whichever of them is inside the box.
        let agents = store(&[
            (AgentMode::Pedestrian, DVec2::new(0.0, 0.0)),
            (AgentMode::Vehicle, DVec2::new(1.0, 0.0)),
            (AgentMode::Pedestrian, DVec2::new(100.0, 0.0)),
        ]);
        let mut index = SpatialIndex::default();
        index.rebuild(&agents);
        let mut out = Vec::new();
        index.candidates_in_aabb(DVec2::new(-2.0, -2.0), DVec2::new(2.0, 2.0), &mut out);
        assert_eq!(out, vec![AgentId::from_index(0), AgentId::from_index(1)]);
    }

    #[test]
    fn candidate_order_is_ascending_across_cells() {
        // Ids 2 and 0 land in different cells but both fall inside the box;
        // the result is still ascending, the documented order.
        let agents = store(&[
            (AgentMode::Vehicle, DVec2::new(0.5, 0.5)),
            (AgentMode::Vehicle, DVec2::new(0.6, 0.6)),
            (AgentMode::Vehicle, DVec2::new(20.0, 20.0)),
        ]);
        let mut index = SpatialIndex::default();
        index.rebuild(&agents);
        let mut out = Vec::new();
        index.candidates_in_aabb(DVec2::new(-30.0, -30.0), DVec2::new(30.0, 30.0), &mut out);
        assert_eq!(
            out,
            vec![
                AgentId::from_index(0),
                AgentId::from_index(1),
                AgentId::from_index(2)
            ]
        );
    }

    #[test]
    fn a_query_never_reports_an_agent_twice_and_skips_the_dead() {
        let mut agents = store(&[
            (AgentMode::Pedestrian, DVec2::new(0.0, 0.0)),
            (AgentMode::Pedestrian, DVec2::new(0.1, 0.1)),
        ]);
        agents.alive[0] = false;
        let mut index = SpatialIndex::default();
        index.rebuild(&agents);
        let mut out = Vec::new();
        index.candidates_in_aabb(DVec2::new(-8.0, -8.0), DVec2::new(8.0, 8.0), &mut out);
        assert_eq!(out, vec![AgentId::from_index(1)]);
    }

    #[test]
    fn an_empty_query_clears_the_reused_buffer() {
        let agents = store(&[(AgentMode::Pedestrian, DVec2::new(0.0, 0.0))]);
        let mut index = SpatialIndex::default();
        index.rebuild(&agents);
        let mut out = vec![AgentId::from_index(9)];
        index.candidates_in_aabb(DVec2::new(50.0, 50.0), DVec2::new(60.0, 60.0), &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn a_circle_overlaps_a_ring_inside_on_an_edge_or_clear() {
        let ring = [
            DVec2::new(-1.0, -1.0),
            DVec2::new(1.0, -1.0),
            DVec2::new(1.0, 1.0),
            DVec2::new(-1.0, 1.0),
        ];
        // Inside the ring, the centre alone overlaps.
        assert!(circle_overlaps_ring(&ring, DVec2::ZERO, 0.1));
        // Outside but within a radius of the edge.
        assert!(circle_overlaps_ring(&ring, DVec2::new(1.2, 0.0), 0.3));
        // Outside and clear of the edge by more than the radius.
        assert!(!circle_overlaps_ring(&ring, DVec2::new(1.31, 0.0), 0.3));
        // A degenerate ring encloses nothing.
        assert!(!circle_overlaps_ring(
            &[DVec2::ZERO, DVec2::X],
            DVec2::ZERO,
            5.0
        ));
    }

    #[test]
    fn the_cell_size_falls_back_when_invalid() {
        let zero = BroadPhase::new(0.0);
        assert!(zero.cell_size_m() > 0.0);
        let nan = BroadPhase::new(f64::NAN);
        assert!(nan.cell_size_m() > 0.0);
    }

    fn body(x: f64, y: f64, radius_m: f64) -> BodyShape {
        BodyShape::Circle {
            centre: DVec2::new(x, y),
            radius_m,
        }
    }

    #[test]
    fn candidate_pairs_are_ascending_unique_and_skip_far_bodies() {
        // Bodies 0 and 1 overlap; body 2 is far from both.
        let bodies = vec![
            (AgentId::from_index(2), body(50.0, 0.0, 0.5)),
            (AgentId::from_index(0), body(0.0, 0.0, 0.5)),
            (AgentId::from_index(1), body(0.5, 0.0, 0.5)),
        ];
        let mut phase = BroadPhase::default();
        phase.rebuild(&bodies);
        assert_eq!(phase.len(), 3);
        let mut pairs = Vec::new();
        phase.candidate_pairs(&mut pairs);
        assert_eq!(
            pairs,
            vec![(AgentId::from_index(0), AgentId::from_index(1))]
        );
    }

    #[test]
    fn rebuild_is_independent_of_input_order() {
        let mut forward = BroadPhase::default();
        forward.rebuild(&[
            (AgentId::from_index(0), body(0.0, 0.0, 1.0)),
            (AgentId::from_index(1), body(1.0, 0.0, 1.0)),
            (AgentId::from_index(2), body(2.0, 0.0, 1.0)),
        ]);
        let mut shuffled = BroadPhase::default();
        shuffled.rebuild(&[
            (AgentId::from_index(2), body(2.0, 0.0, 1.0)),
            (AgentId::from_index(0), body(0.0, 0.0, 1.0)),
            (AgentId::from_index(1), body(1.0, 0.0, 1.0)),
        ]);
        let (mut first, mut second) = (Vec::new(), Vec::new());
        forward.candidate_pairs(&mut first);
        shuffled.candidate_pairs(&mut second);
        assert_eq!(first, second);
        assert_eq!(
            first,
            vec![
                (AgentId::from_index(0), AgentId::from_index(1)),
                (AgentId::from_index(0), AgentId::from_index(2)),
                (AgentId::from_index(1), AgentId::from_index(2)),
            ]
        );
    }

    #[test]
    fn candidates_overlapping_returns_a_body_whose_centre_is_outside() {
        // The body is large enough that its extent reaches into the query box
        // even though its centre sits in a cell the tight query does not visit.
        let mut phase = BroadPhase::default();
        phase.rebuild(&[(AgentId::from_index(0), body(5.0, 0.0, 5.0))]);
        assert!((phase.max_half_extent_m() - 5.0).abs() < 1e-12);
        let bounds = Aabb::new(DVec2::new(-1.0, -1.0), DVec2::new(0.0, 1.0));
        let mut out = Vec::new();
        phase.candidates_overlapping(bounds, &mut out);
        assert_eq!(out, vec![AgentId::from_index(0)]);
        // The tight query, by contrast, misses a body whose centre is outside.
        phase.candidates_in_aabb(bounds, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn the_swept_phase_pairs_a_crossing_body_the_static_phase_misses() {
        // Body 0 crosses body 1 entirely: no overlap at either tick endpoint,
        // so an index of the start bounds never pairs them.
        let start_bodies = vec![
            (AgentId::from_index(0), body(0.0, 0.0, 0.5)),
            (AgentId::from_index(1), body(5.0, 0.0, 0.5)),
        ];
        let mut static_phase = BroadPhase::default();
        static_phase.rebuild(&start_bodies);
        let mut pairs = Vec::new();
        static_phase.candidate_pairs(&mut pairs);
        assert!(
            pairs.is_empty(),
            "a start-bound index cannot pair a crossing body"
        );

        let swept = vec![
            (
                AgentId::from_index(0),
                SweptBody {
                    shape: body(0.0, 0.0, 0.5),
                    displacement_m: DVec2::new(10.0, 0.0),
                },
            ),
            (
                AgentId::from_index(1),
                SweptBody {
                    shape: body(5.0, 0.0, 0.5),
                    displacement_m: DVec2::ZERO,
                },
            ),
        ];
        let mut phase = SweptBroadPhase::default();
        phase.rebuild(&swept);
        assert_eq!(phase.len(), 2);
        assert!((phase.max_reach_m() - 10.5).abs() < 1e-12);
        phase.candidate_pairs(&mut pairs);
        assert_eq!(
            pairs,
            vec![(AgentId::from_index(0), AgentId::from_index(1))]
        );
    }

    #[test]
    fn swept_candidates_are_ascending_unique_and_input_order_independent() {
        let forward = vec![
            (
                AgentId::from_index(0),
                SweptBody {
                    shape: body(0.0, 0.0, 0.5),
                    displacement_m: DVec2::new(1.0, 0.0),
                },
            ),
            (
                AgentId::from_index(1),
                SweptBody {
                    shape: body(1.0, 0.0, 0.5),
                    displacement_m: DVec2::ZERO,
                },
            ),
            (
                AgentId::from_index(2),
                SweptBody {
                    shape: body(50.0, 0.0, 0.5),
                    displacement_m: DVec2::ZERO,
                },
            ),
        ];
        let mut shuffled = forward.clone();
        shuffled.reverse();
        let mut first_phase = SweptBroadPhase::default();
        first_phase.rebuild(&forward);
        let mut second_phase = SweptBroadPhase::default();
        second_phase.rebuild(&shuffled);

        let mut first_pairs = Vec::new();
        let mut second_pairs = Vec::new();
        first_phase.candidate_pairs(&mut first_pairs);
        second_phase.candidate_pairs(&mut second_pairs);
        assert_eq!(first_pairs, second_pairs);
        assert_eq!(
            first_pairs,
            vec![(AgentId::from_index(0), AgentId::from_index(1))]
        );

        let query = Aabb::new(DVec2::new(-1.0, -1.0), DVec2::new(2.0, 1.0));
        let mut candidates = vec![AgentId::from_index(9)];
        first_phase.candidates_overlapping(query, &mut candidates);
        assert_eq!(
            candidates,
            vec![AgentId::from_index(0), AgentId::from_index(1)]
        );
        assert_eq!(
            first_phase.swept_bounds_of(AgentId::from_index(2)),
            Some(body(50.0, 0.0, 0.5).bounds())
        );
        assert_eq!(first_phase.swept_bounds_of(AgentId::from_index(9)), None);
    }

    #[test]
    fn a_swept_candidate_covers_a_body_whose_centre_is_outside_the_query() {
        let query = Aabb::new(DVec2::new(-1.0, -1.0), DVec2::new(1.0, 1.0));
        let crossing = SweptBody {
            shape: body(-6.0, 0.0, 0.5),
            displacement_m: DVec2::new(12.0, 0.0),
        };
        let mut phase = SweptBroadPhase::default();
        phase.rebuild(&[(AgentId::from_index(0), crossing)]);
        let mut out = Vec::new();
        phase.candidates_overlapping(query, &mut out);
        assert_eq!(out, vec![AgentId::from_index(0)]);

        // The same start centre without the crossing motion is not a candidate.
        let still = SweptBody {
            shape: body(-6.0, 0.0, 0.5),
            displacement_m: DVec2::ZERO,
        };
        let mut still_phase = SweptBroadPhase::default();
        still_phase.rebuild(&[(AgentId::from_index(0), still)]);
        still_phase.candidates_overlapping(query, &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn the_body_shape_matches_the_agent_mode() {
        let agents = store(&[
            (AgentMode::Pedestrian, DVec2::new(1.0, 2.0)),
            (AgentMode::Vehicle, DVec2::new(3.0, 4.0)),
        ]);
        assert_eq!(
            query::agent_body(&agents, 0),
            BodyShape::Circle {
                centre: DVec2::new(1.0, 2.0),
                radius_m: 0.25,
            }
        );
        assert_eq!(
            query::agent_body(&agents, 1),
            BodyShape::Box {
                centre: DVec2::new(3.0, 4.0),
                heading_rad: 0.0,
                length_m: 0.5,
                width_m: 0.5,
            }
        );
    }
}
