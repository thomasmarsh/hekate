---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Define contextual wrong-way decision inputs and reason-coded output.
---

Parent [[TAS-097-make-contextual-wrong-way-decisions-reproducible]].

# Outcome

An eligible narrow agent has one documented, reason-coded decision surface for
opposing traversal, built only from the observer-stage context the contract fixes,
so a consumer can read why the agent chose nominal or opposing travel.

# Done when

- The decision reads only the observer-stage context fixed by TAS-083 and adds no
  detailed visibility-error subsystem.
- Output contains perceived permission or obligation, the selected nominal or
  opposing option, reason code, affected movement IDs, and the context values
  used.
- Table-driven tests cover each reason and each threshold tie.

# Context

Extends [[TAS-097-make-contextual-wrong-way-decisions-reproducible]]; reads the
compiled surfaces in [[THO-015-increment-2-compiled-contract-and-model-seams-sc]].
Owns the decision input and output shape; do not add the random draw or motion.

# Result

Landed `crates/hekate-sim/src/wrong_way.rs` (new, `pub mod wrong_way` plus
re-exports in `crates/hekate-sim/src/lib.rs`):

- `WrongWayReason`: the closed set with `ALL` and the contract's stable
  lowercase `label()` codes; `WrongWayOption` `Nominal`/`Opposing`.
- `WrongWayInputs`: exactly the contract's decision-instant inputs — opposing
  physical connectivity, the nominal `NominalDirection` (must not be `Either`),
  nominal and opposing remaining lengths and expected speeds, observed opposing
  density per km, the applicable `Option<PermissionEffect>`, `compliance`,
  `desired_speed_mps`, the `WrongWayPolicySource` thresholds, and the affected
  `FacilityId` plus `Option<MovementId>`. `time_saving_s()` is the contract's own
  formula. `desired_speed_mps` carries no term of its own: the caller caps it by
  the effective facility limit to produce the two expected speeds.
- `WrongWayDecision`: perceived rule, selected option, reason, facility and
  optional movement, and the context values read (`time_saving_s`,
  `opposing_density_per_km`, `urgency`, `compliance`).
- `decide(inputs, draw)`: total and deterministic in the contract's order —
  disconnected opposing path → `no_opposing_path` (checked before
  `no_nominal_direction`, so an `either` object that is also disconnected names
  the path); saving strictly below `min_time_saving_s` →
  `insufficient_time_saving`, checked before density strictly above
  `max_opposing_density_per_km` → `opposing_density_too_high`; both rejections
  take no draw and select nominal. Otherwise `u < urgency * (1 - compliance)`
  selects opposing, an exact tie selects nominal with `compliant_choice`, and a
  selected opposing option is `legal_permission` / `legal_obligation` /
  `noncompliant_choice` from `permit` / `obligate` / absent-or-`prohibit`.
  Rejections record the context in force so an inspector sees what produced the
  code. No perception, motion, routing, event, or draw-keying code was added.

Evidence: `cargo test -p hekate-sim --lib wrong_way` 15 passed (each reason,
both thresholds inclusive at the authored value and rejecting just past it, the
`u == urgency * (1 - compliance)` tie, a fully compliant agent under every
urgency, and a zero-urgency agent); `cargo test -p hekate-sim --lib` 207 passed;
`cargo clippy --workspace --all-targets --all-features -- -D warnings` clean;
`cargo fmt --all --check` clean; `scripts/check-dependency-direction.sh` OK;
`tangle check` passes.

Remaining scope owned elsewhere: the keyed draw and any impossible-option
rejection before a claim belong to
[[TAS-116-key-the-wrong-way-draw-and-reject-impossible-opp]]; execution and
events belong to [[TAS-098-route-wrong-way-agents-through-ordinary-interactions]]
and [[TAS-099-increment-2-events-metrics-and-output]].
