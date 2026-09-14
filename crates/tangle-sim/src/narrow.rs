//! The narrow wheeled family's isolated longitudinal model and its model cards.
//!
//! `PHASE_2_PLAN.md` "Bicycles and scooters" gives bicycles and standing
//! scooters their own defaults and model cards while they share the wheeled
//! steering and lateral-maneuver machinery. This module is the narrow wheeled
//! longitudinal model that family shares: [`NarrowWheeledController`] is the
//! replaceable interface the kernel reaches it through, [`NarrowProfile`] is the
//! sampled parameter set it reads, and [`IdmNarrowWheeledController`] is the
//! initial implementation — the documented Intelligent Driver Model law of
//! [`crate::control`] under a narrow profile. The narrow-specific steering and
//! lateral-clearance parameters are carried on the profile for the Increment 2
//! lateral machinery; this Increment 1 model is longitudinal only, before free
//! lateral maneuvers are enabled.
//!
//! The two cards below document the two modes the family parameterizes. They
//! follow the checked-in template (`docs/model-card-template.md`), the inventory
//! of record, and the drift test (`crates/tangle-sim/tests/model_cards.rs`)
//! checks each of them against it. The behavior is one shared law; each card
//! states its own mode's body and parameter envelope.
//!
//! # Model card — bicycle
//!
//! The bicycle mode's longitudinal model. It is the shared documented
//! Intelligent Driver Model (IDM) law of [`crate::control`] — Martin Treiber,
//! Ansgar Hennecke, and Dirk Helbing, "Congested Traffic States in Empirical
//! Observations and Microscopic Simulations" (2000) — parameterized by a
//! bicycle's sampled [`NarrowProfile`] and reached only through
//! [`NarrowWheeledController`]. It is a documented, replaceable model, not a
//! calibrated scientific claim, and it is not fitted to bicycle observations.
//!
//! ## State
//!
//! The kernel holds one bicycle's longitudinal state: the speed `v` in m/s
//! along its reference path, the path progress that fixes its position and its
//! front-bumper progress, and the capsule body length that turns a
//! centre-to-centre distance into a bumper-to-bumper gap. In Increment 1 the
//! lateral offset and heading follow the reference path, so this model steers
//! nothing and has no lateral state; free lateral motion is Increment 2. Its
//! full output is one commanded acceleration.
//!
//! ## Parameters
//!
//! Parameters travel with the agent as a [`NarrowProfile`], built from the
//! bicycle mode template's authored ranges:
//!
//! - `desired_speed_mps` is `v0`, the free-flow speed (authored 3.5–6.5 m/s);
//! - `time_gap_s` is `T`, the desired following time gap (authored 0.8–1.4 s);
//! - `max_accel_mps2` is `a_max` (authored 0.8–1.5 m/s²);
//! - `comfortable_brake_mps2` is `b` (authored 1.5–3.0 m/s²);
//! - `length_m` and `radius_m` size the capsule body (authored 1.6–1.9 m long,
//!   0.30–0.40 m radius), so the bicycle is the longer, slimmer narrow mode;
//! - `steering_rate_max_rad_s` (authored 0.6–1.2 rad/s),
//!   `lateral_accel_max_mps2`, and `lateral_clearance_m` (authored 0.20–0.50 m)
//!   are carried for the Increment 2 lateral machinery and are not read by this
//!   longitudinal model; `lateral_accel_max_mps2` is present only when the
//!   template's compiled profile declares it;
//! - `compliance` (authored 0.8–1.0) belongs to the signal-compliance decision
//!   ([`crate::compliance`]), not to this longitudinal model.
//!
//! The controller never substitutes its own value for a parameter, so a sampled
//! profile fully determines the command.
//!
//! ## Constants
//!
//! The shared IDM constants, which are model properties and not sampled: the
//! free-flow acceleration exponent `delta = 4` ([`crate::control`]'s
//! `IDM_FREE_FLOW_EXPONENT`), the leader standstill gap the kernel uses when it
//! builds a leader constraint, and the gap floor that keeps a touching
//! constraint finite. This model adds no constant of its own.
//!
//! ## Decision inputs
//!
//! The kernel selects the constraints and passes at most three, each as a gap
//! in metres, a constraint speed in m/s, and a standstill gap in metres: the
//! nearest same-direction leader ahead on the reference path; a required stop
//! line; and an occupied crossing the bicycle is obliged to yield to. An empty
//! list is free-flow riding. The model sees only these constraints, the sampled
//! profile, and the current speed; the kernel owns which of them exist, and the
//! position caps in [Emergency backstop](#emergency-backstop) are applied
//! outside the model.
//!
//! ## Longitudinal law
//!
//! The command is the IDM acceleration
//!
//! ```text
//! a = a_max * [ 1 - (v / v0)^delta - sum_i (s*(v, dv_i) / gap_i)^2 ]
//! ```
//!
//! with `delta = 4` and the desired dynamic gap of a constraint
//! `s*(v, dv) = s0_i + max(0, v * T + v * dv / (2 * sqrt(a_max * b)))`, using
//! the profile's `a_max`, `b`, `T`, and `v0`. The narrow model is exactly the
//! [`crate::control`] IDM law under a bicycle profile, so cars, bicycles, and
//! scooters share one wheeled longitudinal family.
//!
//! ## Bounds
//!
//! The raw acceleration is clamped to `[-b, +a_max]`, so a bicycle's commanded
//! braking never exceeds its comfortable deceleration and its acceleration
//! never exceeds its maximum. The kernel additionally integrates speed within
//! `[0, v0]`.
//!
//! ## Tie-breaks
//!
//! The kernel passes at most one leader constraint, so the model itself never
//! chooses between candidates. The kernel scans live agents in ascending
//! [`crate::AgentId`] order and replaces the leader only for a strictly smaller
//! gap, so two candidates at exactly equal gaps resolve to the lowest agent id.
//!
//! ## Emergency backstop
//!
//! The profile bounds above describe the IDM command only. The kernel adds
//! position caps outside that clamp: the next speed may not pass the nearest
//! leader's rear, a required stop line, or a yield stop point short of an
//! occupied crossing in one step. Each step where a cap brakes harder than the
//! profile's comfortable value is counted in `Simulation::emergency_cap_steps`,
//! so a caller can assert the backstop stayed idle. A bicycle's low comfortable
//! braking makes the cap bind sooner than a car's at the same approach speed.
//!
//! ## Assumptions
//!
//! The model assumes one longitudinal degree of freedom: the bicycle is a point
//! mass on a fixed reference path, with no lateral state and no balance, lean,
//! pedaling, or dismounting. It assumes the sampled profile fully describes the
//! rider and is fixed for the run. It assumes the kernel supplies every
//! interaction constraint and every position cap above. Nothing here is
//! calibrated against observed bicycle trajectories.
//!
//! ## Parameter sources
//!
//! Every parameter is authored scenario data: the normalized `bicycle` mode
//! template's profile ranges, checked in by the mode-template fixture
//! (`crates/tangle-model/tests/fixtures/narrow_mode_templates_v2.json5`) and
//! required in full by the schema's narrow-wheeled validation rule. The numbers
//! are provisional engineering defaults, not fitted to observations, and no
//! parser default supplies them — a narrow wheeled template must state each
//! parameter explicitly or validation rejects it.
//!
//! ## Validated ranges
//!
//! Evidence so far covers the authored bicycle envelope above — desired speed
//! 3.5–6.5 m/s, acceleration 0.8–1.5 m/s², braking 1.5–3.0 m/s², time gap
//! 0.8–1.4 s — through this model's bound tests and the checked-in narrow
//! mode-template fixture. The end-to-end bicycle fixtures (straight, curve,
//! braking, following, signal, and crossing) are separate Increment 1 evidence,
//! so behavior outside the authored envelope is unvalidated rather than
//! credible.
//!
//! ## Known failure modes
//!
//! - The raw acceleration is clamped to `[-b, +a_max]`, so a constraint closer
//!   than the comfortable braking distance saturates the command; the kernel's
//!   position caps, not this model, keep bodies from overlapping.
//! - A low desired speed with a nonzero closing speed can demand more braking
//!   than `b`; the kernel cap then binds and the emergency count rises.
//! - The model has no lateral or steering state, so it cannot represent
//!   passing, lane changes, or wrong-way riding; those are Increment 2.
//! - A non-positive or non-finite desired speed is floored to
//!   `f64::MIN_POSITIVE`, and a non-finite gap contributes no interaction, so a
//!   malformed parameter or constraint is silently absorbed rather than
//!   rejected.
//!
//! ## Incompatible fidelity settings
//!
//! The model takes no fidelity parameter: `a_max`, `b`, `T`, and `delta` are
//! properties of the model, not of the step, so Fast, Standard, and Fine
//! produce the same command from the same state. It is incompatible with any
//! preset that disables the kernel's position caps or that requires lateral or
//! steering state, which it does not express.
//!
//! # Model card — scooter
//!
//! The standing-scooter mode's longitudinal model. It is the same shared
//! documented Intelligent Driver Model (IDM) law of [`crate::control`] —
//! Treiber, Hennecke & Helbing (2000) — parameterized by a scooter's sampled
//! [`NarrowProfile`] and reached only through [`NarrowWheeledController`]. Like
//! the bicycle card it documents a replaceable model, not a calibrated claim,
//! and it is not fitted to scooter observations.
//!
//! ## State
//!
//! The kernel holds one scooter's longitudinal state: the speed `v` in m/s
//! along its reference path, the path progress that fixes its position and its
//! front-bumper progress, and the capsule body length that turns a
//! centre-to-centre distance into a bumper-to-bumper gap. In Increment 1 the
//! lateral offset and heading follow the reference path, so this model steers
//! nothing and has no lateral state; free lateral motion is Increment 2. Its
//! full output is one commanded acceleration.
//!
//! ## Parameters
//!
//! Parameters travel with the agent as a [`NarrowProfile`], built from the
//! scooter mode template's authored ranges:
//!
//! - `desired_speed_mps` is `v0`, the free-flow speed (authored 4.0–7.5 m/s, so
//!   a scooter rides faster than a bicycle);
//! - `time_gap_s` is `T`, the desired following time gap (authored 0.8–1.4 s);
//! - `max_accel_mps2` is `a_max` (authored 1.0–2.0 m/s², so a scooter pulls
//!   away harder than a bicycle);
//! - `comfortable_brake_mps2` is `b` (authored 2.0–3.5 m/s², so a scooter
//!   brakes harder than a bicycle);
//! - `length_m` and `radius_m` size the capsule body (authored 1.0–1.2 m long,
//!   0.25–0.35 m radius), so the scooter is the shorter, more compact narrow
//!   mode;
//! - `steering_rate_max_rad_s` (authored 0.8–1.5 rad/s, quicker than a
//!   bicycle's), `lateral_accel_max_mps2`, and `lateral_clearance_m` (authored
//!   0.20–0.50 m) are carried for the Increment 2 lateral machinery and are not
//!   read by this longitudinal model;
//! - `compliance` (authored 0.6–1.0, wider and lower than a bicycle's) belongs
//!   to the signal-compliance decision ([`crate::compliance`]), not to this
//!   longitudinal model.
//!
//! The controller never substitutes its own value for a parameter, so a sampled
//! profile fully determines the command.
//!
//! ## Constants
//!
//! The shared IDM constants, which are model properties and not sampled: the
//! free-flow acceleration exponent `delta = 4` ([`crate::control`]'s
//! `IDM_FREE_FLOW_EXPONENT`), the leader standstill gap the kernel uses when it
//! builds a leader constraint, and the gap floor that keeps a touching
//! constraint finite. This model adds no constant of its own.
//!
//! ## Decision inputs
//!
//! The kernel selects the constraints and passes at most three, each as a gap
//! in metres, a constraint speed in m/s, and a standstill gap in metres: the
//! nearest same-direction leader ahead on the reference path; a required stop
//! line; and an occupied crossing the scooter is obliged to yield to. An empty
//! list is free-flow riding. The model sees only these constraints, the sampled
//! profile, and the current speed; the kernel owns which of them exist, and the
//! position caps in [Emergency backstop](#emergency-backstop) are applied
//! outside the model.
//!
//! ## Longitudinal law
//!
//! The command is the IDM acceleration
//!
//! ```text
//! a = a_max * [ 1 - (v / v0)^delta - sum_i (s*(v, dv_i) / gap_i)^2 ]
//! ```
//!
//! with `delta = 4` and the desired dynamic gap of a constraint
//! `s*(v, dv) = s0_i + max(0, v * T + v * dv / (2 * sqrt(a_max * b)))`, using
//! the profile's `a_max`, `b`, `T`, and `v0`. The scooter runs the same
//! narrow wheeled law as the bicycle, differing only in its sampled
//! parameters.
//!
//! ## Bounds
//!
//! The raw acceleration is clamped to `[-b, +a_max]`, so a scooter's commanded
//! braking never exceeds its comfortable deceleration and its acceleration
//! never exceeds its maximum. The kernel additionally integrates speed within
//! `[0, v0]`.
//!
//! ## Tie-breaks
//!
//! The kernel passes at most one leader constraint, so the model itself never
//! chooses between candidates. The kernel scans live agents in ascending
//! [`crate::AgentId`] order and replaces the leader only for a strictly smaller
//! gap, so two candidates at exactly equal gaps resolve to the lowest agent id.
//!
//! ## Emergency backstop
//!
//! The profile bounds above describe the IDM command only. The kernel adds
//! position caps outside that clamp: the next speed may not pass the nearest
//! leader's rear, a required stop line, or a yield stop point short of an
//! occupied crossing in one step. Each step where a cap brakes harder than the
//! profile's comfortable value is counted in `Simulation::emergency_cap_steps`,
//! so a caller can assert the backstop stayed idle. A scooter's higher free-flow
//! speed means a stop line entered while still fast can bind the cap despite its
//! firm braking.
//!
//! ## Assumptions
//!
//! The model assumes one longitudinal degree of freedom: the scooter is a point
//! mass on a fixed reference path, with no lateral state and no balance, lean,
//! kick, or dismount. It assumes the sampled profile fully describes the rider
//! and is fixed for the run. It assumes the kernel supplies every interaction
//! constraint and every position cap above. Nothing here is calibrated against
//! observed scooter trajectories.
//!
//! ## Parameter sources
//!
//! Every parameter is authored scenario data: the normalized `scooter` mode
//! template's profile ranges, checked in by the mode-template fixture
//! (`crates/tangle-model/tests/fixtures/narrow_mode_templates_v2.json5`) and
//! required in full by the schema's narrow-wheeled validation rule. The numbers
//! are provisional engineering defaults, not fitted to observations, and no
//! parser default supplies them — a narrow wheeled template must state each
//! parameter explicitly or validation rejects it.
//!
//! ## Validated ranges
//!
//! Evidence so far covers the authored scooter envelope above — desired speed
//! 4.0–7.5 m/s, acceleration 1.0–2.0 m/s², braking 2.0–3.5 m/s², time gap
//! 0.8–1.4 s — through this model's bound tests and the checked-in narrow
//! mode-template fixture. The end-to-end scooter fixtures (straight, curve,
//! braking, following, signal, and crossing) are separate Increment 1 evidence,
//! so behavior outside the authored envelope is unvalidated rather than
//! credible.
//!
//! ## Known failure modes
//!
//! - The raw acceleration is clamped to `[-b, +a_max]`, so a constraint closer
//!   than the comfortable braking distance saturates the command; the kernel's
//!   position caps, not this model, keep bodies from overlapping.
//! - The scooter's higher free-flow speed and lower mass assumption mean a
//!   stop line entered at speed can bind the kernel cap even when the
//!   comfortable braking is applied.
//! - The model has no lateral or steering state, so it cannot represent
//!   passing, lane changes, or wrong-way riding; those are Increment 2.
//! - A non-positive or non-finite desired speed is floored to
//!   `f64::MIN_POSITIVE`, and a non-finite gap contributes no interaction, so a
//!   malformed parameter or constraint is silently absorbed rather than
//!   rejected.
//!
//! ## Incompatible fidelity settings
//!
//! The model takes no fidelity parameter: `a_max`, `b`, `T`, and `delta` are
//! properties of the model, not of the step, so Fast, Standard, and Fine
//! produce the same command from the same state. It is incompatible with any
//! preset that disables the kernel's position caps or that requires lateral or
//! steering state, which it does not express.

