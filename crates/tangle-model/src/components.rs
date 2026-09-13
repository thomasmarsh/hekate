//! Compiled agent components: a compact core plus composable optional parts.
//!
//! `PHASE_2_PLAN.md` describes every simulated road user as a stable agent id
//! plus a compact core (route, pose, velocity, intent, behavior profile,
//! lifecycle state) and the variant data that supplies what differs: body,
//! motion, tactical capabilities, access, occupancy, and social state. A named
//! mode template (see [`crate::source::ModeTemplateSource`]) compiles into one
//! [`AgentComponents`] bundle; shared kernel behavior reads the components and
//! dispatches on [`AgentFamily`], which is derived from the body and motion
//! components and never from a mode id or template name.
//!
//! This module is additive. It does not change [`crate::CompiledScenario`] or
//! any version-1 or version-2 compilation output: wiring authored templates
//! into these bundles is a separate step.

use glam::DVec2;

use crate::compiled::{MovementId, PedestrianRouteId, ProfileRange};
use crate::source::{FacilityKind, RuleKind};

/// The envelope kind of a compiled body component.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BodyKind {
    /// A circle.
    Circle,
    /// An oriented rectangle.
    Box,
    /// A capsule: a segment with a constant radius.
    Capsule,
    /// An ordered chain of convex box segments.
    ArticulatedChain,
}

impl BodyKind {
    /// Short stable label for diagnostics and traces.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Circle => "circle",
            Self::Box => "box",
            Self::Capsule => "capsule",
            Self::ArticulatedChain => "articulated_chain",
        }
    }
}

/// One box segment of an ordered articulated body chain.
///
/// Dimensions are distributions, not sampled values: one value per range is
/// drawn per agent so its body is stable for a run.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodySegment {
    length_m: ProfileRange,
    width_m: ProfileRange,
}

impl BodySegment {
    /// Construct a segment from its length and width distributions.
    pub const fn new(length_m: ProfileRange, width_m: ProfileRange) -> Self {
        Self { length_m, width_m }
    }

    /// Segment length distribution in metres.
    pub fn length_m(self) -> ProfileRange {
        self.length_m
    }

    /// Segment width distribution in metres.
    pub fn width_m(self) -> ProfileRange {
        self.width_m
    }
}

/// The compiled body geometry of one agent.
///
/// The variant is the body family: a single circle, box, or capsule envelope,
/// or an ordered articulated chain of convex segments, front to back.
#[derive(Debug, Clone, PartialEq)]
pub enum AgentBody {
    /// A circular envelope, such as a pedestrian.
    Circle {
        /// Body radius distribution in metres.
        radius_m: ProfileRange,
    },
    /// An oriented rectangular envelope, such as a car, bus, or rigid truck.
    Box {
        /// Body length distribution in metres.
        length_m: ProfileRange,
        /// Body width distribution in metres.
        width_m: ProfileRange,
    },
    /// A capsule envelope: a segment of the given length with a constant
    /// radius, such as a bicycle or scooter.
    Capsule {
        /// Body length distribution in metres.
        length_m: ProfileRange,
        /// Body radius distribution in metres.
        radius_m: ProfileRange,
    },
    /// An ordered articulated chain of convex box segments, such as a
    /// tractor-semitrailer.
    ArticulatedChain {
        /// Segments in chain order, front to back; the list must not be empty.
        segments: Vec<BodySegment>,
    },
}

impl AgentBody {
    /// The envelope kind of this body.
    pub fn kind(&self) -> BodyKind {
        match self {
            Self::Circle { .. } => BodyKind::Circle,
            Self::Box { .. } => BodyKind::Box,
            Self::Capsule { .. } => BodyKind::Capsule,
            Self::ArticulatedChain { .. } => BodyKind::ArticulatedChain,
        }
    }

    /// Number of convex segments the envelope is built from: one for a single
    /// shape, the chain length for an articulated body.
    pub fn segment_count(&self) -> usize {
        match self {
            Self::ArticulatedChain { segments } => segments.len(),
            Self::Circle { .. } | Self::Box { .. } | Self::Capsule { .. } => 1,
        }
    }
}

/// The motion family of one compiled agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentMotion {
    /// Walking with no steering constraint.
    HolonomicWalking,
    /// A single rigid wheeled body steering along a reference path.
    SingleBodyWheeled,
    /// A wheeled tractor pulling hinged, articulated segments.
    ArticulatedWheeled,
}

impl AgentMotion {
    /// Short stable label for diagnostics and traces.
    pub const fn label(self) -> &'static str {
        match self {
            Self::HolonomicWalking => "holonomic_walking",
            Self::SingleBodyWheeled => "single_body_wheeled",
            Self::ArticulatedWheeled => "articulated_wheeled",
        }
    }
}

/// The small body/motion family shared runtime behavior dispatches on.
///
/// A family is derived from an agent's body and motion components and from
/// nothing else: the kernel never branches on an authored mode id or template
/// name. A body and motion pair no family serves is rejected when the bundle is
/// composed, so a family is total over composed agents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentFamily {
    /// A circular, holonomically walking body.
    HolonomicCircle,
    /// A single oriented box that steers.
    WheeledBox,
    /// A capsule envelope that steers.
    WheeledCapsule,
    /// An ordered articulated chain that steers.
    ArticulatedWheeled,
}

