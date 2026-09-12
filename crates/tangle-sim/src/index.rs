//! Deterministic shared spatial index over every live agent, plus the spatial
//! predicates a candidate query feeds.
//!
//! # Model card
//!
//! Phase 1 Increment 3 slice D needs one deterministic way to ask "which bodies
//! are near this region?" that both modes share, so vehicle yielding to an
//! occupied crossing is not a mode-specific shortcut. This module is that
//! foundation: a uniform grid rebuilt each tick from the shared, stable-order
//! [`AgentStore`], and a bounding-box candidate query over it.
//!
//! `PHASE_1_PLAN.md` Increment 4 ("Deterministic uniform-grid broad phase with
//! stable candidate ordering") formalises this into the broad phase and adds
//! exact box/box, circle/circle, and box/circle queries plus swept bounds. This
//! module is deliberately only the foundation: a candidate query with a stable
//! order, used by the crossing-occupancy test, not a speculative full broad
//! phase.
//!
//! ## Determinism
//!
//! The grid stores its cells in a `Vec` sorted by integer cell coordinate and
//! binary-searches that array on a query. Nothing here iterates a hash map, so
//! the candidate set and its order are a pure function of agent state. Agents
//! are inserted in ascending [`AgentId`] order and a query sorts its result
//! ascending, so equal cells and exactly equal positions are broken by
//! [`AgentId`]. Every agent occupies exactly one cell, so a query never reports
//! the same agent twice.
//!
//! ## Query shape
//!
//! [`SpatialIndex::candidates_in_aabb`] returns every live agent whose cell
//! overlaps an axis-aligned box, in ascending [`AgentId`] order. It is a
//! candidate query: a caller applies its own exact shape test, such as
//! [`circle_overlaps_ring`], to each candidate. The query widens the box by a
//! caller-chosen margin so a body whose centre is outside the box can still be
//! returned when its extent reaches in.

use glam::DVec2;

use crate::agent::{AgentId, AgentStore};

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

/// A deterministic uniform-grid candidate index over live agents of both modes.
#[derive(Debug, Clone)]
pub(crate) struct SpatialIndex {
    cell_size_m: f64,
    /// Cells in ascending `Cell` order; each holds the agents whose centre
    /// falls in it, in ascending [`AgentId`] order.
    cells: Vec<(Cell, Vec<AgentId>)>,
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
        let cell_size_m = if cell_size_m.is_finite() && cell_size_m > 0.0 {
            cell_size_m
        } else {
            GRID_CELL_SIZE_M
        };
        Self {
            cell_size_m,
            cells: Vec::new(),
        }
    }

    /// Rebuild the grid from the live agents of `agents`, in ascending id
    /// order.
    ///
    /// A rebuild is a pure function of agent state, so rebuilding at the start
    /// of a tick gives every query that tick one consistent candidate view.
    pub(crate) fn rebuild(&mut self, agents: &AgentStore) {
        let mut entries: Vec<(Cell, AgentId)> = Vec::with_capacity(agents.len());
        for index in 0..agents.len() {
            if !agents.alive[index] {
                continue;
            }
            entries.push((
                self.cell_of(agents.position[index]),
                AgentId::from_index(index),
            ));
        }
        // A stable sort by cell preserves the ascending id order within a cell,
        // so a cell's residents are already in the documented order.
        entries.sort_by_key(|(cell, _)| *cell);
        self.cells.clear();
        for (cell, id) in entries {
            match self.cells.last_mut() {
                Some((last, ids)) if *last == cell => ids.push(id),
                _ => self.cells.push((cell, vec![id])),
            }
        }
    }

    /// The agents whose cell overlaps the axis-aligned box `[min, max]`, in
    /// ascending [`AgentId`] order.
    ///
    /// The result replaces `out`; `out` is only cleared and re-filled, so a
    /// caller can reuse one buffer across a tick. The query is a candidate
    /// query, so a caller applies an exact shape test to each returned agent.
    pub(crate) fn candidates_in_aabb(&self, min: DVec2, max: DVec2, out: &mut Vec<AgentId>) {
        out.clear();
        let low = self.cell_of(min);
        let high = self.cell_of(max);
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

    /// The cell containing a world position.
    fn cell_of(&self, position: DVec2) -> Cell {
        Cell {
            x: (position.x / self.cell_size_m).floor() as i32,
            y: (position.y / self.cell_size_m).floor() as i32,
        }
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
            pedestrian_route: None,
            pedestrian_profile: None,
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
        let zero = SpatialIndex::new(0.0);
        assert!(zero.cell_size_m > 0.0);
        let nan = SpatialIndex::new(f64::NAN);
        assert!(nan.cell_size_m > 0.0);
    }
}