use rand_chacha::ChaCha20Rng;
use tangle_model::{AgentBody, CompiledModeTemplate, PassingSide};

use crate::control::{self, Constraint};
use crate::profile::{VehicleProfile, WheeledLateralLimits};
use crate::rng::uniform01;
use crate::stage::PassSide;

/// A sampled physical and behavior profile for one narrow wheeled agent.
///
/// A narrow wheeled body is a capsule: a segment of `length_m` with a constant
/// `radius_m`, so its bounding width is twice the radius. The longitudinal
/// parameters are the ones the shared IDM law reads; the steering rate and
/// lateral clearance and acceleration are the narrow family's extra parameters,
/// carried for the Increment 2 lateral machinery. Values are built once from the mode template
/// and never resampled, so a body and its behavior are stable for a run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NarrowProfile {
    /// Desired free-flow speed in metres per second (`v0`).
    pub desired_speed_mps: f64,
    /// Capsule segment length in metres.
    pub length_m: f64,
    /// Capsule radius in metres.
    pub radius_m: f64,
    /// Desired following time gap in seconds (`T`).
    pub time_gap_s: f64,
    /// Maximum acceleration in metres per second squared (`a_max`).
    pub max_accel_mps2: f64,
    /// Comfortable deceleration in metres per second squared (`b`).
    pub comfortable_brake_mps2: f64,
    /// Maximum steering/heading rate in radians per second. Carried for the
    /// Increment 2 lateral machinery; unread by the longitudinal model.
    pub steering_rate_max_rad_s: f64,
    /// Preferred lateral clearance from a facility edge in metres. Carried for
    /// the Increment 2 lateral machinery; unread by the longitudinal model.
    pub lateral_clearance_m: f64,
    /// Maximum lateral acceleration in metres per second squared, when the
    /// template's compiled profile declares one; `None` leaves the mode without
    /// a bounded-steering envelope, exactly as Increment 1. Carried for the
    /// Increment 2 lateral machinery; unread by the longitudinal model.
    pub lateral_accel_max_mps2: Option<f64>,
    /// Signal-compliance propensity in `[0, 1]`. Read by the signal-compliance
    /// decision ([`crate::compliance`]), not by the longitudinal model.
    pub compliance: f64,
}