impl AgentFamily {
    /// Every family the runtime serves, in declaration order.
    pub const ALL: [Self; 4] = [
        Self::HolonomicCircle,
        Self::WheeledBox,
        Self::WheeledCapsule,
        Self::ArticulatedWheeled,
    ];

    /// Short stable label for diagnostics and traces.
    pub const fn label(self) -> &'static str {
        match self {
            Self::HolonomicCircle => "holonomic_circle",
            Self::WheeledBox => "wheeled_box",
            Self::WheeledCapsule => "wheeled_capsule",
            Self::ArticulatedWheeled => "articulated_wheeled",
        }
    }
}

/// A tactical maneuver an agent's capabilities may include.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TacticalCapability {
    /// Follow the agent ahead.
    Follow,
    /// Stop for a control, a service, or an obstruction.
    Stop,
    /// Yield to a conflicting user.
    Yield,
    /// Choose a lateral position on the current facility.
    ChooseLateralPosition,
    /// Change lane on a multi-lane facility.
    ChangeLane,
    /// Overtake a slower leader.
    Overtake,
    /// Pass a narrow user within the same lane.
    Pass,
    /// Travel against the facility's nominal direction.
    ReverseNominalDirection,
    /// Serve a stop: align, open service, dwell, and rejoin.
    ServeStop,
}

impl TacticalCapability {
    /// Every capability, in declaration order.
    pub const ALL: [Self; 9] = [
        Self::Follow,
        Self::Stop,
        Self::Yield,
        Self::ChooseLateralPosition,
        Self::ChangeLane,
        Self::Overtake,
        Self::Pass,
        Self::ReverseNominalDirection,
        Self::ServeStop,
    ];

    /// One bit per capability, for the compact capability set.
    const fn bit(self) -> u16 {
        1 << self as u16
    }
}

/// The tactical capabilities one agent supports, as a compact ordered set.
///
/// Storing the set as a bit field keeps hot per-agent state small, while
/// iteration follows [`TacticalCapability::ALL`] order so a bundle that enables
/// capabilities in any order still iterates deterministically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TacticalCapabilities {
    bits: u16,
}

impl TacticalCapabilities {
    /// No capabilities.
    pub const fn none() -> Self {
        Self { bits: 0 }
    }

    /// Whether no capability is supported.
    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }

    /// This set plus `capability`.
    pub const fn with(self, capability: TacticalCapability) -> Self {
        Self {
            bits: self.bits | capability.bit(),
        }
    }

    /// Whether `capability` is supported.
    pub const fn supports(self, capability: TacticalCapability) -> bool {
        self.bits & capability.bit() != 0
    }

    /// The supported capabilities, in declaration order.
    pub fn iter(self) -> impl Iterator<Item = TacticalCapability> {
        TacticalCapability::ALL
            .into_iter()
            .filter(move |capability| self.supports(*capability))
    }
}

impl FromIterator<TacticalCapability> for TacticalCapabilities {
    fn from_iter<I: IntoIterator<Item = TacticalCapability>>(iter: I) -> Self {
        iter.into_iter()
            .fold(Self::none(), |set, capability| set.with(capability))
    }
}

/// The direction an agent's access permits along a reference facility.
///
/// Nominal direction and physical traversability stay separate scenario
/// properties: `Forward` is the facility's authored direction, `Reverse` is
/// travel against it, and `Either` permits both.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NominalDirection {
    /// Only the facility's nominal direction.
    Forward,
    /// Only against the facility's nominal direction.
    Reverse,
    /// Either direction.
    Either,
}

/// The speed policy an agent's access imposes on its travel.
///
/// The policy is the enforced limit, which is separate from the agent's
/// desired-speed distribution in [`AgentBehaviorProfile`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeedPolicy {
    limit_mps: Option<f64>,
}

impl SpeedPolicy {
    /// No enforced limit beyond the agent's own motion limits.
    pub const fn unlimited() -> Self {
        Self { limit_mps: None }
    }

    /// An enforced limit in metres per second.
    pub const fn limited(limit_mps: f64) -> Self {
        Self {
            limit_mps: Some(limit_mps),
        }
    }

    /// The enforced limit in metres per second, if any.
    pub const fn limit_mps(self) -> Option<f64> {
        self.limit_mps
    }
}

/// The compiled access of one agent: which facilities it may use, in which
/// direction, under which speed policy, and subject to which rules.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentAccess {
    facility_kinds: Vec<FacilityKind>,
    nominal_direction: NominalDirection,
    speed_policy: SpeedPolicy,
    rule_kinds: Vec<RuleKind>,
}

impl AgentAccess {
    /// Compose access from its parts, keeping the given facility and rule order.
    pub fn new(
        facility_kinds: Vec<FacilityKind>,
        nominal_direction: NominalDirection,
        speed_policy: SpeedPolicy,
        rule_kinds: Vec<RuleKind>,
    ) -> Self {
        Self {
            facility_kinds,
            nominal_direction,
            speed_policy,
            rule_kinds,
        }
    }

    /// Traversable object kinds the agent may use.
    pub fn facility_kinds(&self) -> &[FacilityKind] {
        &self.facility_kinds
    }

    /// Whether the agent may use `kind`.
    pub fn permits(&self, kind: FacilityKind) -> bool {
        self.facility_kinds.contains(&kind)
    }

