---
status: active
context_rev: 1
priority: P1
updated: 2026-09-15T05:46:56Z
summary: Check in the contextual wrong-way fixtures.
next: Wire `scenarios/phase2/inc2/narrow_wrong_way_v2.json5` into both benchmark-matrix representations and prove declaration-order invariance in the sibling TAS-132 slice.
---

Parent [[TAS-107-check-in-contextual-wrong-way-fixtures]].

# Outcome

Checked version-2 scenarios prove contextual opposing-route selection, ordinary
head-on interaction, and explicit rule evidence.

# Done when

- Fixtures cover a permitted nominal choice, a contextual prohibited-but-connected
  wrong-way choice, a physically disconnected rejection, and an occupied opposing
  corridor with deterministic wait, brake, or abort behavior.
- Tests assert perceived rule, reason, affected movements, interval boundaries,
  distance, duration, exposure, encounters, conflicts, ordinary collision and
  near-miss visibility, and route completion or explicit non-entry.
- CLI validate and run pass.

# Context

Gated on [[TAS-098-route-wrong-way-agents-through-ordinary-interactions]].
Gated on [[TAS-102-record-wrong-way-intervals-and-disaggregated-metrics]].
Extends [[TAS-107-check-in-contextual-wrong-way-fixtures]]; reads the fixture
gates in [[THO-016-increment-2-event-metric-trajectory-presenter-an]].
`apps/hekate-cli/tests/migration_regression.rs` enumerates `scenarios/**/*.json5`.
Owns the core wrong-way fixtures; the matrix wiring is the sibling slice.

# Result

`scenarios/phase2/inc2/narrow_wrong_way_v2.json5` authors four isolated 100 m
forward-nominal corridors, one per case: `a` permitted (`a_back_to_a_left` makes
its reverse traversal possible and a `nominal_direction` `permit` binds
`bicycle`), `b` prohibited-but-connected (same topology, no statement), `c`
physically disconnected (no connector leaves it along its reverse direction), and
`d` occupied (connected and permitted, with two demand sources so a turned rider
meets a body travelling the rule direction). `urgency: 1.0` with every mode's
`compliance` fixed at `0.0` and constant `min == max` bodies, speeds, and
profiles make the decision deterministic for every draw.

`apps/hekate-cli/tests/run_metrics.rs` gains the focused suite: one test per case
plus a replay test and a CLI run-directory test. Each case's entry is recorded
through `Simulation::request_wrong_way_entry`, the seam a tactical leaf supplies,
exactly as the landed TAS-124 fixture's tests do. **That harness path is the
entry path used, not the ordinary demand/compliance path:** the kernel has no
production caller of the request seam (`request_wrong_way_entry` is defined in
`crates/hekate-sim/src/sim.rs` and called only by tests; no tactic leaf or demand
path records it), so a checked-in `.json5` cannot fire the decision, and the test
supplies the request while every input the decision reads stays the fixture's.

Asserted per case: the interval boundaries as event opens/closes cross-checked
against the tracker's own ticks, the decision's perceived rule, reason code,
affected movement, direction, and legality, route completion onto the continuance
or the refusal with no boundary and no handoff, and the six TAS-124 families
(`wrong_way_intervals`, `wrong_way_distance_m`, `wrong_way_duration_s`,
`wrong_way_exposure_agent_s`, `wrong_way_encounters`, `wrong_way_conflicts`) with
their explicit `MetricStatus` at run level and per rule, facility, participant
pair, and mode-pair bucket. The occupied corridor asserts the ordinary collision
scan's near-miss and contact bands, `T-H1` (`>= -1e-9` m on the scan's least
clearance and the measured least bumper gap), `T-H2` (no bypass: the turned rider
never leaves its corridor and never runs a lateral maneuver), and the run-end
closure of the interval the encounter leaves open.

Measured at seed 0, step 0.05 s (no tolerance or disposition widened): corridor
`a` admits its sole rider at tick 1048, `b` at tick 1135 after a pair at 13/23 has
cleared, `c` at tick 3674 — each muted case's turned rider records two intervals
with zero encounters and conflicts and no observed exposure. The occupied
corridor places its pair at tick 119 (leading rider at 25.2 m, 9.8 m bumper gap),
which is inside the 18 m two 6 m/s bodies need to stop under 2 m/s² braking, so
the head-on meeting is unavoidable: one entering near miss, one contact, least
bumper gap -1.3e-15 m, no overlap, no bypass. The CLI run-directory test pins the
fixture's block through the real binary: the countable families are observed
zeros and the value families report `not_observed` (not `not_applicable`) because
the file declares the policy and the capability, with all seven compiled
facilities as buckets.

`hekate-cli validate` and `run --seed 0` pass; `hekate-sim`'s wrong-way suite,
`run_metrics`, and `migration_regression` (which enumerates every `scenarios/**`
`*.json5`) pass. Remaining scope is the sibling slice's: the matrix path entries
and declaration-order invariance. Note for it that a CLI run of this fixture at
the default 250-tick horizon records no opposing traversal and shows only the
occupied corridor's traffic, because the decision needs a request the kernel does
not yet produce on its own.
