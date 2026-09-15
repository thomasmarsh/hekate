//! Per-agent physical and behavior profile sampling.
//!
//! A vehicle samples one stable profile when it is admitted. Its physical and
//! longitudinal parameters come from its own `profile` substream, while its
//! signal-compliance propensity comes from its own `compliance` substream (see
//! [`crate::rng`]), so the two concerns never share a mutable generator. A
//! pedestrian samples its body and gait from the same mode-neutral `profile`
//! stream and its crossing-compliance propensity from the `compliance` stream,
//! both under its own stable agent id.

use hekate_model::{
    AgentBody, CompiledModeTemplate, CompiledPedestrianProfile, CompiledProfile, ProfileRange,
    ProfileRangeSource, ProfileSource,
};
use rand_chacha::ChaCha20Rng;

use crate::articulated::ArticulatedSegmentGeometry;
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

/// The sampled bounded-steering limits of one wheeled agent.
///
/// A wheeled mode's lateral-maneuver limits — its maximum heading rate, its
/// maximum lateral acceleration, and its preferred lateral clearance from a
/// facility edge — live in its compiled mode template, not in the scenario's
/// passenger-car distribution its longitudinal values come from. A capsule
/// samples them into its [`crate::NarrowProfile`]; a box samples them here from
/// its own compiled template, so both families seed one [`crate::BoundedSteering`]
/// envelope from the same shape. A mode whose compiled profile declares no
/// lateral-acceleration limit has no such limits and stays longitudinal-only.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WheeledLateralLimits {
    /// Maximum steering/heading rate in radians per second.
    pub(crate) heading_rate_max_rad_s: f64,
    /// Maximum lateral acceleration in metres per second squared.
    pub(crate) lateral_accel_max_mps2: f64,
    /// Preferred lateral clearance from a facility edge in metres.
    pub(crate) lateral_clearance_m: f64,
}

/// Draw one wheeled mode's lateral limits from its compiled template, or `None`
/// when the template declares no lateral-acceleration limit.
///
/// The three values come from `profile_rng` in a fixed order — lateral
/// acceleration first (which decides whether there is a bounded-steering
/// envelope at all), then the heading rate, then the lateral clearance — so a
/// mode's limits are stable for the run. A template without a
/// `lateral_accel_max_mps2` parameter draws nothing and returns `None`, exactly
/// as a mode with no free lateral motion.
pub(crate) fn sample_wheeled_lateral_limits(
    template: &CompiledModeTemplate,
    profile_rng: &mut ChaCha20Rng,
) -> Option<WheeledLateralLimits> {
    let profile = template.profile();
    let lateral_accel_max_mps2 = profile
        .lateral_accel_max_mps2()?
        .sample(uniform01(profile_rng));
    Some(WheeledLateralLimits {
        heading_rate_max_rad_s: profile
            .steering_rate_max_rad_s()
            .map_or(0.0, |range| range.sample(uniform01(profile_rng))),
        lateral_accel_max_mps2,
        lateral_clearance_m: profile
            .lateral_clearance_m()
            .map_or(0.0, |range| range.sample(uniform01(profile_rng))),
    })
}

/// A stable physical and behavioral profile for one pedestrian.
///
/// A pedestrian body is a circle in Phase 1, so the physical parameter is a
/// radius. Values are drawn once and never resampled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PedestrianProfile {
    /// Body radius in metres.
    pub radius_m: f64,
    /// Desired walking speed in metres per second.
    pub desired_speed_mps: f64,
    /// Crossing-compliance propensity in `[0, 1]`, drawn from the `compliance`
    /// stream. See [`crate::pedestrian_compliance`] for how the crossing
    /// decision uses it.
    pub compliance: f64,
}