    /// The direction the agent may travel in.
    pub fn nominal_direction(&self) -> NominalDirection {
        self.nominal_direction
    }

    /// The speed policy the agent travels under.
    pub fn speed_policy(&self) -> SpeedPolicy {
        self.speed_policy
    }

    /// Rule kinds the agent is subject to.
    pub fn rule_kinds(&self) -> &[RuleKind] {
        &self.rule_kinds
    }

    /// Whether the agent is subject to `rule`.
    pub fn subject_to(&self, rule: RuleKind) -> bool {
        self.rule_kinds.contains(&rule)
    }
}

/// Aggregate transit occupancy: a passenger capacity and its boarding state.
///
/// Boarding is aggregate rather than per-passenger, so a vehicle carries one
/// passenger count and a capacity instead of an interior model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TransitOccupancy {
    capacity: u32,
    onboard: u32,
}

impl TransitOccupancy {
    /// An occupied/empty transit vehicle with `capacity` passenger places;
    /// `onboard` is clamped to `capacity`.
    pub const fn new(capacity: u32, onboard: u32) -> Self {
        Self {
            capacity,
            onboard: if onboard < capacity {
                onboard
            } else {
                capacity
            },
        }
    }

    /// Passenger places the vehicle carries in total.
    pub const fn capacity(self) -> u32 {
        self.capacity
    }

    /// Passengers currently on board.
    pub const fn onboard(self) -> u32 {
        self.onboard
    }

    /// Passenger places still free.
    pub const fn spare_capacity(self) -> u32 {
        self.capacity - self.onboard
    }

    /// Admit up to the spare capacity and return the denied passengers.
    pub fn board(&mut self, passengers: u32) -> u32 {
        let admitted = passengers.min(self.spare_capacity());
        self.onboard += admitted;
        passengers - admitted
    }

    /// Remove up to the onboard passengers and return how many alighted.
    pub fn alight(&mut self, passengers: u32) -> u32 {
        let leaving = passengers.min(self.onboard);
        self.onboard -= leaving;
        leaving
    }
}

/// The compiled occupancy of one agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentOccupancy {
    /// One operator and no passengers.
    OperatorOnly,
    /// A fixed number of occupants.
    Fixed {
        /// Occupants the agent carries, operator included.
        occupants: u32,
    },
    /// A transit vehicle carrying passengers under a capacity.
    Transit(TransitOccupancy),
}

/// Dense index of a compiled pedestrian group.
///
/// A group is a relationship among ordinary pedestrian agents rather than one
/// collision body, so membership names the group by index while each member
/// keeps its own body and contacts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PedestrianGroupId(u32);

impl PedestrianGroupId {
    /// Construct a group identifier from its dense index.
    pub const fn from_index(index: usize) -> Self {
        Self(index as u32)
    }

    /// The zero-based index of this group.
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    /// The raw integer value, suitable for serialization.
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// A pedestrian's role within its group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GroupRole {
    /// An ordinary member that follows the group's shared destination and
    /// crossing decision.
    Member,
    /// The member whose desired speed and crossing decision the group shares.
    Leader,
}

/// Membership of one pedestrian in one group.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GroupMembership {
    group: PedestrianGroupId,
    role: GroupRole,
}

impl GroupMembership {
    /// Construct membership naming the group and the member's role.
    pub const fn new(group: PedestrianGroupId, role: GroupRole) -> Self {
        Self { group, role }
    }

    /// The group the agent belongs to.
    pub const fn group(self) -> PedestrianGroupId {
        self.group
    }

    /// The agent's role in the group.
    pub const fn role(self) -> GroupRole {
        self.role
    }
}

/// The compiled social state of one agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SocialState {
    /// Not a member of any group.
    Individual,
    /// A member of one pedestrian group.
    GroupMember(GroupMembership),
}

/// The route an agent follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentRoute {
    /// A vehicle movement connector.
    Movement(MovementId),
    /// A pedestrian route.
    Pedestrian(PedestrianRouteId),
}

/// The world pose of an agent.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AgentPose {
    position: DVec2,
    heading_rad: f64,
}

impl AgentPose {
    /// Construct a pose from a world position and a heading in radians.
    pub const fn new(position: DVec2, heading_rad: f64) -> Self {
        Self {
            position,
            heading_rad,
        }
    }

    /// World position in metres.
    pub const fn position(self) -> DVec2 {
        self.position
    }

    /// World heading in radians, counter-clockwise from the x axis.
    pub const fn heading_rad(self) -> f64 {
        self.heading_rad
    }
}

/// The world velocity of an agent.
///
/// World velocity is the collision and output truth; route-relative
/// longitudinal progress and lateral offset are derived from it every step
/// rather than stored as the authoritative position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AgentVelocity {
    velocity_mps: DVec2,
}

impl AgentVelocity {
    /// Construct a velocity from its world vector in metres per second.
    pub const fn new(velocity_mps: DVec2) -> Self {
        Self { velocity_mps }
    }

    /// The world velocity vector in metres per second.
    pub const fn vector(self) -> DVec2 {
        self.velocity_mps
    }

    /// Speed in metres per second.
    pub fn speed_mps(self) -> f64 {
        self.velocity_mps.length()
    }
}

