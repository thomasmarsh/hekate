---
context_rev: 1
updated: 2026-09-14T00:50:52Z
summary: The Increment 2 authored, compiled, policy, state-machine, and evidence contract is fixed in docs/schema-v2-contract.md and the Increment 1 deferrals are reconciled.
---

Parent [[TAS-082-increment-2-authored-and-compiled-contract]].

# Outcome

docs/schema-v2-contract.md extends version 2 in place with the complete
consumer-visible Increment 2 contract before implementation begins.

# Done when

- Exact source fields, types, units, omission/default rules, identifier targets,
  and normalized examples are fixed for lateral-use policy, target clearance
  and horizon, adjacent-lane or facility transitions, clearance bands, unsafe
  commit policy, lane_use/overtake/crossing permissions, and contextual
  nominal-direction violation.
- Compiled semantics separate nominal, permitted, and physically possible
  traversal; define usable and predicted corridors; and name front, rear, side,
  and swept clearance facts.
- The following/preparing/committed/returning/aborted transitions, simultaneous
  claim tie-break, commitment loss policy, wrong-way decision inputs, and
  violation interval boundaries are unambiguous and deterministic.
- The event and metric section names payloads, ordering, applicability, and the
  exact definition-version bumps that later consumers must observe.
- Increment 1 deferral rows are reconciled, later-increment scope remains
  deferred, and all cited implementation seams exist.

# Context

Read PHASE_2_PLAN.md sections Continuous lateral motion, Overtaking and close
passing, Wrong-way movement, Safety operations and outputs, Reproducibility and
fidelity, and Decisions for implementation; read the resolved Increment 1
contract leaf [[TAS-073-extend-the-version-2-schema-contract-with-increm]].
Owns docs/schema-v2-contract.md only. Do not edit source, generated schema,
benchmark matrix, or resolved node files.

This documentation is the dependency contract for TAS-084 through TAS-110.

# Result

`docs/schema-v2-contract.md` gains a terminal section, **Increment 2 additions:
lateral motion, passing, and wrong-way travel**, which extends the Increment 0
and Increment 1 contract in place. No earlier shape, field meaning,
version-negotiation rule, provenance field, or deferral boundary was
reinterpreted, no new schema version or provenance field was introduced, and
every new field is required or explicitly optional with a documented absence
rule.

**Source shapes fixed**, each with types, units, identifier targets, omission
rules, and a normalized example:

- `mode_templates[].tactics` gains `change_lane`, `overtake`, `pass`, and
  `reverse_direction` with their compiled capabilities (plus
  `ChooseLateralPosition` for `change_lane`/`pass`);
  `mode_templates[].lateral` fixes `target_clearance_m` (m) and `horizon_s` (s);
  `lateral_accel_max_mps2` joins the wheeled profile set and is required exactly
  when `lateral` is present, while `steering_rate_max_rad_s` and
  `lateral_clearance_m` become required for every lateral-capable wheeled
  template. The plan's steering-angle limit is represented by the heading-rate
  and lateral-acceleration limits because version 2 authors no axle geometry; the
  document says so explicitly instead of inventing a wheelbase.
- `facilities[].lateral_policy.passing_side` (`left`/`right`/`most_clearance`)
  plus the free-lateral-motion behaviour of the existing `lateral_use` values.
- `facility_adjacencies[]` (`id`, `first`, `second`, `side`) as the only lateral
  transition relation; Increment 1 `facility_connectors[]` keeps its exact
  meaning.
- `clearance_bands[]` (`id`, `threshold_m`, `violation`, optional
  `applies_to_modes`) as scenario-scoped definitions declared in strictly
  increasing thresholds, with the rule that a band's `threshold_m` is the scalar
  a tolerance's "authored clearance band" names.
- `maneuver_policy` with `commit` (`min_predicted_clearance_m`,
  `hold_timeout_s`) and `wrong_way` (`min_time_saving_s`,
  `max_opposing_density_per_km`, `urgency`).
