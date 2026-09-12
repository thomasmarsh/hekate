//! Per-agent physical and behavior profile sampling.
//!
//! A vehicle samples one stable profile when it is admitted. Its physical and
//! longitudinal parameters come from its own `profile` substream, while its
//! signal-compliance propensity comes from its own `compliance` substream (see
//! [`crate::rng`]), so the two concerns never share a mutable generator.

use rand_chacha::ChaCha20Rng;
use tangle_model::CompiledProfile;

use crate::rng::uniform01;

/// A stable physical and behavior profile for one vehicle.
///
/// Values are drawn once and never resampled, so a vehicle's body and
/// longitudinal behavior do not change over a run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VehicleProfile {
    /// Desired free-flow speed in metres per second.
    pub desired_speed_mps: f64,
    /// Body length in metres.
    pub length_m: f64,
    /// Body width in metres.
    pub width_m: f64,
    /// Desired following time gap in seconds.
    pub time_gap_s: f64,
    /// Maximum acceleration in metres per second squared.
    pub max_accel_mps2: f64,
    /// Comfortable deceleration in metres per second squared.
    pub comfortable_brake_mps2: f64,
    /// Signal-compliance propensity in `[0, 1]`, drawn from the `compliance`
    /// stream. See [`crate::compliance`] for how the red-light decision uses
    /// it.
    pub compliance: f64,
}

/// Draw one profile from `profile_rng` and its compliance propensity from
/// `compliance_rng`, each sampled in a fixed order.
///
/// The physical/longitudinal draw order is part of the reproducibility
/// contract: reordering those calls changes every sampled profile for the same
/// seed. Compliance is drawn from a separate stream so it can be resampled
/// without disturbing the rest of the profile.
pub(crate) fn sample_profile(
    profile: &CompiledProfile,
    profile_rng: &mut ChaCha20Rng,
    compliance_rng: &mut ChaCha20Rng,
) -> VehicleProfile {
    VehicleProfile {
        desired_speed_mps: profile.speed_mps().sample(uniform01(profile_rng)),
        length_m: profile.length_m().sample(uniform01(profile_rng)),
        width_m: profile.width_m().sample(uniform01(profile_rng)),
        time_gap_s: profile.time_gap_s().sample(uniform01(profile_rng)),
        max_accel_mps2: profile.max_accel_mps2().sample(uniform01(profile_rng)),
        comfortable_brake_mps2: profile
            .comfortable_brake_mps2()
            .sample(uniform01(profile_rng)),
        compliance: profile.compliance().sample(uniform01(compliance_rng)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::{STREAM_COMPLIANCE, STREAM_PROFILE, derive_stream};
    use tangle_model::{CompiledScenario, parse_scenario_source};

    const FLOW: &str = r#"
    {
      schema_version: 1,
      id: 'flow',
      coordinate_system: { x: 'east_m', y: 'north_m' },
      paths: [ { id: 'guide', points: [ { x: 0, y: 0 }, { x: 100, y: 0 } ] } ],
      portals: [
        { id: 'entry', path: 'guide', end: 'start', width_m: 3.5 },
        { id: 'exit', path: 'guide', end: 'end', width_m: 3.5 },
      ],
      movements: [
        { id: 'through', from: 'entry', to: 'exit', path: 'guide', priority: 0 },
      ],
      demand: [ { id: 'inflow', portal: 'entry', rate_vph: 600.0,
        routes: [ { movement: 'through', weight: 1.0 } ] } ],
      profiles: {
        speed_mps: { min: 10.0, max: 10.0 },
        length_m: { min: 4.0, max: 4.0 },
        width_m: { min: 2.0, max: 2.0 },
        time_gap_s: { min: 1.5, max: 1.5 },
        max_accel_mps2: { min: 2.0, max: 2.0 },
        comfortable_brake_mps2: { min: 3.0, max: 3.0 },
      },
    }
    "#;

    fn scenario() -> CompiledScenario {
        let source = parse_scenario_source(FLOW).expect("parses");
        CompiledScenario::compile(source).expect("compiles")
    }

    #[test]
    fn a_degenerate_range_samples_the_constant() {
        let profile = sample_profile(
            scenario().profiles(),
            &mut derive_stream(1, STREAM_PROFILE, 0),
            &mut derive_stream(1, STREAM_COMPLIANCE, 0),
        );
        assert!((profile.desired_speed_mps - 10.0).abs() < 1e-9);
        assert!((profile.length_m - 4.0).abs() < 1e-9);
        assert!((profile.time_gap_s - 1.5).abs() < 1e-9);
        assert!((profile.comfortable_brake_mps2 - 3.0).abs() < 1e-9);
        // A profile that omits `compliance` defaults to fully compliant.
        assert!((profile.compliance - 1.0).abs() < 1e-9);
    }

    #[test]
    fn one_agents_profile_is_stable_across_resampling() {
        let scenario = scenario();
        let first = sample_profile(
            scenario.profiles(),
            &mut derive_stream(9, STREAM_PROFILE, 4),
            &mut derive_stream(9, STREAM_COMPLIANCE, 4),
        );
        let second = sample_profile(
            scenario.profiles(),
            &mut derive_stream(9, STREAM_PROFILE, 4),
            &mut derive_stream(9, STREAM_COMPLIANCE, 4),
        );
        assert_eq!(first, second);
    }

    #[test]
    fn sampled_values_stay_within_the_configured_ranges() {
        let scenario = scenario();
        let mut rng = derive_stream(3, STREAM_PROFILE, 0);
        // Same-stream sampling lands inside the configured envelope.
        for _ in 0..100 {
            let sample = scenario.profiles().speed_mps().sample(uniform01(&mut rng));
            assert!((10.0..=10.0).contains(&sample));
        }
    }
}
