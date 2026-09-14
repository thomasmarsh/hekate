//! Named, portable random streams derived from the run root seed.
//!
//! `PHASE_1_PLAN.md` requires a portable ChaCha generator with independent
//! named streams (`demand`, `profile`, `compliance`, `perception`) derived from
//! the root seed and stable agent/run identifiers. Concerns must never share
//! one mutable generator, so adding a draw to one stream cannot reshuffle
//! another.
//!
//! This module owns the derivation for the streams drawn from so far: one
//! `demand` substream per vehicle demand source, one `pedestrian_demand`
//! substream per pedestrian demand source, one `profile` substream per agent
//! of either mode, one `compliance` substream per agent, and one `maneuver`
//! substream per agent for the contextual wrong-way draw. The `perception`
//! stream belongs to a later increment and is not derived here.

use rand_chacha::ChaCha20Rng;
use rand_chacha::rand_core::{Rng, SeedableRng};

/// Stream name for portal demand generation and route assignment.
pub const STREAM_DEMAND: &str = "demand";

/// Stream name for pedestrian demand generation and route assignment.
///
/// Pedestrian demand draws from its own named stream rather than sharing
/// `demand`, so adding or removing a pedestrian source cannot reshuffle a
/// vehicle source's arrival or route sequence.
pub const STREAM_PEDESTRIAN_DEMAND: &str = "pedestrian_demand";

/// Stream name for per-agent physical and behavior profile sampling.
///
/// The stream is mode-neutral: a vehicle and a pedestrian derive it from the
/// same stable agent id, and agent ids are unique across modes, so each agent
/// has its own substream regardless of body kind.
pub const STREAM_PROFILE: &str = "profile";

/// Stream name for per-agent signal-compliance propensity sampling.
///
/// The red-light decision itself is a deterministic function of its context
/// (signal color, stop-line gap, speed, and the sampled propensity), so this
/// stream carries the stable per-agent draw rather than a fresh coin flip each
/// decision. See [`crate::compliance`].
pub const STREAM_COMPLIANCE: &str = "compliance";

/// Stream name for the contextual wrong-way `maneuver` draw.
///
/// The draw is keyed by the root seed, the stable agent id, and the agent's own
/// decision ordinal, so one agent's value never depends on how many other
/// agents drew or in what order the decisions were evaluated. See
/// [`crate::wrong_way::maneuver_draw`].
pub const STREAM_MANEUVER: &str = "maneuver";

/// FNV-1a 64-bit offset basis.
const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
/// FNV-1a 64-bit prime.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
/// SplitMix64 golden-gamma increment.
const SPLITMIX_GAMMA: u64 = 0x9e37_79b9_7f4a_7c15;

/// Derive a named ChaCha stream from the root seed and a stable identifier.
///
/// The identifier is a demand-source index or an agent id, so each stream is
/// independent and stable: the same `(root_seed, name, id)` always yields the
/// same sequence on every run and platform. This is a reproducibility
/// derivation, not a cryptographic one; the generated values need only be
/// deterministic and well mixed.
pub(crate) fn derive_stream(root_seed: u64, name: &str, id: u32) -> ChaCha20Rng {
    ChaCha20Rng::seed_from_u64(derive_seed(root_seed, name, id))
}

/// The 64-bit seed a named stream is seeded from.
fn derive_seed(root_seed: u64, name: &str, id: u32) -> u64 {
    let name_hash = fnv1a(name);
    // Mix the name hash with the root seed and the stable identifier present
    // that the resulting seed depends on all three inputs.
    let mixed = name_hash ^ root_seed ^ u64::from(id).wrapping_mul(SPLITMIX_GAMMA);
    splitmix64(mixed)
}