/// Draw one pedestrian profile from its own `profile` substream and its
/// compliance propensity from its own `compliance` substream.
///
/// The radius and gait draws come from the `profile` stream in a fixed order, so
/// a pedestrian's body and gait are stable for the run; compliance comes from
/// the separate `compliance` stream so it can be resampled without disturbing
/// the physical draws. Both streams are keyed by the stable agent id, which is
/// unique across modes, so no two agents share a generator.
pub(crate) fn sample_pedestrian_profile(
    profile: &CompiledPedestrianProfile,
    profile_rng: &mut ChaCha20Rng,
    compliance_rng: &mut ChaCha20Rng,
) -> PedestrianProfile {
    PedestrianProfile {
        radius_m: profile.radius_m().sample(uniform01(profile_rng)),
        desired_speed_mps: profile.speed_mps().sample(uniform01(profile_rng)),
        compliance: profile.compliance().sample(uniform01(compliance_rng)),
    }
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

/// Draw one wheeled box mode's body and dynamics from its own compiled
/// template, in [`sample_profile`]'s draw order and from the same two streams.
///
/// A wheeled box mode authors its own body and longitudinal envelope, so an
/// admitted bus or rigid truck samples its own dimensions, desired speed,
/// following gap, acceleration, and braking rather than the passenger car's.
/// The six `profile_rng` draws are exactly [`sample_profile`]'s — desired speed,
/// length, width, time gap, maximum acceleration, comfortable braking — and
/// compliance comes from `compliance_rng`, so the template the version-1 view is
/// derived from (`passenger_car`, see `CompiledScenario`'s `v2_to_v1_view`)
/// samples the values `sample_profile` gives it, while every other wheeled box
/// samples its own authored values.
///
/// A validated wheeled box declares all six parameters and carries a box body,
/// so the fallbacks below are unreachable for a compiled scenario. Each is a
/// value the version-1 view uses rather than a new default: the zero range it
/// substitutes for a body it cannot read a box from, and the default envelope it
/// compiles when a scenario declares no `passenger_car` template. A template
/// that somehow lacks one samples its version-1 value instead of aborting an
/// otherwise valid run.
pub(crate) fn sample_mode_template_profile(
    template: &CompiledModeTemplate,
    profile_rng: &mut ChaCha20Rng,
    compliance_rng: &mut ChaCha20Rng,
) -> VehicleProfile {
    let profile = template.profile();
    let v1_view = ProfileSource::default();
    let v1_range = |range: ProfileRangeSource| ProfileRange::new(range.min, range.max);
    let (length_m, width_m) = match template.body() {
        AgentBody::Box { length_m, width_m } => (*length_m, *width_m),
        _ => (ProfileRange::new(0.0, 0.0), ProfileRange::new(0.0, 0.0)),
    };
    VehicleProfile {
        desired_speed_mps: profile.desired_speed_mps().sample(uniform01(profile_rng)),
        length_m: length_m.sample(uniform01(profile_rng)),
        width_m: width_m.sample(uniform01(profile_rng)),
        time_gap_s: profile
            .time_gap_s()
            .unwrap_or_else(|| v1_range(v1_view.time_gap_s))
            .sample(uniform01(profile_rng)),
        max_accel_mps2: profile
            .max_accel_mps2()
            .unwrap_or_else(|| v1_range(v1_view.max_accel_mps2))
            .sample(uniform01(profile_rng)),
        comfortable_brake_mps2: profile
            .comfortable_brake_mps2()
            .unwrap_or_else(|| v1_range(v1_view.comfortable_brake_mps2))
            .sample(uniform01(profile_rng)),
        compliance: profile.compliance().sample(uniform01(compliance_rng)),
    }
}

/// A sampled `ArticulatedWheeled` chain: the lead segment's own longitudinal
/// [`VehicleProfile`] (so every existing longitudinal, entry-admission, and
/// collision code path that reads an agent's `profile`/`body_length_m`/
/// `body_width_m` treats it exactly like a `WheeledBox`) plus every segment's
/// own drawn geometry and the chain's articulation limit.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ArticulatedChainProfile {
    pub(crate) vehicle: VehicleProfile,
    pub(crate) segments: Vec<ArticulatedSegmentGeometry>,
    pub(crate) articulation_limit_rad: f64,
}

