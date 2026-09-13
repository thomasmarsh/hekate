//! Compile authored mode templates into agent component bundles.
//!
//! [`ModeTemplateSource`] is the authored, named bundle of body, motion,
//! tactical capabilities, access, occupancy, and profile distributions.
//! [`compile_mode_template`] is the pure compiler that turns one template into
//! a [`CompiledModeTemplate`]: the same components expressed with the TAS-066
//! [`crate::components`] types, plus the template id.
//!
//! The compiler maps each authored field onto its component and derives the
//! body/motion [`AgentFamily`] from the compiled body and motion alone, exactly
//! as [`crate::AgentComponents::compose`] does. It never reads the template id,
//! so no named mode can take a branch shared code does not. A template whose
//! components cannot coexist — transit occupancy or stop service (dwell)
//! without a passenger capacity, or articulation parameters on a holonomic
//! body — is rejected by [`CompiledModeTemplate::validate`] with a stable
//! diagnostic naming the authored template id.
//!
//! Mode templates are per-mode data. Per-agent runtime state — the core's
//! route, pose, velocity, intent, and lifecycle, and the social state — is not
//! authored here; [`CompiledModeTemplate::compose`] combines a template with
//! that state at spawn, so the template compiler never fabricates it.

use crate::compiled::ProfileRange;
use crate::components::{
    AgentAccess, AgentBehaviorProfile, AgentBody, AgentComponents, AgentCore, AgentFamily,
    AgentMotion, AgentOccupancy, ComponentMismatch, NominalDirection, SocialState, SpeedPolicy,
    TacticalCapabilities, TacticalCapability, derive_family,
};
use crate::source::{
    AccessSource, FacilityDirection, ModeBodySource, ModeTemplateSource, MotionKind, OccupancyKind,
    ProfileRangeSource, SpeedPolicySource, TacticKind,
};
use crate::validate::{Diagnostic, DiagnosticCode, required_profile_params};

/// A mode template compiled into its agent components.
///
/// The bundle carries the template id alongside the six components the authored
/// template maps onto: [`AgentBody`], [`AgentMotion`], [`TacticalCapabilities`],
/// [`AgentAccess`], [`AgentOccupancy`], and [`AgentBehaviorProfile`], plus the
/// [`AgentFamily`] derived from the body and motion when one serves the pair.
///
/// A bundle built directly with [`Self::new`] may carry an impossible
/// combination; [`Self::validate`] rejects it. A bundle returned by
/// [`compile_mode_template`] has already been validated.
#[derive(Debug, Clone, PartialEq)]
pub struct CompiledModeTemplate {
    id: String,
    body: AgentBody,
    motion: AgentMotion,
    tactics: TacticalCapabilities,
    access: AgentAccess,
    occupancy: AgentOccupancy,
    profile: AgentBehaviorProfile,
    family: Option<AgentFamily>,
}

impl CompiledModeTemplate {
    /// Compose a bundle from a template id and its compiled components.
    ///
    /// The body/motion family is derived here and is `None` for a pair no
    /// family serves; [`Self::validate`] reports that, and [`Self::compose`]
    /// rejects it. No id, name, or template participates in the derivation.
    pub fn new(
        id: String,
        body: AgentBody,
        motion: AgentMotion,
        tactics: TacticalCapabilities,
        access: AgentAccess,
        occupancy: AgentOccupancy,
        profile: AgentBehaviorProfile,
    ) -> Self {
        let family = derive_family(body.kind(), motion).ok();
        Self {
            id,
            body,
            motion,
            tactics,
            access,
            occupancy,
            profile,
            family,
        }
    }

    /// The authored template id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The compiled body geometry.
    pub fn body(&self) -> &AgentBody {
        &self.body
    }

    /// The compiled motion family.
    pub fn motion(&self) -> AgentMotion {
        self.motion
    }

    /// The compiled tactical capabilities.
    pub fn tactics(&self) -> TacticalCapabilities {
        self.tactics
    }

    /// The compiled facility access.
    pub fn access(&self) -> &AgentAccess {
        &self.access
    }

    /// The compiled occupancy.
    pub fn occupancy(&self) -> &AgentOccupancy {
        &self.occupancy
    }

    /// The compiled behavior profile.
    pub fn profile(&self) -> &AgentBehaviorProfile {
        &self.profile
    }

    /// The body/motion family shared kernel behavior dispatches on, or `None`
    /// when no family serves the pair.
    pub fn family(&self) -> Option<AgentFamily> {
        self.family
    }

