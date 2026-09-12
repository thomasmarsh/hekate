//! Portal demand generation, route assignment, and the pending spawn queue.
//!
//! Each compiled demand source owns one `demand` substream and a bounded FIFO
//! queue of arrivals waiting for safe spawn admission. Arrivals and route
//! choices are drawn when an arrival is generated; the profile is sampled from
//! the `profile` stream at admission, once the stable agent id is known.

use std::collections::VecDeque;

use rand_chacha::ChaCha20Rng;
use tangle_model::{CompiledDemand, MovementId};

use crate::rng::uniform01;

/// Most pending arrivals one demand source holds before dropping new ones.
///
/// A bounded queue is the safe-admission backstop: saturated demand fills it
/// and then sheds load deterministically instead of growing without limit.
pub(crate) const MAX_PENDING_SPAWNS: usize = 64;

/// Runtime demand state for one compiled demand source.
#[derive(Debug)]
pub(crate) struct DemandRuntime {
    /// Index of the compiled demand source this runtime serves.
    pub(crate) source: usize,
    /// The owning source's `demand` substream.
    pub(crate) rng: ChaCha20Rng,
    /// Arrivals awaiting safe admission, oldest first.
    pub(crate) pending: VecDeque<MovementId>,
    /// Arrivals shed because the pending queue was full.
    pub(crate) dropped: u64,
}

impl DemandRuntime {
    /// Create a runtime for one compiled demand source.
    pub(crate) fn new(source: usize, rng: ChaCha20Rng) -> Self {
        Self {
            source,
            rng,
            pending: VecDeque::new(),
            dropped: 0,
        }
    }

    /// Vehicles waiting for safe admission.
    pub(crate) fn pending_len(&self) -> usize {
        self.pending.len()
    }
}

/// Choose one route for an arrival by relative weight.
///
/// Validation guarantees at least one positive-weight route, so the fallback
/// only covers floating-point rounding at the end of the cumulative sweep.
pub(crate) fn sample_route(demand: &CompiledDemand, rng: &mut ChaCha20Rng) -> MovementId {
    let total: f64 = demand.routes().iter().map(|route| route.weight()).sum();
    let mut pick = uniform01(rng) * total;
    for route in demand.routes() {
        if pick < route.weight() {
            return route.movement();
        }
        pick -= route.weight();
    }
    demand
        .routes()
        .last()
        .expect("validated demand has at least one route")
        .movement()
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