impl NarrowProfile {
    /// The capsule body's bounding width in metres: twice the radius. The body
    /// long axis is [`Self::length_m`].
    pub fn body_width_m(self) -> f64 {
        self.radius_m * 2.0
    }

    /// The shared wheeled longitudinal profile this narrow profile projects
    /// onto.
    ///
    /// The shared IDM controller and the kernel's signal, stop-line, and
    /// position-cap logic read a [`VehicleProfile`]; the narrow-wheeled extras
    /// (`steering_rate_max_rad_s`, `lateral_clearance_m`) stay on this type for
    /// the Increment 2 lateral machinery. The projection is pure and loses no
    /// longitudinal value, so a narrow agent runs the same shared stages as a
    /// car with no mode branch.
    pub fn vehicle_profile(self) -> VehicleProfile {
        VehicleProfile {
            desired_speed_mps: self.desired_speed_mps,
            length_m: self.length_m,
            width_m: self.body_width_m(),
            time_gap_s: self.time_gap_s,
            max_accel_mps2: self.max_accel_mps2,
            comfortable_brake_mps2: self.comfortable_brake_mps2,
            compliance: self.compliance,
        }
    }

    /// The bounded-steering limits this narrow profile carries, in the shared
    /// shape the wheeled lateral seam reads; `None` when the template declares
    /// no lateral-acceleration limit, exactly as a mode with no free lateral
    /// motion.
    pub(crate) fn lateral_limits(self) -> Option<WheeledLateralLimits> {
        self.lateral_accel_max_mps2
            .map(|lateral_accel_max_mps2| WheeledLateralLimits {
                heading_rate_max_rad_s: self.steering_rate_max_rad_s,
                lateral_accel_max_mps2,
                lateral_clearance_m: self.lateral_clearance_m,
            })
    }
}