/// The tactical intent an agent is currently executing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentIntent {
    /// Traveling toward the route destination with no maneuver selected.
    Travel,
    /// Executing one of the agent's tactical capabilities.
    Tactic(TacticalCapability),
}

impl AgentIntent {
    /// The tactic being executed, if any.
    pub const fn tactic(self) -> Option<TacticalCapability> {
        match self {
            Self::Travel => None,
            Self::Tactic(capability) => Some(capability),
        }
    }
}

/// Where an agent is in its life cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AgentLifecycle {
    /// In the world and updated every step.
    Active,
    /// Holding at the end of its route without interacting.
    Arrived,
    /// Removed from the world; its slot is retained so identifiers and
    /// iteration order never shift.
    Departed,
}

/// The behavior distributions one agent samples its controller parameters from.
///
/// A family reads only the parameters it uses, so walking agents carry no
/// following time gap and wheeled agents carry no walking-specific value. The
/// narrow wheeled family (a capsule that steers) additionally carries its
/// steering response and lateral-clearance preference; every other family
/// leaves those absent. Body dimensions are not here: they belong to the body
/// component.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AgentBehaviorProfile {
    desired_speed_mps: ProfileRange,
    compliance: ProfileRange,
    time_gap_s: Option<ProfileRange>,
    max_accel_mps2: Option<ProfileRange>,
    comfortable_brake_mps2: Option<ProfileRange>,
    steering_rate_max_rad_s: Option<ProfileRange>,
    lateral_clearance_m: Option<ProfileRange>,
}

impl AgentBehaviorProfile {
    /// A holonomically walking agent's behavior: desired speed and a
    /// signal-compliance propensity.
    pub const fn walking(desired_speed_mps: ProfileRange, compliance: ProfileRange) -> Self {
        Self {
            desired_speed_mps,
            compliance,
            time_gap_s: None,
            max_accel_mps2: None,
            comfortable_brake_mps2: None,
            steering_rate_max_rad_s: None,
            lateral_clearance_m: None,
        }
    }

    /// A wheeled agent's behavior: desired speed, following time gap,
    /// acceleration and braking limits, and a rule-compliance propensity.
    ///
    /// This is the Increment 0 wheeled set; a narrow wheeled agent adds its
    /// steering and lateral-clearance parameters through
    /// [`Self::narrow_wheeled`].
    pub const fn wheeled(
        desired_speed_mps: ProfileRange,
        time_gap_s: ProfileRange,
        max_accel_mps2: ProfileRange,
        comfortable_brake_mps2: ProfileRange,
        compliance: ProfileRange,
    ) -> Self {
        Self {
            desired_speed_mps,
            compliance,
            time_gap_s: Some(time_gap_s),
            max_accel_mps2: Some(max_accel_mps2),
            comfortable_brake_mps2: Some(comfortable_brake_mps2),
            steering_rate_max_rad_s: None,
            lateral_clearance_m: None,
        }
    }

    /// A narrow wheeled agent's behavior: the wheeled set plus the steering
    /// response and lateral-clearance preference a capsule body steering on a
    /// reference path uses.
    pub const fn narrow_wheeled(
        desired_speed_mps: ProfileRange,
        time_gap_s: ProfileRange,
        max_accel_mps2: ProfileRange,
        comfortable_brake_mps2: ProfileRange,
        steering_rate_max_rad_s: ProfileRange,
        lateral_clearance_m: ProfileRange,
        compliance: ProfileRange,
    ) -> Self {
        Self {
            desired_speed_mps,
            compliance,
            time_gap_s: Some(time_gap_s),
            max_accel_mps2: Some(max_accel_mps2),
            comfortable_brake_mps2: Some(comfortable_brake_mps2),
            steering_rate_max_rad_s: Some(steering_rate_max_rad_s),
            lateral_clearance_m: Some(lateral_clearance_m),
        }
    }

    /// Desired free-flow speed distribution in metres per second.
    pub fn desired_speed_mps(&self) -> ProfileRange {
        self.desired_speed_mps
    }

    /// Rule-compliance propensity distribution, a fraction in `[0, 1]`.
    pub fn compliance(&self) -> ProfileRange {
        self.compliance
    }

    /// Desired following time-gap distribution, absent for a walking agent.
    pub fn time_gap_s(&self) -> Option<ProfileRange> {
        self.time_gap_s
    }

    /// Maximum acceleration distribution, absent for a walking agent.
    pub fn max_accel_mps2(&self) -> Option<ProfileRange> {
        self.max_accel_mps2
    }

    /// Comfortable deceleration distribution, absent for a walking agent.
    pub fn comfortable_brake_mps2(&self) -> Option<ProfileRange> {
        self.comfortable_brake_mps2
    }

    /// Maximum steering/heading rate distribution in radians per second, present
    /// only for a narrow wheeled agent (a capsule that steers).
    pub fn steering_rate_max_rad_s(&self) -> Option<ProfileRange> {
        self.steering_rate_max_rad_s
    }

    /// Preferred lateral clearance from the facility edge distribution in
    /// metres, present only for a narrow wheeled agent.
    pub fn lateral_clearance_m(&self) -> Option<ProfileRange> {
        self.lateral_clearance_m
    }
}

/// The compact core every agent has, whatever its components.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AgentCore {
    route: AgentRoute,
    pose: AgentPose,
    velocity: AgentVelocity,
    intent: AgentIntent,
    profile: AgentBehaviorProfile,
    lifecycle: AgentLifecycle,
}