- effect semantics, resolution, precedence, and contradiction rules for the
  `nominal_direction`, `lane_use`, `overtake`, and `crossing` permission kinds,
  including the rule that the permitted set never leaves the physically possible
  set.

**Compiled semantics fixed**: nominal, permitted, and physically possible
traversal per `(mode, traversal)`; usable interval, usable corridor, and
predicted corridor; the `front_clearance_m`, `rear_clearance_m`,
`side_clearance_m`, and `swept_clearance_m` facts with their sign convention,
limiting object, time, and closing speed; lateral and longitudinal transition
targets; the single geometric handoff per transition kind; and the
forbidden-boundary fact.

**State machines and policies fixed**: the five states with a full legal
transition table (guards and recorded edge); batch claim arbitration with a total
winner key that is invariant to declaration, discovery, and insertion order; the
ordered brake/hold/abort commitment-loss response and the authored thresholds it
reads; the wrong-way decision inputs, the `maneuver`-stream non-compliance draw
with its tie rule and its placement relative to the non-random preconditions; and
the violation-interval open/close boundaries.

**Evidence fixed**: four new event variants with payloads, applicability, and
append-only ordering that leaves existing kinds' sort positions unchanged;
additive metric families with applicability and disaggregation; the additive
trajectory/snapshot columns; and the definition-version bumps, each bumped
exactly once for its additive union with the implementing leaf named
(`EVENT_VERSION` by [[TAS-100-version-the-maneuver-event-and-trace-surface]],
`TRAJECTORY_FORMAT_VERSION` by [[TAS-088-add-route-relative-lateral-agent-state]]
with [[TAS-102-record-wrong-way-intervals-and-disaggregated-metrics]] adding
columns under it, `SCENE_FORMAT_VERSION` by
[[TAS-111-present-increment-2-corridor-gap-and-rule-overlays]],
`METRIC_DEFINITION_VERSION` by
[[TAS-101-measure-close-passes-with-exact-clearance-evidence]] with TAS-102
adding families under it, and the run manifest's own shape version by TAS-088).
The numeric values are deliberately not fixed here; the rule and the
"bump exactly once for the additive union" statement are.

**Reconciliation**: a new *Reconciling the Increment 1 deferrals* table disposes
every Increment 1 deferral row. Owned now: free lateral position selection,
lane/facility transitions, gap prediction, manoeuvre commitment, overtaking and
passing, close-pass evidence, contextual wrong-way selection, the four tactics,
the `lane_use`/`overtake`/`crossing` permissions, and the `lateral_use` behaviour
sentence. Still deferred with their incumbent named: heavy and articulated
templates, `articulated_chain`, `articulated_wheeled`, `fixed`/`transit`
occupancy (Increment 3), transit stops, `serve_stop`, and `stop_service`
(Increment 4), pedestrian groups (Increment 5), and the navigation mesh. Two
Increment 1 limitations Increment 2 evidence touches are recorded with their
consequence and no new field — polyline-only `paths[].points` (a curved reference
follows the `CompiledReferencePath::arc` chord pattern the Increment 1 curve
fixture uses) and the parsed-but-unsimulated `demand[].spawn.rate.interval_s`
window (`FBK-028`).

**Maintenance**: the Increment 0 and Increment 1 seam line citations had gone
stale when Increment 1's shapes landed (they pointed at blank lines or unrelated
symbols). Each was refreshed to the symbol it names; the Increment 2 section
cites symbols by name instead of by line for that reason. No semantic text of
either earlier section changed.

Acceptance:

- `braintree check` — `graph check: passed (154 nodes)` while this node was still
  in `proposed/`.