/// FNV-1a hash of a stream name.
fn fnv1a(name: &str) -> u64 {
    let mut hash = FNV_OFFSET;
    for byte in name.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// SplitMix64 finalizer, used to decorrelate the mixed seed bits.
fn splitmix64(state: u64) -> u64 {
    let mut z = state.wrapping_add(SPLITMIX_GAMMA);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Draw a uniform `f64` in `[0, 1)` from a stream.
///
/// Uses the top 53 bits so every representable value has the same spacing.
pub(crate) fn uniform01(rng: &mut ChaCha20Rng) -> f64 {
    const SCALE: f64 = 1.0 / (1u64 << 53) as f64;
    (rng.next_u64() >> 11) as f64 * SCALE
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_inputs_reproduce_a_stream() {
        let mut first = derive_stream(42, STREAM_DEMAND, 0);
        let mut second = derive_stream(42, STREAM_DEMAND, 0);
        for _ in 0..16 {
            assert_eq!(first.next_u64(), second.next_u64());
        }
    }

    #[test]
    fn named_streams_and_identifiers_are_independent() {
        fn draw(root: u64, name: &str, id: u32) -> u64 {
            let mut rng = derive_stream(root, name, id);
            rng.next_u64()
        }
        assert_ne!(draw(42, STREAM_DEMAND, 0), draw(42, STREAM_PROFILE, 0));
        assert_ne!(draw(42, STREAM_DEMAND, 0), draw(42, STREAM_DEMAND, 1));
        assert_ne!(draw(42, STREAM_DEMAND, 0), draw(43, STREAM_DEMAND, 0));
        assert_ne!(draw(42, STREAM_COMPLIANCE, 0), draw(42, STREAM_PROFILE, 0));
    }

    #[test]
    fn maneuver_stream_is_independent_of_the_other_named_streams() {
        fn draw(root: u64, name: &str, id: u32) -> u64 {
            let mut rng = derive_stream(root, name, id);
            rng.next_u64()
        }
        let maneuver = draw(42, STREAM_MANEUVER, 0);
        for name in [
            STREAM_DEMAND,
            STREAM_PEDESTRIAN_DEMAND,
            STREAM_PROFILE,
            STREAM_COMPLIANCE,
        ] {
            assert_ne!(maneuver, draw(42, name, 0), "{name}");
        }
    }

    #[test]
    fn pedestrian_streams_are_independent_of_vehicle_streams() {
        fn draw(root: u64, name: &str, id: u32) -> u64 {
            let mut rng = derive_stream(root, name, id);
            rng.next_u64()
        }
        assert_ne!(
            draw(42, STREAM_PEDESTRIAN_DEMAND, 0),
            draw(42, STREAM_DEMAND, 0)
        );
        assert_ne!(
            draw(42, STREAM_PEDESTRIAN_DEMAND, 0),
            draw(42, STREAM_PROFILE, 0)
        );
        assert_ne!(
            draw(42, STREAM_PEDESTRIAN_DEMAND, 0),
            draw(42, STREAM_PEDESTRIAN_DEMAND, 1)
        );
    }

    #[test]
    fn extra_pedestrian_draws_do_not_perturb_vehicle_streams() {
        // The same isolation contract as the compliance stream: a new draw on
        // the pedestrian demand stream leaves every vehicle demand, profile,
        // and compliance sequence byte-identical.
        fn vehicle_streams(root: u64, id: u32, pedestrian_draws: usize) -> Vec<Vec<u64>> {
            let mut pedestrian = derive_stream(root, STREAM_PEDESTRIAN_DEMAND, id);
            for _ in 0..pedestrian_draws {
                let _ = uniform01(&mut pedestrian);
            }
            [STREAM_DEMAND, STREAM_PROFILE, STREAM_COMPLIANCE]
                .into_iter()
                .map(|name| {
                    let mut rng = derive_stream(root, name, id);
                    (0..8).map(|_| rng.next_u64()).collect()
                })
                .collect()
        }

        assert_eq!(vehicle_streams(23, 2, 0), vehicle_streams(23, 2, 7));
    }

    #[test]
    fn pedestrian_compliance_draws_do_not_perturb_other_streams() {
        // A pedestrian draws its crossing-compliance propensity from the
        // `compliance` stream under its own stable agent id. The vehicle
        // `demand`, `profile`, and `compliance` sequences under a different
        // agent id are therefore byte-identical with and without pedestrian
        // compliance draws.
        fn vehicle_streams(
            root: u64,
            vehicle_id: u32,
            pedestrian_id: u32,
            pedestrian_draws: usize,
        ) -> (Vec<u64>, Vec<u64>, Vec<u64>) {
            let mut pedestrian = derive_stream(root, STREAM_COMPLIANCE, pedestrian_id);
            for _ in 0..pedestrian_draws {
                let _ = uniform01(&mut pedestrian);
            }

            let sequence = |name: &str| {
                let mut rng = derive_stream(root, name, vehicle_id);
                (0..8).map(|_| rng.next_u64()).collect()
            };
            (
                sequence(STREAM_DEMAND),
                sequence(STREAM_PROFILE),
                sequence(STREAM_COMPLIANCE),
            )
        }

        assert_eq!(vehicle_streams(31, 1, 2, 0), vehicle_streams(31, 1, 2, 4));
    }

    #[test]
    fn an_extra_compliance_draw_does_not_perturb_demand_or_profile() {
        // The stream-isolation contract: drawing from one named stream cannot
        // reshuffle another, so an added compliance draw leaves the demand and
        // profile sequences byte-identical.
        fn demand_and_profile(root: u64, id: u32, compliance_draws: usize) -> (Vec<u64>, Vec<u64>) {
            let mut demand = derive_stream(root, STREAM_DEMAND, id);
            let demand_values = (0..8).map(|_| demand.next_u64()).collect();

            let mut compliance = derive_stream(root, STREAM_COMPLIANCE, id);
            for _ in 0..compliance_draws {
                let _ = uniform01(&mut compliance);
            }

            let mut profile = derive_stream(root, STREAM_PROFILE, id);
            let profile_values = (0..8).map(|_| profile.next_u64()).collect();
            (demand_values, profile_values)
        }

        assert_eq!(demand_and_profile(17, 3, 0), demand_and_profile(17, 3, 5));
    }

    #[test]
    fn uniform_values_stay_in_the_unit_interval() {
        let mut rng = derive_stream(7, STREAM_PROFILE, 3);
        for _ in 0..1000 {
            let value = uniform01(&mut rng);
            assert!((0.0..1.0).contains(&value));
        }
    }
}