impl AgentCore {
    /// Construct a core from its components.
    pub const fn new(
        route: AgentRoute,
        pose: AgentPose,
        velocity: AgentVelocity,
        intent: AgentIntent,
        profile: AgentBehaviorProfile,
        lifecycle: AgentLifecycle,
    ) -> Self {
        Self {
            route,
            pose,
            velocity,
            intent,
            profile,
            lifecycle,
        }
    }

    /// The route the agent follows.
    pub const fn route(self) -> AgentRoute {
        self.route
    }

    /// The agent's world pose.
    pub const fn pose(self) -> AgentPose {
        self.pose
    }

    /// The agent's world velocity.
    pub const fn velocity(self) -> AgentVelocity {
        self.velocity
    }

    /// The tactical intent the agent is executing.
    pub const fn intent(self) -> AgentIntent {
        self.intent
    }

    /// The behavior distributions the agent samples from.
    pub const fn profile(self) -> AgentBehaviorProfile {
        self.profile
    }

    /// Where the agent is in its life cycle.
    pub const fn lifecycle(self) -> AgentLifecycle {
        self.lifecycle
    }
}

/// A body and motion pair no runtime family serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error(
    "no agent family serves a {body} body with {motion} motion",
    body = .body.label(),
    motion = .motion.label()
)]
pub struct ComponentMismatch {
    body: BodyKind,
    motion: AgentMotion,
}

impl ComponentMismatch {
    /// The envelope kind of the rejected body.
    pub const fn body(self) -> BodyKind {
        self.body
    }

    /// The rejected motion family.
    pub const fn motion(self) -> AgentMotion {
        self.motion
    }
}

/// One compiled agent: a compact core plus the components that differ.
///
/// The variant components are composable and independent: a bundle carries
/// exactly one body, one motion, one tactical capability set, one access, one
/// occupancy, and one social state. The body/motion [`AgentFamily`] is derived
/// once when the bundle is composed, so dispatch never reads a mode name.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentComponents {
    core: AgentCore,
    body: AgentBody,
    motion: AgentMotion,
    tactics: TacticalCapabilities,
    access: AgentAccess,
    occupancy: AgentOccupancy,
    social: SocialState,
    family: AgentFamily,
}

impl AgentComponents {
    /// Compose one agent from its core and its components.
    ///
    /// The family is derived from the body and motion components here — no mode
    /// id, template name, or other string participates — and a pair no family
    /// serves is rejected as [`ComponentMismatch`]. A composed bundle therefore
    /// always has a family, and [`Self::family`] cannot fail.
    pub fn compose(
        core: AgentCore,
        body: AgentBody,
        motion: AgentMotion,
        tactics: TacticalCapabilities,
        access: AgentAccess,
        occupancy: AgentOccupancy,
        social: SocialState,
    ) -> Result<Self, ComponentMismatch> {
        let family = derive_family(body.kind(), motion)?;
        Ok(Self {
            core,
            body,
            motion,
            tactics,
            access,
            occupancy,
            social,
            family,
        })
    }

    /// The agent's compact core.
    pub fn core(&self) -> &AgentCore {
        &self.core
    }

    /// The agent's body geometry.
    pub fn body(&self) -> &AgentBody {
        &self.body
    }

    /// The agent's motion family.
    pub fn motion(&self) -> AgentMotion {
        self.motion
    }

    /// The tactics the agent may select.
    pub fn tactics(&self) -> TacticalCapabilities {
        self.tactics
    }

    /// The agent's facility access.
    pub fn access(&self) -> &AgentAccess {
        &self.access
    }

    /// The agent's occupancy.
    pub fn occupancy(&self) -> &AgentOccupancy {
        &self.occupancy
    }

    /// The agent's social state.
    pub fn social(&self) -> SocialState {
        self.social
    }

    /// The body/motion family runtime behavior dispatches on.
    pub fn family(&self) -> AgentFamily {
        self.family
    }
}

