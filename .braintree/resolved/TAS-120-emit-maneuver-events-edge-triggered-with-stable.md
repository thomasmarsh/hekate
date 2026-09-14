---
context_rev: 1
priority: P1
updated: 2026-09-14T17:30:57Z
summary: Emit maneuver events edge-triggered with stable ordering.
---

Parent [[TAS-100-version-the-maneuver-event-and-trace-surface]].

# Outcome

Maneuver and rule events are emitted edge-triggered from existing state changes
with explicit stable ordering, so no transition is duplicated or reordered.

# Done when

- Emission is edge-triggered from state changes already owned by the maneuver and
  wrong-way stages; retries cannot duplicate a transition.
- Event ordering has explicit stable keys and is invariant to declaration or
  candidate insertion order.
- Lifecycle, ordering, replay, and old-fixture regression tests pass.

# Context

Gated on [[TAS-095-complete-lane-transitions-and-safe-aborts]].
Gated on [[TAS-098-route-wrong-way-agents-through-ordinary-interactions]].
Extends [[TAS-100-version-the-maneuver-event-and-trace-surface]]; reads
[[THO-016-increment-2-event-metric-trajectory-presenter-an]]. Owns emission
timing and ordering; the payload shape and version are the sibling slice.

# Result

Complete. Exactly the two events of this slice are emitted, both from the state
change the owning stage already computed; `OpposingTraversal` is untouched
([[TAS-123-record-the-wrong-way-interval-and-rule-state-tra]] owns its interval
edges), the payload shapes and `EVENT_VERSION = 3` are unchanged, and no decision
logic moved.

Emission seams:

- `crates/tangle-sim/src/sim.rs (Simulation::resolve_maneuvers)`, in the apply
  loop that writes the batch's planned state: one `Event::Maneuver` per
  `ManeuverTransition` the pass recorded, pushed where the edge is applied, so
  the event is the transition and a repeated step cannot re-derive one.
  `Simulation::maneuver_event` assembles the payload from the transition plus the
  route state that carries the maneuver's fixed facts — the state the attempt
  enters, and the state every other edge leaves, which keeps them until the settle
  edge into `following` clears them. `kind` is the state's target facility or a
  preserved `return_facility` (`change_lane`) else the mode's compiled `pass`
  (`overtake` otherwise); `partner` is `passed_body`; `source_facility` is
  `return_facility.unwrap_or(facility)`, the traversal the maneuver was attempted
  on; `target_offset_m` is the fixed target; `side` its sign in the agent's travel
  frame. A transition with no fixed target reports the payload's absent values:
  no partner, no target facility, `0.0` offset, and `left` — the side
  `narrow::pass_side` resolves an exact tie to. `reason` is the recorded
  termination reason on an `aborted` edge, `settled` on a completing edge, and
  `slower_leader` on the attempt and commit edges.
- `crates/tangle-sim/src/sim.rs (Simulation::record_facility_transition)`, called
  from `Simulation::handoff_connector` and `Simulation::handoff_lateral` where
  each already pushed its `FacilityTransitionRecord`: one
  `Event::FacilityTransition` per record, mapping every field, so the event and
  the record never disagree.
- Both buffers are the tick's own (`self.transitions` / `self.facility_transitions`
  are cleared per tick), and `Simulation::step` sorts by `Event::order_key` after
  every emission point, so the two new kinds take the appended positions TAS-119
  fixed with no change to `Event::order_key`.

Evidence (all commands run from the repository root):

- `cargo test -p tangle-sim`: 29 targets, 0 failed, including the new
  `sim::tests::a_maneuver_transition_emits_one_event_carrying_the_maneuver_facts`
  (one event per edge of the whole legal table, with the payload of each edge, and
  five later steps that record no transition and emit none),
  `sim::tests::a_transition_with_no_fixed_target_reports_the_payload_defaults`,
  `sim::tests::the_event_buffer_is_key_ordered_and_invariant_to_request_order`
  (per-step non-decreasing `order_key`, and an identical whole stream for the two
  request insertion orders at one event per recorded transition),
  `tests/maneuver_lifecycle.rs
  (every_recorded_edge_emits_one_maneuver_event_in_a_stable_key_order)`, and
  `tests/wrong_way.rs
  (a_connector_handoff_emits_one_facility_transition_event_mapping_its_record)`.
- `cargo test --workspace`: 84 targets, 0 failed — `wrong_way`,
  `maneuver_lifecycle`, `lane_transitions`, the CLI replay and summary suites, and
  the old-fixture regressions all pass.
- No golden changed: `apps/tangle-cli/tests/golden_trace.rs`,
  `increment6_trace.rs`, and `migration_regression.rs` pass byte-for-byte,
  because no Phase 1 or Increment 1 scenario authors a lateral, handoff, or
  wrong-way shape, so the new kinds emit nothing there. `apps/tangle-cli/src/trace.rs`
  needed no edit: TAS-119's mapping of both variants was already complete.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo fmt --all --check`, and `scripts/check-dependency-direction.sh` are
  clean.

Residual risk (reported, not silently accepted): a cross-facility `change_lane`
that has already crossed back reports the within-facility kind on its terminal
`-> following` edge, because `handoff_lateral` clears `target_facility` on the
outbound crossing and `return_facility` on the return crossing, so at that step
nothing in the route state still names the crossing. Closing it needs a maneuver
fact carried on `crates/tangle-sim/src/agent.rs (RouteState)`, which is outside
this slice's write set and would be a new decision; every other edge of every
maneuver reports one kind consistently.

Pending advance (coordinator): this node was the `next` of
[[TAS-100-version-the-maneuver-event-and-trace-surface]], whose write set this
slice does not own. Both of that node's children
([[TAS-119-add-the-maneuver-event-payloads-and-bump-event-v]] and this one) are
now resolved, so whether it removes `next` or names its first remaining action
is the coordinator's roll-up decision, not this slice's; verify with
`braintree check --allow-pending-advance TAS-100`.