/// Draw one narrow wheeled profile from a compiled narrow mode template.
///
/// The template must carry a capsule body and the narrow wheeled profile set
/// (validation guarantees both for a capsule that steers). Physical and
/// longitudinal draws come from `profile_rng` in a fixed order — desired speed,
/// capsule length, capsule radius, time gap, maximum acceleration, comfortable
/// braking, steering rate, then lateral clearance — so a narrow agent's body and
/// behavior are stable for the run; compliance comes from the separate
/// `compliance_rng`, exactly as the vehicle and pedestrian profiles split it.
/// Both streams are keyed by the stable agent id, which is unique across modes,
/// so a narrow agent never shares a generator with a car, a pedestrian, or
/// another narrow agent.
pub(crate) fn sample_narrow_profile(
    template: &CompiledModeTemplate,
    profile_rng: &mut ChaCha20Rng,
    compliance_rng: &mut ChaCha20Rng,
) -> NarrowProfile {
    let AgentBody::Capsule { length_m, radius_m } = template.body() else {
        panic!(
            "the narrow wheeled profile is sampled only for a capsule template, got a {} body",
            template.body().kind().label()
        );
    };
    let profile = template.profile();
    NarrowProfile {
        desired_speed_mps: profile.desired_speed_mps().sample(uniform01(profile_rng)),
        length_m: length_m.sample(uniform01(profile_rng)),
        radius_m: radius_m.sample(uniform01(profile_rng)),
        time_gap_s: profile
            .time_gap_s()
            .expect("a narrow wheeled template carries a time gap")
            .sample(uniform01(profile_rng)),
        max_accel_mps2: profile
            .max_accel_mps2()
            .expect("a narrow wheeled template carries an acceleration bound")
            .sample(uniform01(profile_rng)),
        comfortable_brake_mps2: profile
            .comfortable_brake_mps2()
            .expect("a narrow wheeled template carries a braking bound")
            .sample(uniform01(profile_rng)),
        steering_rate_max_rad_s: profile
            .steering_rate_max_rad_s()
            .expect("a narrow wheeled template carries a steering rate")
            .sample(uniform01(profile_rng)),
        lateral_clearance_m: profile
            .lateral_clearance_m()
            .expect("a narrow wheeled template carries a lateral clearance")
            .sample(uniform01(profile_rng)),
        // Appended after the Increment 1 draws so an existing narrow profile is
        // unchanged; a template with no compiled lateral acceleration draws
        // nothing and keeps the Increment 1 longitudinal-only behaviour.
        lateral_accel_max_mps2: profile
            .lateral_accel_max_mps2()
            .map(|range| range.sample(uniform01(profile_rng))),
        compliance: profile.compliance().sample(uniform01(compliance_rng)),
    }
}

