---
context_rev: 1
priority: P1
updated: 2026-09-13T15:54:33Z
summary: The tangle-model component model composes each agent's core with body, motion, tactics, access, occupancy, and social state, and derives a small body/motion family for dispatch.
---

Parent [[TAS-058-agent-components-and-controller-stages]].

# Outcome

`tangle-model` compiled data carries each simulated road user as a compact core
plus composable optional components: body (circle, oriented box, capsule, or
ordered articulated chain), motion (holonomic walking, single-body wheeled, or
articulated wheeled), tactical capabilities, access, occupancy, and social
state.

# Done when

- The compiled representation defines each component from the agent-composition section of `PHASE_2_PLAN.md`.
- Components compose without a mode name in the kernel and dispatch on a small number of body/motion families.
- `cargo test -p tangle-model` passes with the existing Phase 1 compilation unchanged.

# Context

No gate; starts the component stream of [[TAS-058-agent-components-and-controller-stages]].

# Result

`tangle-model` now carries the compiled agent component model, additive to the
compiled scenario. Three `Done when` criteria hold.

- **Every agent-composition component is defined.**
  `crates/tangle-model/src/components.rs` (new) defines the bundle
  `AgentComponents` as a compact `AgentCore` plus six composable components:
  `AgentBody` (`Circle` / `Box` / `Capsule` / `ArticulatedChain` of ordered
  `BodySegment`s), `AgentMotion` (`HolonomicWalking` / `SingleBodyWheeled` /
  `ArticulatedWheeled`), `TacticalCapabilities` (a compact bit set over
  `TacticalCapability::{Follow, Stop, Yield, ChooseLateralPosition, ChangeLane,
  Overtake, Pass, ReverseNominalDirection, ServeStop}`), `AgentAccess`
  (facility kinds, `NominalDirection`, `SpeedPolicy`, rule kinds),
  `AgentOccupancy` (`OperatorOnly` / `Fixed` / `Transit(TransitOccupancy)` with
  capacity and aggregate onboard/boarding state), and `SocialState`
  (`Individual` / `GroupMember(GroupMembership)` naming a `PedestrianGroupId`
  and a `GroupRole`). The core carries `AgentRoute`, `AgentPose`,
  `AgentVelocity`, `AgentIntent`, `AgentBehaviorProfile`, and `AgentLifecycle`.
- **Composition without a mode name.** `AgentComponents::compose` takes no id,
  name, or template argument and derives `AgentFamily` (`HolonomicCircle`,
  `WheeledBox`, `WheeledCapsule`, `ArticulatedWheeled`) from the body kind and
  the motion family alone; the family is stored once and `family()` is total
  afterwards. A body/motion pair no family serves is rejected with
  `ComponentMismatch` naming both parts, so the two impossible combinations in
  `PHASE_2_PLAN.md` (a circle with single-body wheeled steering, articulation on
  a holonomic body) cannot compose.
- **Existing compilation unchanged.** `CompiledScenario` and every version-1 and
  version-2 compiled field are untouched; the only change outside the new module
  is the additive `ProfileRange::new` constructor the body distributions use and
  the `lib.rs` exports.

Evidence: `cargo test -p tangle-model` passes (69 unit + 4 new integration tests
in `crates/tangle-model/tests/agent_components.rs`; the integration tests compose
pedestrian, passenger-car, capsule, and articulated bundles through the public
exports and assert the derived family); `cargo test --workspace` passes;
`cargo clippy --workspace --all-targets --all-features` is clean;
`cargo fmt --all` applied; `scripts/check-dependency-direction.sh` reports
`dependency direction OK`.

Limitation carried to [[TAS-067-mode-template-compilation]]: the compiled kinds
for capsules, articulated chains, articulated wheeled motion, transit occupancy,
the extended tactics, and social state have no authored counterpart yet, so
compiling authored mode templates into these bundles — and mapping a rejection
to a stable diagnostic — is that node's scope.
