---
context_rev: 1
priority: P1
updated: 2026-09-15T00:14:35Z
summary: Record the wrong-way interval and rule-state trajectory.
next: [[TAS-144-add-the-wrong-way-rule-state-to-trajectories-and]]
---

Parent [[TAS-102-record-wrong-way-intervals-and-disaggregated-metrics]].

# Outcome

Wrong-way travel is visible as one reasoned violation interval bounded by the
rule geometry and as optional rule state on sampled trajectories.

# Done when

- The interval opens and closes at TAS-083 geometric and rule boundaries and
  records perceived rule, decision reason, facility, and affected movement IDs;
  rejected decisions create no false travel interval.
- Sampled trajectories expose optional rule state and opposing direction; sparse
  transitions remain event records and survive immutable run and replay.
- Focused tests cover clear interval, abort-before-entry, facility handoff, route
  exit, replay, and inapplicable modes.

# Context

Gated on [[TAS-098-route-wrong-way-agents-through-ordinary-interactions]].
Gated on [[TAS-100-version-the-maneuver-event-and-trace-surface]].
Extends [[TAS-102-record-wrong-way-intervals-and-disaggregated-metrics]]; reads
[[THO-016-increment-2-event-metric-trajectory-presenter-an]]. Owns the interval
and trajectory rule state; do not add the aggregate metrics.

# Result

Landed the interval half. The trajectory and snapshot rule-state columns are the
remaining scope, split to
[[TAS-144-add-the-wrong-way-rule-state-to-trajectories-and]].

- `crates/tangle-sim/src/wrong_way.rs` (`OpposingTraversalTracker`,
  `OpposingTraversalObservation`, `traversal_record`, `opposing_reason`): the
  interval tracker, a pure observer of the integrated state and the compiled
  scenario like `crate::close_pass::ClosePassTracker`. An interval opens on the
  first observed step where the body centre lies inside its object's compiled
  reference extent `[0, length]` and its traversal direction is against the
  object's authored nominal direction; it closes on the first later step that
  leaves the extent, leaves the rule direction, enters a different object (a
  lateral or connector handoff), or despawns, and
  `OpposingTraversalTracker::close_open` closes a still-open interval at the
  run's final simulation time. Nominal travel is not counted at all, so a
  rejected decision creates no interval. `violating` is false exactly when the
  perceived rule is `permit` or `obligate`, so a legal opposing traversal is
  recorded under its rule and never as a violation. The record's perceived rule,
  reason code, and affected movement are the decision's own when the traversal
  is the opposing option a recorded decision selected, and are otherwise derived
  from the compiled policy through the same `opposing_reason` mapping `decide`
  uses, so a traversal the decision did not select still carries an inspectable
  reason.
- `crates/tangle-sim/src/agent.rs` (`RouteState::wrong_way_decision`): the most
  recent decision the agent evaluated, so the interval presents the decision's
  facts rather than re-deriving them. The wrong-way entry leaves the movement
  route behind (`Simulation::enter_opposing_traversal`), so the decision is the
  only surviving spelling of the connector the agent entered on and the record
  can name the affected movement.
- `crates/tangle-sim/src/sim.rs` (`Simulation::opposing_traversals`,
  `Simulation::opposing_traversal_event`, `Simulation::resolve_wrong_way_entries`,
  `Simulation::opposing_traversal_tracker`,
  `Simulation::close_open_opposing_traversals`): the tracker observes the
  integrated tick beside the close-pass tracker, and the kernel emits exactly
  one `Event::OpposingTraversal` at the open boundary and one at the close
  boundary from the latched record. `OpposingTraversal` was already defined and
  already serialized by `apps/tangle-cli/src/trace.rs`, so no event, version, or
  trace change was needed.

Evidence, all in this worktree: `cargo test -p tangle-sim` green (all suites;
`--lib` 243 passed, `--test wrong_way` 19 passed with 7 new tests);
`cargo test -p tangle-cli` green (including `golden_trace`, which guards the
Phase 1 trace hashes — no Phase 1 golden changed); `cargo clippy --workspace
--all-targets --all-features -- -D warnings` clean; `cargo fmt --all --check`
clean; `scripts/check-dependency-direction.sh` OK; `braintree check` passes.

New focused tests: clear interval with both boundaries and the record's
perceived rule, reason, facility, and affected movement
(`the_interval_opens_at_the_entry_and_closes_at_the_object_boundary`); the
handoff tick is both the source close and the destination open in the same test;
rejected decision (`a_rejected_decision_creates_no_interval`); route exit
(`the_interval_opens_at_the_entry_and_closes_at_the_object_boundary`); replay
(`the_interval_boundaries_survive_replay`); inapplicable mode and a permitted
(legal) opposing interval; the run-end close; and in-crate boundary tests for a
body centre outside the extent, an `either` object with no rule direction, and a
direction change closing and reopening the interval
(`crates/tangle-sim/src/wrong_way.rs`, `mod interval_tests`).

One pre-existing test changed: `a_connector_handoff_emits_one_facility_transition_event_mapping_its_record`
(`crates/tangle-sim/tests/wrong_way.rs`) asserted that no event at all followed a
rider's handoff within one step. That premise is falsified by the new interval
boundaries, so the assertion now checks the documented kind order the test
exists to pin (the rider's records are non-decreasing by `EventKind::order`).

Open reading recorded, not resolved: the contract's interval boundary says the
rule direction is the object's nominal direction "unless an `obligate` statement
applies, in which case travelling *with* it is the violation", while the same
section says "a permitted or obligated opposing traversal opens a *legal*
opposing interval", and it also says "nominal travel is not counted at all".
Those three cannot all hold for an `obligate` object. This slice implements the
nominal-relative reading, which the metric families ("a legal opposing traversal
is reported under its rule and never as a violation") and "nominal travel is not
counted at all" both support, and which keeps the recorded `reason` exactly the
decision's own code; the obligate flip is therefore unrepresented. No required
test depends on it, and the fix is local to `traversal_record` if the contract
is read the other way.
