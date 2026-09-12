//! Portal demand generation, route assignment, and the pending spawn queue.
//!
//! Each compiled demand source of either mode owns one named substream and a
//! bounded FIFO queue of arrivals waiting for safe spawn admission. Arrivals
//! and route choices are drawn when an arrival is generated; the profile is
//! sampled from the `profile` stream at admission, once the stable agent id is
//! known. Vehicle sources draw from `demand` and pedestrian sources from
//! `pedestrian_demand` (see [`crate::rng`]), so the two modes never share a
//! generator.

use std::collections::VecDeque;

use rand_chacha::ChaCha20Rng;
use tangle_model::{CompiledDemand, CompiledPedestrianDemand, MovementId, PedestrianRouteId};

use crate::rng::uniform01;

/// Most pending arrivals one demand source holds before dropping new ones.
///
/// A bounded queue is the safe-admission backstop: saturated demand fills it
/// and then sheds load deterministically instead of growing without limit.
pub(crate) const MAX_PENDING_SPAWNS: usize = 64;

/// Runtime demand state for one compiled demand source of either mode.
///
/// `T` is the dense route identifier the source's arrivals follow: a
/// [`MovementId`] for vehicles and a [`PedestrianRouteId`] for pedestrians.
#[derive(Debug)]
pub(crate) struct DemandRuntime<T> {
    /// Index of the compiled demand source this runtime serves.
    pub(crate) source: usize,
    /// The owning source's named substream.
    pub(crate) rng: ChaCha20Rng,
    /// Arrivals awaiting safe admission, oldest first.
    pub(crate) pending: VecDeque<T>,
    /// Arrivals shed because the pending queue was full.
    pub(crate) dropped: u64,
}

impl<T> DemandRuntime<T> {
    /// Create a runtime for one compiled demand source.
    pub(crate) fn new(source: usize, rng: ChaCha20Rng) -> Self {
        Self {
            source,
            rng,
            pending: VecDeque::new(),
            dropped: 0,
        }
    }

    /// Arrivals waiting for safe admission.
    pub(crate) fn pending_len(&self) -> usize {
        self.pending.len()
    }
}

/// Index of the weighted choice a single `[0, 1)` draw selects.
///
/// Validation guarantees at least one positive-weight route, so the fallback
/// only covers floating-point rounding at the end of the cumulative sweep.
fn pick_weighted(weights: &[f64], rng: &mut ChaCha20Rng) -> usize {
    let total: f64 = weights.iter().sum();
    let mut pick = uniform01(rng) * total;
    for (index, &weight) in weights.iter().enumerate() {
        if pick < weight {
            return index;
        }
        pick -= weight;
    }
    weights.len().saturating_sub(1)
}

/// Choose one route for a vehicle arrival by relative weight.
pub(crate) fn sample_route(demand: &CompiledDemand, rng: &mut ChaCha20Rng) -> MovementId {
    let weights: Vec<f64> = demand.routes().iter().map(|route| route.weight()).collect();
    let index = pick_weighted(&weights, rng);
    demand
        .routes()
        .get(index)
        .expect("validated demand has at least one route")
        .movement()
}

/// Choose one route for a pedestrian arrival by relative weight.
pub(crate) fn sample_pedestrian_route(
    demand: &CompiledPedestrianDemand,
    rng: &mut ChaCha20Rng,
) -> PedestrianRouteId {
    let weights: Vec<f64> = demand.routes().iter().map(|share| share.weight()).collect();
    let index = pick_weighted(&weights, rng);
    demand
        .routes()
        .get(index)
        .expect("validated pedestrian demand has at least one route")
        .route()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::{STREAM_DEMAND, derive_stream};
    use tangle_model::{CompiledScenario, parse_scenario_source};

    fn scenario() -> CompiledScenario {
        let source = parse_scenario_source(
            r#"
            {
              schema_version: 1,
              id: 'one_route',
              coordinate_system: { x: 'east_m', y: 'north_m' },
              paths: [
                { id: 'guide', points: [ { x: 0, y: 0 }, { x: 100, y: 0 } ] },
              ],
              portals: [
                { id: 'entry', path: 'guide', end: 'start', width_m: 3.5 },
                { id: 'exit', path: 'guide', end: 'end', width_m: 3.5 },
              ],
              movements: [
                { id: 'through', from: 'entry', to: 'exit', path: 'guide', priority: 0 },
              ],
              demand: [ { id: 'inflow', portal: 'entry', rate_vph: 600.0,
                routes: [ { movement: 'through', weight: 1.0 } ] } ],
            }
            "#,
        )
        .expect("parses");
        CompiledScenario::compile(source).expect("compiles")
    }

    #[test]
    fn a_single_route_is_always_selected() {
        let scenario = scenario();
        let demand = &scenario.demand()[0];
        let mut rng = derive_stream(0, STREAM_DEMAND, 0);
        for _ in 0..50 {
            assert_eq!(
                sample_route(demand, &mut rng),
                demand.routes()[0].movement()
            );
        }
    }

    #[test]
    fn a_pedestrian_route_is_selected_by_weight() {
        let source = parse_scenario_source(
            r#"
            {
              schema_version: 1,
              id: 'pedestrian_routes',
              coordinate_system: { x: 'east_m', y: 'north_m' },
              paths: [
                { id: 'west_walk', points: [ { x: 0, y: 0 }, { x: 10, y: 0 } ] },
                { id: 'east_walk', points: [ { x: 0, y: 5 }, { x: 10, y: 5 } ] },
              ],
              portals: [
                { id: 'south', path: 'west_walk', end: 'start', width_m: 2.0 },
                { id: 'north', path: 'west_walk', end: 'end', width_m: 2.0 },
                { id: 'east_south', path: 'east_walk', end: 'start', width_m: 2.0 },
                { id: 'east_north', path: 'east_walk', end: 'end', width_m: 2.0 },
              ],
              pedestrian_routes: [
                { id: 'west_route', from: 'south', to: 'north', path: 'west_walk' },
                { id: 'east_route', from: 'east_south', to: 'east_north', path: 'east_walk' },
              ],
              pedestrian_demand: [ { id: 'footfall', portal: 'south', rate_pph: 200.0,
                routes: [ { route: 'west_route', weight: 1.0 } ] } ],
            }
            "#,
        )
        .expect("parses");
        let scenario = CompiledScenario::compile(source).expect("compiles");
        let demand = &scenario.pedestrian_demand()[0];
        let mut rng = derive_stream(0, crate::rng::STREAM_PEDESTRIAN_DEMAND, 0);
        for _ in 0..50 {
            assert_eq!(
                sample_pedestrian_route(demand, &mut rng),
                demand.routes()[0].route()
            );
        }
    }

    #[test]
    fn route_selection_is_deterministic_for_a_seed() {
        let scenario = scenario();
        let demand = &scenario.demand()[0];
        let mut first = derive_stream(5, STREAM_DEMAND, 0);
        let mut second = derive_stream(5, STREAM_DEMAND, 0);
        for _ in 0..20 {
            assert_eq!(
                sample_route(demand, &mut first),
                sample_route(demand, &mut second)
            );
        }
    }
}