/// Derive the dispatch family from a body envelope kind and a motion family.
///
/// Every family serves exactly one pair, so this is a total function over
/// composed agents and a rejection everywhere else. The mode-template compiler
/// uses it too, so the valid body/motion pairs have a single definition.
pub(crate) fn derive_family(
    body: BodyKind,
    motion: AgentMotion,
) -> Result<AgentFamily, ComponentMismatch> {
    match (body, motion) {
        (BodyKind::Circle, AgentMotion::HolonomicWalking) => Ok(AgentFamily::HolonomicCircle),
        (BodyKind::Box, AgentMotion::SingleBodyWheeled) => Ok(AgentFamily::WheeledBox),
        (BodyKind::Capsule, AgentMotion::SingleBodyWheeled) => Ok(AgentFamily::WheeledCapsule),
        (BodyKind::ArticulatedChain, AgentMotion::ArticulatedWheeled) => {
            Ok(AgentFamily::ArticulatedWheeled)
        }
        _ => Err(ComponentMismatch { body, motion }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range(min: f64, max: f64) -> ProfileRange {
        ProfileRange::new(min, max)
    }

    fn core_with(route: AgentRoute, profile: AgentBehaviorProfile) -> AgentCore {
        AgentCore::new(
            route,
            AgentPose::new(DVec2::new(0.0, 0.0), 0.0),
            AgentVelocity::new(DVec2::ZERO),
            AgentIntent::Travel,
            profile,
            AgentLifecycle::Active,
        )
    }

    fn walking_tactics() -> TacticalCapabilities {
        [
            TacticalCapability::Follow,
            TacticalCapability::Stop,
            TacticalCapability::Yield,
        ]
        .into_iter()
        .collect()
    }

    fn pedestrian_access() -> AgentAccess {
        AgentAccess::new(
            vec![
                FacilityKind::Path,
                FacilityKind::Crossing,
                FacilityKind::WaitingArea,
            ],
            NominalDirection::Either,
            SpeedPolicy::unlimited(),
            vec![RuleKind::Free],
        )
    }

    fn pedestrian_bundle() -> AgentComponents {
        AgentComponents::compose(
            core_with(
                AgentRoute::Pedestrian(PedestrianRouteId::from_index(0)),
                AgentBehaviorProfile::walking(range(1.0, 1.6), range(1.0, 1.0)),
            ),
            AgentBody::Circle {
                radius_m: range(0.20, 0.30),
            },
            AgentMotion::HolonomicWalking,
            walking_tactics(),
            pedestrian_access(),
            AgentOccupancy::OperatorOnly,
            SocialState::Individual,
        )
        .expect("a circle that walks is the holonomic-circle family")
    }

    fn car_bundle() -> AgentComponents {
        AgentComponents::compose(
            core_with(
                AgentRoute::Movement(MovementId::from_index(0)),
                AgentBehaviorProfile::wheeled(
                    range(9.0, 15.0),
                    range(1.0, 2.0),
                    range(1.2, 2.5),
                    range(2.0, 3.5),
                    range(1.0, 1.0),
                ),
            ),
            AgentBody::Box {
                length_m: range(4.0, 5.2),
                width_m: range(1.7, 2.0),
            },
            AgentMotion::SingleBodyWheeled,
            [
                TacticalCapability::Follow,
                TacticalCapability::Stop,
                TacticalCapability::Yield,
                TacticalCapability::ChangeLane,
                TacticalCapability::Overtake,
            ]
            .into_iter()
            .collect(),
            AgentAccess::new(
                vec![FacilityKind::Path],
                NominalDirection::Forward,
                SpeedPolicy::limited(13.4),
                vec![RuleKind::Yield, RuleKind::Signal],
            ),
            AgentOccupancy::Fixed { occupants: 2 },
            SocialState::Individual,
        )
        .expect("a box that steers is the wheeled-box family")
    }

    fn articulated_bundle() -> AgentComponents {
        AgentComponents::compose(
            core_with(
                AgentRoute::Movement(MovementId::from_index(1)),
                AgentBehaviorProfile::wheeled(
                    range(20.0, 24.0),
                    range(1.5, 2.5),
                    range(0.6, 1.1),
                    range(1.5, 2.5),
                    range(0.7, 0.9),
                ),
            ),
            AgentBody::ArticulatedChain {
                segments: vec![
                    BodySegment::new(range(5.0, 5.5), range(2.4, 2.5)),
                    BodySegment::new(range(12.0, 13.6), range(2.4, 2.6)),
                ],
            },
            AgentMotion::ArticulatedWheeled,
            [TacticalCapability::Follow, TacticalCapability::Overtake]
                .into_iter()
                .collect(),
            AgentAccess::new(
                vec![FacilityKind::Path],
                NominalDirection::Forward,
                SpeedPolicy::limited(22.0),
                vec![RuleKind::Yield, RuleKind::Signal],
            ),
            AgentOccupancy::OperatorOnly,
            SocialState::Individual,
        )
        .expect("a chain that steers is the articulated-wheeled family")
    }

    fn capsule_bundle() -> AgentComponents {
        AgentComponents::compose(
            core_with(
                AgentRoute::Movement(MovementId::from_index(2)),
                AgentBehaviorProfile::wheeled(
                    range(4.0, 6.0),
                    range(1.0, 1.5),
                    range(1.0, 2.0),
                    range(2.0, 3.0),
                    range(0.8, 1.0),
                ),
            ),
            AgentBody::Capsule {
                length_m: range(1.6, 1.9),
                radius_m: range(0.3, 0.4),
            },
            AgentMotion::SingleBodyWheeled,
            [TacticalCapability::Follow, TacticalCapability::Pass]
                .into_iter()
                .collect(),
            AgentAccess::new(
                vec![FacilityKind::Path],
                NominalDirection::Either,
                SpeedPolicy::unlimited(),
                vec![RuleKind::Yield],
            ),
            AgentOccupancy::OperatorOnly,
            SocialState::Individual,
        )
        .expect("a capsule that steers is the wheeled-capsule family")
    }

    #[test]
    fn composes_a_pedestrian_bundle_from_components() {
        let agent = pedestrian_bundle();
        assert_eq!(agent.family(), AgentFamily::HolonomicCircle);
        assert_eq!(agent.body().kind(), BodyKind::Circle);
        assert_eq!(agent.motion(), AgentMotion::HolonomicWalking);
        assert_eq!(agent.body().segment_count(), 1);
        assert!(agent.tactics().supports(TacticalCapability::Yield));
        assert!(agent.access().permits(FacilityKind::Crossing));
        assert_eq!(agent.occupancy(), &AgentOccupancy::OperatorOnly);
        assert_eq!(agent.social(), SocialState::Individual);
        assert_eq!(
            agent.core().route(),
            AgentRoute::Pedestrian(PedestrianRouteId::from_index(0))
        );
        assert_eq!(agent.core().lifecycle(), AgentLifecycle::Active);
        assert_eq!(agent.core().profile().time_gap_s(), None);
    }

    #[test]
    fn composes_a_passenger_car_bundle_from_components() {
        let agent = car_bundle();
        assert_eq!(agent.family(), AgentFamily::WheeledBox);
        assert_eq!(agent.body().kind(), BodyKind::Box);
        assert_eq!(agent.motion(), AgentMotion::SingleBodyWheeled);
        assert_eq!(
            agent.core().route(),
            AgentRoute::Movement(MovementId::from_index(0))
        );
        assert!(agent.tactics().supports(TacticalCapability::Overtake));
        assert!(!agent.tactics().supports(TacticalCapability::Pass));
        assert!(agent.access().permits(FacilityKind::Path));
        assert!(!agent.access().permits(FacilityKind::WaitingArea));
        assert!(agent.access().subject_to(RuleKind::Signal));
        assert_eq!(
            agent.access().nominal_direction(),
            NominalDirection::Forward
        );
        assert_eq!(agent.access().speed_policy().limit_mps(), Some(13.4));
        assert_eq!(agent.occupancy(), &AgentOccupancy::Fixed { occupants: 2 });
        assert_eq!(
            agent
                .core()
                .profile()
                .max_accel_mps2()
                .map(ProfileRange::min),
            Some(1.2)
        );
    }

    #[test]
    fn composes_an_articulated_bundle_from_ordered_segments() {
        let agent = articulated_bundle();
        assert_eq!(agent.family(), AgentFamily::ArticulatedWheeled);
        assert_eq!(agent.body().kind(), BodyKind::ArticulatedChain);
        assert_eq!(agent.motion(), AgentMotion::ArticulatedWheeled);
        assert_eq!(agent.body().segment_count(), 2);
        let AgentBody::ArticulatedChain { segments } = agent.body() else {
            panic!("the articulated bundle carries a chain body");
        };
        assert!((segments[0].length_m().min() - 5.0).abs() < 1e-9);
        assert!((segments[1].length_m().max() - 13.6).abs() < 1e-9);
    }

    #[test]
    fn derives_a_capsule_family_from_components() {
        assert_eq!(capsule_bundle().family(), AgentFamily::WheeledCapsule);
    }

    #[test]
    fn the_narrow_wheeled_profile_carries_steering_and_clearance() {
        let narrow = AgentBehaviorProfile::narrow_wheeled(
            range(3.5, 6.5),
            range(0.8, 1.4),
            range(0.8, 1.5),
            range(1.5, 3.0),
            range(0.6, 1.2),
            range(0.20, 0.50),
            range(0.8, 1.0),
        );
        assert_eq!(narrow.steering_rate_max_rad_s(), Some(range(0.6, 1.2)));
        assert_eq!(narrow.lateral_clearance_m(), Some(range(0.20, 0.50)));
        assert_eq!(narrow.time_gap_s(), Some(range(0.8, 1.4)));

        // The Increment 0 wheeled and walking profiles carry neither parameter,
        // so their layout and values are unchanged.
        let wheeled = AgentBehaviorProfile::wheeled(
            range(9.0, 15.0),
            range(1.0, 2.0),
            range(1.2, 2.5),
            range(2.0, 3.5),
            range(1.0, 1.0),
        );
        assert_eq!(wheeled.steering_rate_max_rad_s(), None);
        assert_eq!(wheeled.lateral_clearance_m(), None);
        assert_eq!(
            AgentBehaviorProfile::walking(range(1.0, 1.6), range(1.0, 1.0)).lateral_clearance_m(),
            None
        );
    }

    #[test]
    fn rejects_a_body_and_motion_pair_no_family_serves() {
        let circle_steering = AgentComponents::compose(
            core_with(
                AgentRoute::Movement(MovementId::from_index(0)),
                AgentBehaviorProfile::walking(range(1.0, 1.6), range(1.0, 1.0)),
            ),
            AgentBody::Circle {
                radius_m: range(0.2, 0.3),
            },
            AgentMotion::SingleBodyWheeled,
            walking_tactics(),
            pedestrian_access(),
            AgentOccupancy::OperatorOnly,
            SocialState::Individual,
        )
        .expect_err("a circle cannot steer as a single rigid wheeled body");
        assert_eq!(circle_steering.body(), BodyKind::Circle);
        assert_eq!(circle_steering.motion(), AgentMotion::SingleBodyWheeled);
        assert!(circle_steering.to_string().contains("no agent family"));

        // Articulation parameters on a holonomic body are impossible too.
        let walking_chain = AgentComponents::compose(
            core_with(
                AgentRoute::Movement(MovementId::from_index(0)),
                AgentBehaviorProfile::walking(range(1.0, 1.6), range(1.0, 1.0)),
            ),
            AgentBody::ArticulatedChain {
                segments: vec![BodySegment::new(range(5.0, 5.5), range(2.4, 2.5))],
            },
            AgentMotion::HolonomicWalking,
            TacticalCapabilities::none(),
            pedestrian_access(),
            AgentOccupancy::OperatorOnly,
            SocialState::Individual,
        )
        .expect_err("an articulated chain cannot walk holonomically");
        assert_eq!(walking_chain.body(), BodyKind::ArticulatedChain);
        assert_eq!(walking_chain.motion(), AgentMotion::HolonomicWalking);
    }

    #[test]
    fn the_family_is_derived_only_from_body_and_motion() {
        // Same body and motion, different route, tactics, access, occupancy,
        // profile, and social state: the family does not move.
        let first = pedestrian_bundle();
        let second = AgentComponents::compose(
            AgentCore::new(
                AgentRoute::Pedestrian(PedestrianRouteId::from_index(7)),
                AgentPose::new(DVec2::new(12.0, -3.0), 1.25),
                AgentVelocity::new(DVec2::new(-1.1, 0.4)),
                AgentIntent::Tactic(TacticalCapability::Yield),
                AgentBehaviorProfile::walking(range(0.8, 1.2), range(0.2, 0.6)),
                AgentLifecycle::Arrived,
            ),
            AgentBody::Circle {
                radius_m: range(0.25, 0.35),
            },
            AgentMotion::HolonomicWalking,
            [
                TacticalCapability::Follow,
                TacticalCapability::ReverseNominalDirection,
            ]
            .into_iter()
            .collect(),
            AgentAccess::new(
                vec![FacilityKind::Path],
                NominalDirection::Forward,
                SpeedPolicy::limited(1.5),
                vec![RuleKind::Stop],
            ),
            AgentOccupancy::OperatorOnly,
            SocialState::GroupMember(GroupMembership::new(
                PedestrianGroupId::from_index(3),
                GroupRole::Leader,
            )),
        )
        .expect("the same body and motion compose");
        assert_eq!(first.family(), second.family());
        assert_eq!(second.family(), AgentFamily::HolonomicCircle);
        // Everything except body and motion differs between the two bundles.
        assert_ne!(first.core(), second.core());
        assert_ne!(first.tactics(), second.tactics());
        assert_ne!(first.access(), second.access());
        assert_ne!(first.social(), second.social());
    }

    #[test]
    fn every_family_serves_exactly_one_body_and_motion_pair() {
        let families: Vec<AgentFamily> = [
            pedestrian_bundle(),
            car_bundle(),
            capsule_bundle(),
            articulated_bundle(),
        ]
        .into_iter()
        .map(|agent| agent.family())
        .collect();
        assert_eq!(families, AgentFamily::ALL);
    }

    #[test]
    fn capabilities_are_a_set_that_iterates_in_declaration_order() {
        assert!(TacticalCapabilities::none().is_empty());
        let tactics: TacticalCapabilities = [
            TacticalCapability::Overtake,
            TacticalCapability::Follow,
            TacticalCapability::Overtake,
        ]
        .into_iter()
        .collect();
        assert!(!tactics.is_empty());
        assert!(tactics.supports(TacticalCapability::Follow));
        assert!(!tactics.supports(TacticalCapability::ServeStop));
        assert_eq!(
            tactics.iter().collect::<Vec<_>>(),
            [TacticalCapability::Follow, TacticalCapability::Overtake]
        );
        assert_eq!(TacticalCapabilities::none().iter().collect::<Vec<_>>(), []);
    }

    #[test]
    fn transit_occupancy_boards_and_alights_within_capacity() {
        let mut occupancy = AgentOccupancy::Transit(TransitOccupancy::new(60, 0));
        let AgentOccupancy::Transit(transit) = &mut occupancy else {
            panic!("a transit agent carries transit occupancy");
        };
        assert_eq!(transit.capacity(), 60);
        assert_eq!(transit.spare_capacity(), 60);
        assert_eq!(transit.board(45), 0);
        assert_eq!(transit.onboard(), 45);
        assert_eq!(transit.board(30), 15, "boarding beyond capacity is denied");
        assert_eq!(transit.onboard(), 60);
        assert_eq!(transit.alight(25), 25);
        assert_eq!(transit.spare_capacity(), 25);
        assert_eq!(transit.alight(100), 35, "alighting is bounded onboard");
        assert_eq!(transit.onboard(), 0);
        // Onboard is clamped to capacity when composing an occupied vehicle.
        assert_eq!(TransitOccupancy::new(4, 9).onboard(), 4);
    }

    #[test]
    fn social_state_carries_group_membership_and_role() {
        let membership = GroupMembership::new(PedestrianGroupId::from_index(2), GroupRole::Member);
        assert_eq!(membership.group(), PedestrianGroupId::from_index(2));
        assert_eq!(membership.role(), GroupRole::Member);
        assert_ne!(
            SocialState::Individual,
            SocialState::GroupMember(membership)
        );
    }

    #[test]
    fn labels_name_each_body_motion_and_family() {
        assert_eq!(BodyKind::Circle.label(), "circle");
        assert_eq!(BodyKind::ArticulatedChain.label(), "articulated_chain");
        assert_eq!(AgentMotion::HolonomicWalking.label(), "holonomic_walking");
        assert_eq!(
            AgentMotion::ArticulatedWheeled.label(),
            "articulated_wheeled"
        );
        assert_eq!(AgentFamily::WheeledBox.label(), "wheeled_box");
        for family in AgentFamily::ALL {
            assert!(!family.label().is_empty());
        }
    }
}