- Seam resolution — all 13 `file:line` citations in the document point at the
  symbol they name (`source.rs` 17/613, `components.rs` 285, `validate.rs`
  205/367/1604, `compiled.rs` 565/570/585/2440, `mode_template.rs` 297,
  `baseline.rs` 88, `run_dir.rs` 301), and the 9 shorthand lines of the Increment
  1 seam list (`source.rs` 663/788/919/1019, `components.rs` 93/163/299/325/618)
  were verified the same way. Every symbol-level seam the new section names
  exists (`rg` over `crates/**` and `apps/**`): the source and component types,
  the compiled facility/connector/traversal/reference geometry, the validation
  seams, the stage, sim, controller, control, agent, profile, event, metric,
  query, swept, rng, config, presentation, and CLI surfaces listed in its
  *Authority and scope*.
- Wikilinks — all 37 node links in the document resolve to node files under
  `.braintree/`; `CC-OVERTAKE`/`CC-OPPOSE`, `T-O1`…`T-O3`, `T-H1`/`T-H2`, and
  the four Increment 2 fixture ids resolve in `docs/benchmark-matrix.md`;
  `DEF-004`/`DEF-005` resolve under `.braintree/resolved/`.
- `git status --porcelain` — `docs/schema-v2-contract.md` modified, this node
  moved to `resolved/`; no source, generated schema, benchmark-matrix, or other
  node file touched. Docs only, so no crate test was run or required.
- Post-move note: `braintree check --allow-pending-advance TAS-082` was run after
  this file moved, because [[TAS-082-increment-2-authored-and-compiled-contract]]
  still names this node in its `next`; the coordinator advances it.

Coordination findings (no new node created):

- Adding a field to `ScenarioSourceV2`, `ModeTemplateSource`, or `AccessSource`
  forces mechanical edits in `crates/tangle-model/src/migrate.rs` (three
  exhaustive literals) and in the exhaustive test literals of
  `crates/tangle-model/tests/mode_template_compilation.rs`,
  `version2_source.rs`, and `synthetic_mode_template.rs`. None of those files is
  in an Increment 2 leaf's declared write set, so
  [[TAS-084-parse-increment-2-lateral-and-rule-source-shapes]] needs its write set
  extended to cover the migration and those literals before it lands the source
  fields.
- This contract fixes the tolerance-facing band scalar as a band's
  `threshold_m`, and fixes the "wrong-way permission with no opposing path" case
  as an inert statement closed by the decision's `no_opposing_path` reason rather
  than a hard validation failure, because the accepted Increment 1 fixtures
  (`crates/tangle-model/tests/facility_validation.rs`, `source.rs` tests) author
  exactly that combination and must stay valid.
  [[TAS-086-validate-increment-2-lateral-and-wrong-way-policy]] owns the
  diagnostics and must reconcile its rule wording with this section.

Friction (for the session `FBK`, not created here):

- Attempted: extend the contract while trusting its existing `file:line` seam
  citations. Friction: 12 of the 22 citations had drifted to blank lines or
  unrelated symbols once Increment 1's shapes landed, and nothing failed —
  `braintree check` validates the graph, not a document's citations. Improvement:
  have `SKILL.md` state that a contract cites `path (Symbol)` and only adds a
  line when the citing leaf also owns the file, or let the repo doctor check that
  every `file:line` citation still contains its named symbol.
- Attempted: add the Increment 2 fields to version-2 source structs as the
  contract requires. Friction: the write sets are per-leaf and per-file, so the
  shared migration and the exhaustive struct literals in three test files fall
  between the leaves that must change them. Improvement: when a leaf's outcome
  adds a struct field, its write set should name the migration and the exhaustive
  literal sites, or decomposition should record shared-file ownership explicitly.
- Attempted: reconcile "the Increment 1 deferral rows". Friction: the deferrals
  live in two places (the Increment 1 reconciliation table and its *What
  Increment 1 still defers* bullets), and the permission kind `stop_service`
  versus the tactic `serve_stop` are separate deferred items with one incumbent.
  Improvement: contracts should carry one deferral table with a single row per
  deferred item and its owning increment, and tasking should cite rows by their
  exact wording.