    /// Every impossible combination this bundle carries, in a stable order.
    ///
    /// An empty result means the components can coexist. A body and motion pair
    /// no [`AgentFamily`] serves is reported as `E_MODE_TEMPLATE_BODY_MOTION`;
    /// transit occupancy or stop service (dwell) without a passenger capacity
    /// is reported as `E_MODE_TEMPLATE_OCCUPANCY`. Every diagnostic names the
    /// authored template id.
    pub fn validate(&self) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        if self.family.is_none() {
            diagnostics.push(Diagnostic {
                code: DiagnosticCode::ModeTemplateBodyMotion,
                object: Some(self.id.clone()),
                message: format!(
                    "mode template '{}' pairs a {} body with {} motion, which no agent \
                     family serves",
                    self.id,
                    self.body.kind().label(),
                    self.motion.label()
                ),
            });
        }

        let transit_capacity = match self.occupancy {
            AgentOccupancy::Transit(transit) => Some(transit.capacity()),
            AgentOccupancy::OperatorOnly | AgentOccupancy::Fixed { .. } => None,
        };
        if transit_capacity == Some(0) {
            diagnostics.push(Diagnostic {
                code: DiagnosticCode::ModeTemplateOccupancy,
                object: Some(self.id.clone()),
                message: format!(
                    "mode template '{}' declares transit occupancy without a passenger \
                     capacity",
                    self.id
                ),
            });
        } else if transit_capacity.is_none() && self.tactics.supports(TacticalCapability::ServeStop)
        {
            diagnostics.push(Diagnostic {
                code: DiagnosticCode::ModeTemplateOccupancy,
                object: Some(self.id.clone()),
                message: format!(
                    "mode template '{}' serves stops (dwell) without a transit capacity",
                    self.id
                ),
            });
        }

        diagnostics
    }

    /// Combine this template with one agent's core and social state.
    ///
    /// The template supplies the body, motion, tactics, access, occupancy, and
    /// behavior profile; `core` and `social` are the per-agent runtime state a
    /// template does not author. Delegates to [`AgentComponents::compose`], so a
    /// body and motion pair no family serves is rejected as
    /// [`ComponentMismatch`].
    pub fn compose(
        &self,
        core: AgentCore,
        social: SocialState,
    ) -> Result<AgentComponents, ComponentMismatch> {
        AgentComponents::compose(
            core,
            self.body.clone(),
            self.motion,
            self.tactics,
            self.access.clone(),
            self.occupancy,
            social,
        )
    }
}

/// Compile a mode template into its agent component bundle.
///
/// The function reads only the authored fields; it never branches on the
/// template id. Every impossible combination the compiled bundle carries is
/// returned as a stable diagnostic naming the template id, so a caller never
/// receives a bundle whose components cannot coexist. A missing profile the
/// motion family requires is reported the same way, before any bundle is built.
pub fn compile_mode_template(
    template: &ModeTemplateSource,
) -> Result<CompiledModeTemplate, Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();
    let body = compiled_body(&template.body);
    let motion = compiled_motion(template.motion);
    let tactics = template
        .tactics
        .iter()
        .copied()
        .map(compiled_tactic)
        .collect();
    let access = compiled_access(&template.access);
    let occupancy = compiled_occupancy(template.occupancy);
    let Some(profile) = compiled_profile(template, &mut diagnostics) else {
        return Err(diagnostics);
    };

    let compiled = CompiledModeTemplate::new(
        template.id.clone(),
        body,
        motion,
        tactics,
        access,
        occupancy,
        profile,
    );
    diagnostics.extend(compiled.validate());
    if diagnostics.is_empty() {
        Ok(compiled)
    } else {
        Err(diagnostics)
    }
}

/// Whether a body and motion pair a version-2 template selects is one an
/// [`AgentFamily`] serves.
///
/// Source validation and the template compiler share this, so the set of valid
/// pairs has a single definition: a new body or motion shape only needs its
/// mapping compiled, not a second rule.
pub(crate) fn body_motion_pair_has_family(body: &ModeBodySource, motion: MotionKind) -> bool {
    derive_family(compiled_body(body).kind(), compiled_motion(motion)).is_ok()
}

/// Map an authored body onto its compiled body component.
pub(crate) fn compiled_body(body: &ModeBodySource) -> AgentBody {
    match body {
        ModeBodySource::Box { length_m, width_m } => AgentBody::Box {
            length_m: range(*length_m),
            width_m: range(*width_m),
        },
        ModeBodySource::Circle { radius_m } => AgentBody::Circle {
            radius_m: range(*radius_m),
        },
        ModeBodySource::Capsule { length_m, radius_m } => AgentBody::Capsule {
            length_m: range(*length_m),
            radius_m: range(*radius_m),
        },
    }
}