/// Draw one articulated chain's body and dynamics from its own compiled
/// template.
///
/// The draw order, all from `profile_rng` except the final compliance draw:
/// desired speed; then, for each segment in chain order, its length, its
/// width, and (for every trailing segment only) its hitch/kingpin setback;
/// then the chain's articulation limit; then the shared wheeled longitudinal
/// set (time gap, maximum acceleration, comfortable braking) [`sample_profile`]
/// draws in the same relative order. Compliance is the one draw from
/// `compliance_rng`, last, exactly as every other sampler in this module. This
/// order has no legacy view to match — `ArticulatedWheeled` is a new family —
/// so it only needs to be internally consistent and is fixed here for
/// determinism: reordering these calls changes every sampled chain for the
/// same seed.
///
/// A validated articulated chain declares at least two segments and every
/// trailing segment's hitch offset, so the empty-chain fallback below is
/// unreachable for a compiled scenario.
pub(crate) fn sample_articulated_chain_profile(
    template: &CompiledModeTemplate,
    profile_rng: &mut ChaCha20Rng,
    compliance_rng: &mut ChaCha20Rng,
) -> ArticulatedChainProfile {
    let profile = template.profile();
    let (segment_sources, limit_range): (&[hekate_model::BodySegment], ProfileRange) =
        match template.body() {
            AgentBody::ArticulatedChain {
                segments,
                articulation_limit_rad,
            } => (segments.as_slice(), *articulation_limit_rad),
            _ => (&[], ProfileRange::new(0.0, 0.0)),
        };

    let desired_speed_mps = profile.desired_speed_mps().sample(uniform01(profile_rng));
    let segments: Vec<ArticulatedSegmentGeometry> = segment_sources
        .iter()
        .map(|segment| ArticulatedSegmentGeometry {
            length_m: segment.length_m().sample(uniform01(profile_rng)),
            width_m: segment.width_m().sample(uniform01(profile_rng)),
            hitch_offset_m: segment
                .hitch_offset_m()
                .map(|range| range.sample(uniform01(profile_rng))),
        })
        .collect();
    let articulation_limit_rad = limit_range.sample(uniform01(profile_rng));

    let v1_view = ProfileSource::default();
    let v1_range = |range: ProfileRangeSource| ProfileRange::new(range.min, range.max);
    let time_gap_s = profile
        .time_gap_s()
        .unwrap_or_else(|| v1_range(v1_view.time_gap_s))
        .sample(uniform01(profile_rng));
    let max_accel_mps2 = profile
        .max_accel_mps2()
        .unwrap_or_else(|| v1_range(v1_view.max_accel_mps2))
        .sample(uniform01(profile_rng));
    let comfortable_brake_mps2 = profile
        .comfortable_brake_mps2()
        .unwrap_or_else(|| v1_range(v1_view.comfortable_brake_mps2))
        .sample(uniform01(profile_rng));
    let compliance = profile.compliance().sample(uniform01(compliance_rng));

    let (lead_length_m, lead_width_m) = segments
        .first()
        .map_or((0.0, 0.0), |segment| (segment.length_m, segment.width_m));

    ArticulatedChainProfile {
        vehicle: VehicleProfile {
            desired_speed_mps,
            length_m: lead_length_m,
            width_m: lead_width_m,
            time_gap_s,
            max_accel_mps2,
            comfortable_brake_mps2,
            compliance,
        },
        segments,
        articulation_limit_rad,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::{STREAM_COMPLIANCE, STREAM_PROFILE, derive_stream};
    use hekate_model::{
        CompiledScenario, compile_mode_template, parse_scenario_source, parse_scenario_source_v2,
    };

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

    /// A version-2 scenario whose `passenger_car` template authors ranges wider
    /// than a point.
    ///
    /// A point range samples its own bound whatever the draw, so a fixture whose
    /// bodies and dynamics are constants cannot observe a draw-order or stream
    /// change. These ranges make [`sample_mode_template_profile`]'s draw order
    /// observable, which is the property the version-1 view's derivation needs.
    const VARIED_CAR: &str = r#"{
      schema_version: 2, id: 'varied_car',
      coordinate_system: { x: 'east_m', y: 'north_m' },
      paths: [], portals: [],
      mode_templates: [
        {
          id: 'passenger_car',
          body: { kind: 'box', length_m: { min: 4.0, max: 5.0 },
            width_m: { min: 1.7, max: 2.0 } },
          motion: 'single_body_wheeled',
          tactics: [ 'follow', 'stop', 'yield' ],
          access: { facility_kinds: [ 'facility' ], nominal_direction: 'either',
            speed_policy: { limit_mps: null } },
          occupancy: 'operator_only',
          profiles: {
            speed_mps: { min: 9.0, max: 15.0 },
            max_accel_mps2: { min: 1.2, max: 2.5 },
            comfortable_brake_mps2: { min: 2.0, max: 3.5 },
            time_gap_s: { min: 1.0, max: 2.0 },
            compliance: { min: 0.5, max: 1.0 },
          },
        },
      ],
    }"#;

    /// A wheeled box sampled from its own template draws exactly what the
    /// version-1 view draws for the template the view is derived from — the
    /// same values, from the same two streams, in the same order — so routing
    /// the `passenger_car` template through it cannot shift the existing
    /// goldens.
    #[test]
    fn a_template_sampled_passenger_car_matches_the_version_1_view() {
        let source = parse_scenario_source_v2(VARIED_CAR).expect("the document parses");
        let scenario = CompiledScenario::compile_v2(source).expect("the document compiles");
        let template = scenario
            .mode_templates()
            .iter()
            .find(|template| template.id() == "passenger_car")
            .expect("the document authors the car");

        let from_view = sample_profile(
            scenario.profiles(),
            &mut derive_stream(5, STREAM_PROFILE, 2),
            &mut derive_stream(5, STREAM_COMPLIANCE, 2),
        );
        let from_template = sample_mode_template_profile(
            template,
            &mut derive_stream(5, STREAM_PROFILE, 2),
            &mut derive_stream(5, STREAM_COMPLIANCE, 2),
        );
        assert_eq!(from_view, from_template);

        // The ranges are wider than a point, so the equality is the draw order
        // and the streams rather than a constant both sides return: every
        // sampled value lies inside its authored range, and at least one is
        // strictly inside it.
        let profile = template.profile();
        let ranges = [
            (from_template.desired_speed_mps, profile.desired_speed_mps()),
            (from_template.length_m, scenario.profiles().length_m()),
            (from_template.width_m, scenario.profiles().width_m()),
            (
                from_template.time_gap_s,
                profile.time_gap_s().expect("a wheeled box has a time gap"),
            ),
            (
                from_template.max_accel_mps2,
                profile
                    .max_accel_mps2()
                    .expect("a wheeled box has an acceleration"),
            ),
            (
                from_template.comfortable_brake_mps2,
                profile
                    .comfortable_brake_mps2()
                    .expect("a wheeled box has a braking bound"),
            ),
        ];
        assert!(
            ranges.iter().all(|(value, range)| *value >= range.min()
                && *value <= range.max()
                && (range.max() - range.min()) > 1e-9),
            "every sampled value must lie inside an authored range wider than a point: {ranges:?}"
        );
        assert!(
            ranges
                .iter()
                .any(|(value, range)| *value > range.min() + 1e-9 && *value < range.max() - 1e-9),
            "at least one sampled value must be strictly inside its range, so the draws, not the \
             point bounds, produced this profile: {ranges:?}"
        );
    }

    /// A lateral box template's three authored lateral parameters sample into
    /// the shared wheeled lateral limits, and a wheeled template without a
    /// `lateral` object samples none.
    #[test]
    fn a_lateral_wheeled_mode_samples_its_lateral_limits_from_its_template() {
        const BOXES: &str = r#"{
          schema_version: 2, id: 'lateral_boxes',
          coordinate_system: { x: 'east_m', y: 'north_m' },
          paths: [], portals: [],
          mode_templates: [
            {
              id: 'passenger_car',
              body: { kind: 'box', length_m: { min: 4.5, max: 4.5 },
                width_m: { min: 1.8, max: 1.8 } },
              motion: 'single_body_wheeled',
              tactics: [ 'follow', 'overtake' ],
              access: { facility_kinds: [ 'facility' ] },
              occupancy: 'operator_only',
              profiles: {
                speed_mps: { min: 9.0, max: 9.0 },
                max_accel_mps2: { min: 1.2, max: 1.2 },
                comfortable_brake_mps2: { min: 2.0, max: 2.0 },
                time_gap_s: { min: 1.0, max: 1.0 },
                steering_rate_max_rad_s: { min: 0.9, max: 0.9 },
                lateral_accel_max_mps2: { min: 2.0, max: 2.0 },
                lateral_clearance_m: { min: 0.3, max: 0.3 },
                compliance: { min: 1.0, max: 1.0 },
              },
              lateral: { target_clearance_m: 0.75, horizon_s: 2.0 },
            },
            {
              id: 'plain_car',
              body: { kind: 'box', length_m: { min: 4.5, max: 4.5 },
                width_m: { min: 1.8, max: 1.8 } },
              motion: 'single_body_wheeled',
              tactics: [ 'follow', 'stop', 'yield' ],
              access: { facility_kinds: [ 'facility' ] },
              occupancy: 'operator_only',
              profiles: {
                speed_mps: { min: 9.0, max: 9.0 },
                max_accel_mps2: { min: 1.2, max: 1.2 },
                comfortable_brake_mps2: { min: 2.0, max: 2.0 },
                time_gap_s: { min: 1.0, max: 1.0 },
                compliance: { min: 1.0, max: 1.0 },
              },
            },
          ],
        }"#;
        let source = parse_scenario_source_v2(BOXES).expect("the document parses");
        let lateral =
            compile_mode_template(&source.mode_templates[0]).expect("the lateral box compiles");
        let plain =
            compile_mode_template(&source.mode_templates[1]).expect("the plain box compiles");

        let limits =
            sample_wheeled_lateral_limits(&lateral, &mut derive_stream(1, STREAM_PROFILE, 0))
                .expect("a lateral box carries limits");
        assert!((limits.heading_rate_max_rad_s - 0.9).abs() < 1e-9);
        assert!((limits.lateral_accel_max_mps2 - 2.0).abs() < 1e-9);
        assert!((limits.lateral_clearance_m - 0.3).abs() < 1e-9);

        assert_eq!(
            sample_wheeled_lateral_limits(&plain, &mut derive_stream(1, STREAM_PROFILE, 0)),
            None,
            "a wheeled mode without a lateral policy carries no limits"
        );
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

    /// A minimal walkable scenario whose pedestrian ranges are degenerate, so
    /// a sample must equal the authored constant.
    const PEDESTRIAN_CONSTANT: &str = r#"
    {
      schema_version: 1,
      id: 'pedestrian_constant',
      coordinate_system: { x: 'east_m', y: 'north_m' },
      paths: [ { id: 'walk', points: [ { x: 0, y: 0 }, { x: 20, y: 0 } ] } ],
      portals: [
        { id: 'south', path: 'walk', end: 'start', width_m: 2.0 },
        { id: 'north', path: 'walk', end: 'end', width_m: 2.0 },
      ],
      pedestrian_profiles: {
        radius_m: { min: 0.25, max: 0.25 },
        speed_mps: { min: 1.2, max: 1.2 },
      },
    }
    "#;

    #[test]
    fn a_degenerate_pedestrian_range_samples_the_constant() {
        let source = parse_scenario_source(PEDESTRIAN_CONSTANT).expect("parses");
        let scenario = CompiledScenario::compile(source).expect("compiles");
        let profile = scenario.pedestrian_profiles();
        let sampled = sample_pedestrian_profile(
            profile,
            &mut derive_stream(1, STREAM_PROFILE, 0),
            &mut derive_stream(1, STREAM_COMPLIANCE, 0),
        );
        assert!((sampled.radius_m - 0.25).abs() < 1e-9);
        assert!((sampled.desired_speed_mps - 1.2).abs() < 1e-9);
        // The constant scenario omits `pedestrian_profiles.compliance`, so it
        // defaults to the fully compliant propensity.
        assert!((sampled.compliance - 1.0).abs() < 1e-9);
    }

    #[test]
    fn pedestrian_profiles_stay_within_their_authored_ranges() {
        // The walking scenario omits `pedestrian_profiles`, so this also covers
        // the additive default envelope.
        let compiled = scenario();
        let profile = compiled.pedestrian_profiles();
        let mut profile_rng = derive_stream(9, STREAM_PROFILE, 3);
        let mut compliance_rng = derive_stream(9, STREAM_COMPLIANCE, 3);
        for _ in 0..200 {
            let sampled = sample_pedestrian_profile(profile, &mut profile_rng, &mut compliance_rng);
            assert!(
                sampled.radius_m >= profile.radius_m().min() - 1e-9
                    && sampled.radius_m <= profile.radius_m().max() + 1e-9,
                "radius {} left its authored range",
                sampled.radius_m
            );
            assert!(
                sampled.desired_speed_mps >= profile.speed_mps().min() - 1e-9
                    && sampled.desired_speed_mps <= profile.speed_mps().max() + 1e-9,
                "speed {} left its authored range",
                sampled.desired_speed_mps
            );
            assert!(
                (0.0..=1.0).contains(&sampled.compliance),
                "compliance {} left [0, 1]",
                sampled.compliance
            );
        }
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