/// Resolve the side a within-facility pass or overtake displaces toward.
///
/// `left_clearance_m` and `right_clearance_m` are the ordinary predictor's
/// predicted minimum swept clearances of a candidate target placed on the
/// positive- and negative-`d` side respectively, in the maneuvering agent's own
/// travel frame. A facility that authors `left` or `right` fixes the side from
/// policy alone, so the clearances are unread; `most_clearance` takes the
/// strictly greater predicted clearance, and an exact tie resolves to
/// [`PassSide::Left`], exactly as `docs/schema-v2-contract.md` *Increment 2
/// additions* fixes. The rule reads the authored policy and measured geometry,
/// never a mode or template id.
pub fn pass_side(policy: PassingSide, left_clearance_m: f64, right_clearance_m: f64) -> PassSide {
    match policy {
        PassingSide::Left => PassSide::Left,
        PassingSide::Right => PassSide::Right,
        PassingSide::MostClearance => {
            if left_clearance_m >= right_clearance_m {
                PassSide::Left
            } else {
                PassSide::Right
            }
        }
    }
}

/// One replaceable narrow wheeled longitudinal model.
///
/// Implementations must be pure in their inputs and must not draw from any
/// random stream, so swapping a model cannot reorder a named stream. A model
/// commands an acceleration only; the kernel clamps speed to `[0, v0]`, applies
/// the safety position caps, and counts the steps where a cap brakes harder than
/// the command's own bound. `Send + Sync` for the same reason as
/// [`crate::controller::VehicleController`].
pub(crate) trait NarrowWheeledController: std::fmt::Debug + Send + Sync {
    /// Stable model name, for provenance and diagnostics.
    fn name(&self) -> &'static str;