/// Map an authored motion family onto its compiled motion component.
pub(crate) fn compiled_motion(motion: MotionKind) -> AgentMotion {
    match motion {
        MotionKind::HolonomicWalking => AgentMotion::HolonomicWalking,
        MotionKind::SingleBodyWheeled => AgentMotion::SingleBodyWheeled,
    }
}

/// Map an authored tactic onto its compiled tactical capability.
fn compiled_tactic(tactic: TacticKind) -> TacticalCapability {
    match tactic {
        TacticKind::Follow => TacticalCapability::Follow,
        TacticKind::Stop => TacticalCapability::Stop,
        TacticKind::Yield => TacticalCapability::Yield,
    }
}

/// Map an authored access onto its compiled access component.
///
/// Increment 0 fixed the compiled direction as `either` and the speed policy as
/// unlimited because nothing authored them; the Increment 1 `nominal_direction`
/// and `speed_policy` fields, when present, override those values. An omitted
/// field keeps the Increment 0 meaning, so an Increment 0 template compiles
/// unchanged. Rule kinds are still deferred, so a compiled template carries
/// none.
fn compiled_access(access: &AccessSource) -> AgentAccess {
    AgentAccess::new(
        access.facility_kinds.clone(),
        access
            .nominal_direction
            .map(compiled_nominal_direction)
            .unwrap_or(NominalDirection::Either),
        access
            .speed_policy
            .map(compiled_speed_policy)
            .unwrap_or_else(SpeedPolicy::unlimited),
        Vec::new(),
    )
}

/// Map an authored facility direction onto its compiled access direction.
pub(crate) fn compiled_nominal_direction(direction: FacilityDirection) -> NominalDirection {
    match direction {
        FacilityDirection::Forward => NominalDirection::Forward,
        FacilityDirection::Reverse => NominalDirection::Reverse,
        FacilityDirection::Either => NominalDirection::Either,
    }
}

/// Map an authored speed policy onto its compiled access policy.
pub(crate) fn compiled_speed_policy(policy: SpeedPolicySource) -> SpeedPolicy {
    match policy.limit_mps.value() {
        Some(limit_mps) => SpeedPolicy::limited(limit_mps),
        None => SpeedPolicy::unlimited(),
    }
}

/// Map an authored occupancy onto its compiled occupancy component.
fn compiled_occupancy(occupancy: OccupancyKind) -> AgentOccupancy {
    match occupancy {
        OccupancyKind::OperatorOnly => AgentOccupancy::OperatorOnly,
    }
}

/// Build the behavior profile from the profile map for the body/motion family.
///
/// Every parameter the family requires is read by name; a missing one is
/// reported as `E_MODE_TEMPLATE_PROFILE` and returns `None`, so no bundle is
/// built from an incomplete profile map. The narrow wheeled family (a capsule
/// body that steers) carries its steering and lateral-clearance parameters in
/// addition to the Increment 0 wheeled set; the selection is keyed on the
/// derived body/motion pair, exactly as [`required_profile_params`] is, never on
/// the template id.
fn compiled_profile(
    template: &ModeTemplateSource,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<AgentBehaviorProfile> {
    let mut missing = false;
    for name in required_profile_params(&template.body, template.motion) {
        if !template.profiles.contains_key(*name) {
            missing = true;
            diagnostics.push(Diagnostic {
                code: DiagnosticCode::ModeTemplateProfile,
                object: Some(template.id.clone()),
                message: format!(
                    "mode template '{}' must declare profile '{name}' for its motion family",
                    template.id
                ),
            });
        }
    }
    if missing {
        return None;
    }

    let param = |name: &str| {
        let source = template
            .profiles
            .get(name)
            .expect("a required profile is present after the presence check");
        ProfileRange::new(source.min, source.max)
    };
    Some(match (&template.body, template.motion) {
        (ModeBodySource::Capsule { .. }, MotionKind::SingleBodyWheeled) => {
            AgentBehaviorProfile::narrow_wheeled(
                param("speed_mps"),
                param("time_gap_s"),
                param("max_accel_mps2"),
                param("comfortable_brake_mps2"),
                param("steering_rate_max_rad_s"),
                param("lateral_clearance_m"),
                param("compliance"),
            )
        }
        (_, MotionKind::HolonomicWalking) => {
            AgentBehaviorProfile::walking(param("speed_mps"), param("compliance"))
        }
        (_, MotionKind::SingleBodyWheeled) => AgentBehaviorProfile::wheeled(
            param("speed_mps"),
            param("time_gap_s"),
            param("max_accel_mps2"),
            param("comfortable_brake_mps2"),
            param("compliance"),
        ),
    })
}

/// Convert an authored inclusive range into its compiled form.
fn range(source: ProfileRangeSource) -> ProfileRange {
    ProfileRange::new(source.min, source.max)
}