    /// Commanded longitudinal acceleration in m/s² for one step.
    ///
    /// `constraints` is the list of constraints ahead the kernel selected — a
    /// leader, a required stop line, an occupied crossing the agent yields to —
    /// or an empty slice for free-flow riding.
    fn desired_acceleration(
        &self,
        profile: &NarrowProfile,
        speed_mps: f64,
        constraints: &[Constraint],
    ) -> f64;
}

/// The initial narrow wheeled model: the shared IDM law under a narrow profile.
///
/// The equations, parameters, bounds, and backstops are the model cards in
/// [`crate::narrow`]; this type is only the seam binding, and it reads no mode
/// id — the command is a function of the profile and the constraints alone.
#[derive(Debug, Clone, Copy)]
pub(crate) struct IdmNarrowWheeledController;

impl NarrowWheeledController for IdmNarrowWheeledController {
    fn name(&self) -> &'static str {
        "idm-narrow"
    }

    fn desired_acceleration(
        &self,
        profile: &NarrowProfile,
        speed_mps: f64,
        constraints: &[Constraint],
    ) -> f64 {
        control::idm_acceleration(
            profile.max_accel_mps2,
            profile.comfortable_brake_mps2,
            profile.desired_speed_mps,
            profile.time_gap_s,
            speed_mps,
            constraints,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::{STREAM_COMPLIANCE, STREAM_PROFILE, derive_stream};
    use tangle_model::{
        AgentBody, BodyKind, CompiledModeTemplate, compile_mode_template, parse_scenario_source_v2,
    };

    /// The checked-in narrow mode-template fixture (TAS-076), the two narrow
    /// templates this model parameterizes.
    const FIXTURE: &str =
        include_str!("../../tangle-model/tests/fixtures/narrow_mode_templates_v2.json5");

    /// Every compiled capsule template in the fixture, in authoring order.
    fn narrow_templates() -> Vec<CompiledModeTemplate> {
        let source =
            parse_scenario_source_v2(FIXTURE).expect("the fixture is a version-2 document");
        source
            .mode_templates
            .iter()
            .map(|template| compile_mode_template(template).expect("the template compiles"))
            .filter(|template| template.body().kind() == BodyKind::Capsule)
            .collect()
    }

    /// One sampled profile for a template, from the same named streams the
    /// spawn path uses, keyed by a fixed agent id.
    fn sample(template: &CompiledModeTemplate, agent_id: u32) -> NarrowProfile {
        sample_narrow_profile(
            template,
            &mut derive_stream(7, STREAM_PROFILE, agent_id),
            &mut derive_stream(7, STREAM_COMPLIANCE, agent_id),
        )
    }

    fn leader(gap_m: f64, speed_mps: f64) -> Constraint {
        Constraint {
            gap_m,
            speed_mps,
            standstill_m: control::IDM_STANDSTILL_GAP_M,
        }
    }

    #[test]
    fn the_sampler_draws_within_the_authored_narrow_envelopes() {
        for template in narrow_templates() {
            let AgentBody::Capsule { length_m, radius_m } = template.body() else {
                panic!("a narrow template carries a capsule body");
            };
            let profile = template.profile();
            for agent_id in 0..200 {
                let sampled = sample(&template, agent_id);
                assert!((length_m.min()..=length_m.max()).contains(&sampled.length_m));
                assert!((radius_m.min()..=radius_m.max()).contains(&sampled.radius_m));
                assert!(
                    (profile.desired_speed_mps().min()..=profile.desired_speed_mps().max())
                        .contains(&sampled.desired_speed_mps)
                );
                assert!((0.0..=1.0).contains(&sampled.compliance));
            }
        }
    }

    #[test]
    fn the_two_narrow_templates_sample_distinct_profiles() {
        let templates = narrow_templates();
        assert_eq!(
            templates.len(),
            2,
            "the narrow fixture authors exactly the bicycle and scooter capsule templates"
        );
        let bicycle = sample(&templates[0], 0);
        let scooter = sample(&templates[1], 0);
        assert_ne!(
            bicycle, scooter,
            "bicycle and scooter sample different profiles"
        );
        assert!((bicycle.desired_speed_mps - scooter.desired_speed_mps).abs() > 1e-12);
    }

    #[test]
    fn the_scooter_capsule_is_shorter_than_the_bicycle_capsule() {
        let templates = narrow_templates();
        let bicycle = sample(&templates[0], 0);
        let scooter = sample(&templates[1], 0);
        // The fixture authors the bicycle's capsule longer than the scooter's, so
        // every draw keeps the bicycle the longer body.
        assert!(bicycle.length_m > scooter.length_m);
        assert!((bicycle.body_width_m() - bicycle.radius_m * 2.0).abs() < 1e-12);
    }

    #[test]
    fn the_command_depends_only_on_the_profile_not_the_template_id() {
        // The no-id-branch probe for the narrow model: a template renamed to a
        // different id but otherwise identical compiles to the same components,
        // samples the same profile, and commands the same acceleration. If the
        // model branched on the id, the renamed template would diverge here.
        let source =
            parse_scenario_source_v2(FIXTURE).expect("the fixture is a version-2 document");
        let original = source
            .mode_templates
            .iter()
            .find(|template| matches!(template.body, tangle_model::ModeBodySource::Capsule { .. }))
            .expect("the fixture authors a capsule template");
        let mut renamed = original.clone();
        renamed.id = "a_different_name".to_owned();

        let first = compile_mode_template(original).expect("the template compiles");
        let second = compile_mode_template(&renamed).expect("renaming the id keeps the components");
        assert_ne!(first.id(), second.id());

        let before = sample(&first, 3);
        let after = sample(&second, 3);
        assert_eq!(before, after, "the id never reaches the sampled profile");

        let model = IdmNarrowWheeledController;
        let constraints = [leader(8.0, 2.0)];
        assert_eq!(
            model.desired_acceleration(&before, 4.0, &constraints),
            model.desired_acceleration(&after, 4.0, &constraints),
        );
    }

    #[test]
    fn the_command_respects_the_narrow_acceleration_and_braking_bounds() {
        let profile = sample(&narrow_templates()[0], 0);
        let model = IdmNarrowWheeledController;

        // Free flow from rest cannot exceed the profile's maximum acceleration.
        let free = model.desired_acceleration(&profile, 0.0, &[]);
        assert!((free - profile.max_accel_mps2).abs() < 1e-9);

        // A closing bicycle almost touching a stopped leader demands the most
        // braking the model can express, clamped to the comfortable value.
        let tight = [leader(0.01, 0.0)];
        let braking = model.desired_acceleration(&profile, profile.desired_speed_mps, &tight);
        assert!((braking + profile.comfortable_brake_mps2).abs() < 1e-9);
    }

    #[test]
    fn the_narrow_command_is_the_shared_idm_law() {
        // The narrow model is not a second implementation: it is the same IDM
        // law, parameterized by the narrow profile's longitudinal values. The
        // vehicle profile the narrow profile projects onto commands the same
        // acceleration at the same state.
        let narrow = sample(&narrow_templates()[0], 1);
        let constraints = [leader(6.0, 1.0)];
        let narrow_command =
            IdmNarrowWheeledController.desired_acceleration(&narrow, 3.0, &constraints);
        let projected_command =
            control::desired_acceleration(&narrow.vehicle_profile(), 3.0, &constraints);
        assert_eq!(narrow_command, projected_command);
    }

    #[test]
    fn the_narrow_model_reports_a_stable_name() {
        assert_eq!(IdmNarrowWheeledController.name(), "idm-narrow");
    }

    /// The side a pass selects: a `left`/`right` policy fixes it from the
    /// authored policy alone, and `most_clearance` takes the greater predicted
    /// clearance with an exact tie resolving to `left`.
    #[test]
    fn the_pass_side_reads_policy_then_geometry_with_a_left_tie() {
        assert_eq!(pass_side(PassingSide::Left, 0.1, 5.0), PassSide::Left);
        assert_eq!(pass_side(PassingSide::Right, 5.0, 0.1), PassSide::Right);
        assert_eq!(
            pass_side(PassingSide::MostClearance, 1.0, 0.5),
            PassSide::Left,
            "the greater left clearance wins"
        );
        assert_eq!(
            pass_side(PassingSide::MostClearance, 0.5, 1.0),
            PassSide::Right,
            "the greater right clearance wins"
        );
        assert_eq!(
            pass_side(PassingSide::MostClearance, 0.5, 0.5),
            PassSide::Left,
            "an exact tie resolves to left"
        );
    }
}
