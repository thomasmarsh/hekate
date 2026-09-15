# Phase 2 benchmark matrix and tolerances

**Status:** checked-in planning artifact for the Increment 0 deliverable of
`PHASE_2_PLAN.md` ("Benchmark matrix and quantitative tolerances for
independent, pairwise, and mixed-mode validation").
**Owner:** [[TAS-059-benchmark-matrix-and-tolerances]].
**Machine-readable companion:** `docs/benchmark-matrix.json` carries the same
modes, families, pairs, cells, dispositions, and counts.

This document names every independent-mode cell, every pairwise
`(mode pair, interaction family)` cell, and every mixed-mode cell of the Phase 2
validation ladder (`PHASE_2_PLAN.md`, *Validation strategy*), and fixes for each
one the fixture, the quantity compared, the numeric tolerance, the baseline the
tolerance derives from, and the fidelity presets that apply. It is a plan: no
scenario, test, or code is added by it. Later increments cite it and replace its
planned fixture paths with the paths they check in; they must not widen a
disposition or relax a tolerance without revising this document.

## 1. How to read a cell

A cell is one coordinate of the matrix. It carries exactly one disposition:

- **supported** — Phase 2 commits a fixture and a metric tolerance for the
  interaction. Written `S:<class>`.
- **impossible** — the interaction cannot arise under the Phase 2 model's
  construction. Written `I:<reason>`, and the reason is fixed in §4.3.
- **deferred** — physically possible and inside Phase 2 scope, but no Phase 2
  increment commits a dedicated fixture. Written `D:<increment>`. No cell in
  this revision is deferred; §9 explains why and §10 lists what Phase 2 defers
  outside the cell taxonomy.

A supported cell names all four required fields through its **cell class**:

> A supported cell written `S:<class>` is complete exactly when the class in §5
> fixes the fixture (§7), the quantity compared, the numeric tolerance bound and
> its baseline (§6), and the fidelity presets (§2).

This composition rule makes the grid compact without losing information: §8
carries the grid, §5 defines every class it uses, and §11 proves by count that
every class reference in the grid resolves.

## 2. Fidelity presets

The three Phase 2 presets and their physics steps are frozen by the Phase 1
baseline (`baselines/phase1/baseline.json`, `presets[].step_s`) and extended by
`PHASE_2_PLAN.md`, *Reproducibility and fidelity* with lateral-decision cadence,
steering integration tolerance, articulated-segment sweep tolerance, and
maneuver-prediction horizon.

| Abbrev. | Preset | Physics step | Used by |
| --- | --- | --- | --- |
| `F` | Fast | 0.100 s | independent-mode physical/operational cells only |
| `S` | Standard | 0.050 s | every cell |
| `f` | Fine | 0.020 s | every cell; the reference fidelity for convergence |

An interaction cell is judged at `S` and `f`, matching the ladder's fidelity rung
("selected fixtures and experiment findings at Standard and Fine"). Independent
physical cells additionally run at `F` because their bounds are analytic and
step-independent, so `F` is a cheap regression preset rather than a second
measurement.

## 3. Baselines

Every tolerance in §6 names the baseline it derives from. Two kinds appear:

- **Phase 1 baseline** — `baselines/phase1/baseline.json` plus the Phase 1
  fixtures it freezes (`scenarios/benchmarks/*_v1.json5`,
  `scenarios/experiments/four_leg_pedestrian_*_v1.json5`). It applies to every
  cell whose pair is `passenger_car` and/or `pedestrian`, at the frozen
  `event_version` it records (`baseline.json` records `event_version: 2`, and the
  metric definition is the frozen `DEF-005` v2).
- **Increment-declared reference** — the analytic, high-resolution, or
  hand-computable reference the owning increment declares with its fixture. It
  applies to every new mode and to cross-mode cells whose new-mode side has no
  Phase 1 evidence: Increment 1 (bicycle/scooter analytic path reference),
  Increment 2 (fine-step executed clearance for lateral maneuvers),
  Increment 3 (high-resolution articulated reference integrator),
  Increment 4 (hand-computable dwell/cohort ledger), Increment 5 (pairwise and
  group reference fixtures), Increment 6 (ambiguity fixture), Increment 7
  (release experiment spec, demand bank, and seed bank).

No tolerance below is a trace-hash or golden-file comparison. The exact
determinism/replay checks (a run reproduces its own trace hash) are gates, not
tolerances, and are deliberately kept out of §6; §7.4 states where they live.

## 4. Coverage axes

### 4.1 Modes

The seven Phase 2 product-slice modes. "Class" is the mode class used by the
disposition rules: `M` motor single-body wheeled, `N` narrow wheeled, `A`
articulated wheeled, `P` pedestrian.

| Mode | Class | Independent cell | Fixture (§7) | Owned by |
| --- | --- | --- | --- | --- |
| `passenger_car` | M | `CC-CAR` | `straight_approach_v1` (existing) | Phase 1 |
| `pedestrian` | P | `CC-PED` | `pedestrian_crossing_v1` (existing) | Phase 1 |
| `bicycle` | N | `CC-NARROW` | `narrow_isolated_straight_v2`, `narrow_isolated_curve_v2` (checked in) | Increment 1 |
| `scooter` | N | `CC-NARROW` | `narrow_isolated_straight_v2`, `narrow_isolated_curve_v2` (checked in) | Increment 1 |
| `bus` | M | `CC-HEAVY`, `CC-TRANSIT` | `heavy_isolated_v2`, `transit_low_demand_v2` (planned) | Increments 3, 4 |
| `rigid_truck` | M | `CC-HEAVY` | `heavy_isolated_v2` (planned) | Increment 3 |
| `tractor_semitrailer` | A | `CC-ARTIC` | `articulated_reference_v2` (planned) | Increment 3 |

All seven modes have disposition **supported** for their independent cell. No
mode is impossible or deferred: `PHASE_2_PLAN.md` *Phase 2 product slice* commits
each one, and Increments 1, 3, and 4 own their independent fixtures.

### 4.2 Interaction families

The taxonomy is fixed by `PHASE_2_PLAN.md`, *Safety, operations, and outputs*
("following, crossing, merging, overtaking, head-on/opposing, shared-space, or
stop-service related ... plus `unknown`"). Increment 5's summary is re-expressed
in these names: its *passing* is `overtaking`, its *opposing* is
`head-on/opposing`, and its *shared facility* is `shared-space`.

| Family | Cell class | Precondition for a supported cell |
| --- | --- | --- |
| following | `CC-FOLLOW` | both modes share one same-direction facility (wheeled) or one walking stream (pedestrians) |
| crossing | `CC-CROSS` | the two paths cross at an authored conflict region |
| merging | `CC-MERGE` | two streams join one downstream facility or connector |
| overtaking | `CC-OVERTAKE` | a wheeled agent holds a lateral-gap tactic past a slower user |
| head-on / opposing | `CC-OPPOSE` | two streams travel toward each other on one facility, or a wrong-way agent joins one |
| shared-space | `CC-SHARED` | a facility with shared-space semantics mixes pedestrian and wheeled agents |
| stop-service | `CC-STOP` | a bus serves a stop and interacts with a waiting cohort or the surrounding stream |
| unknown | `CC-UNKNOWN` | reserved classification: an ambiguous trajectory is labelled `unknown`, never forced |

### 4.3 Impossible-reason codes

| Code | Reason |
| --- | --- |
| `I:FD` | Disjoint facility geometry: pedestrians use Phase 1 walking geometry (paths, crossings, waiting areas) while wheeled agents use facility regions and reference paths, so the two do not share a same-direction or merging facility. The co-located pedestrian/wheeled case is the `shared-space` family. |
| `I:OP` | No pedestrian lateral-passing tactic: the walking model follows routes with local collision avoidance; pedestrian passing and dispersion are measured inside the `shared-space` and `crossing` families, not as `overtaking`. |
| `I:WWS` | Wheeled-only shared space is undefined: Phase 2 gives shared-space semantics only to facilities that mix pedestrians with wheeled modes, so two wheeled agents co-located outside those semantics use the `following`, `crossing`, or `overtaking` families. |
| `I:NOBUS` | No stop service without a bus: the `stop-service` family requires a `bus` participant. |

## 5. Cell classes

Each class below fixes the quantity compared, the numeric tolerance, the baseline
it derives from, and the fidelity presets. A tolerance with two or three
quantities is a conjunction: all must hold. `S` and `f` are the interaction
presets; `F, S, f` are the independent-mode presets.

### 5.1 Independent-mode classes

| Class | Quantity compared | Tolerance (metric bound) | Baseline | Presets |
| --- | --- | --- | --- | --- |
| `CC-CAR` | coordinate round-trip error; body dimension error; command-envelope violations; steady-state free-speed error | `T-RT`, `T-DIM`, `T-ENV`, `T-SPD` | Phase 1: `straight_approach_v1`, `baselines/phase1/baseline.json` | F, S, f |
| `CC-PED` | circle-diameter error; command-envelope violations; steady-state free-speed error | `T-DIM`, `T-ENV`, `T-SPD` | Phase 1: `pedestrian_crossing_v1`, `mixed_interaction_v1` | F, S, f |
| `CC-NARROW` | coordinate round-trip error; body dimension error; command-envelope violations; steady-state free-speed error | `T-RT`, `T-DIM`, `T-ENV`, `T-SPD` | Increment 1 declared reference: `narrow_isolated_straight_v2`, `narrow_isolated_curve_v2` (analytic path geometry) | F, S, f |
| `CC-HEAVY` | coordinate round-trip error; body dimension error; command-envelope violations; swept-turn feasibility | `T-RT`, `T-DIM`, `T-ENV`, `T-SWEPT` | Increment 3 declared reference: `heavy_isolated_v2` (analytic straight/constant-radius) | F, S, f |
| `CC-ARTIC` | max segment pose error; off-tracking error; missed segment contacts | `T-ART`, `T-OFF`, `T-SWEPT` | Increment 3 declared reference: `articulated_reference_v2` (high-resolution reference integrator) | F, S, f |
| `CC-TRANSIT` | passenger conservation residual; hand-computed dwell error; occupancy bound | `T-CONS`, `T-DWELL`, `T-OCC` | Increment 4 declared reference: `transit_low_demand_v2`, `transit_variable_dwell_v2` (cohort ledger) | S, f |
| `CC-GROUP` | stranded group members; group conservation residual | `T-GROUP`, `T-CONS` | Increment 5 declared reference: `group_*_v2` | S, f |

### 5.2 Pairwise and mixed classes

| Class | Quantity compared | Tolerance (metric bound) | Baseline | Presets |
| --- | --- | --- | --- | --- |
| `CC-FOLLOW` | per-mode-pair minimum surface separation; command-envelope violations; collision-event count | `T-SEP`, `T-ENV`, `T-REG` | Phase 1 for `passenger_car`/`pedestrian` cells (`car_following_v1`); Increment 5 pairwise reference fixture for new modes | S, f |
| `CC-CROSS` | per-mode-pair minimum surface separation; minimum TTC; PET; collision-event count | `T-SEP`, `T-TTC`, `T-REG` | Phase 1 (`perpendicular_conflict_v1`, `pedestrian_crossing_v1`, `mixed_interaction_v1`, `four_leg_signal_v1`); Increment 5 pairwise reference fixture otherwise | S, f |
| `CC-MERGE` | per-mode-pair minimum surface separation; unresolved mutual-yield steps | `T-SEP`, `T-DEAD` | Increment 5 pairwise reference fixture | S, f |
| `CC-OVERTAKE` | predicted vs executed minimum clearance; close-pass minimum clearance; boundary-crossings without an event | `T-O1`, `T-O2`, `T-O3` | Increment 2 declared reference (fine-step executed clearance, authored clearance band) | S, f |
| `CC-OPPOSE` | minimum separation in the opposing corridor; occupied-corridor bypasses | `T-H1`, `T-H2` | Increment 2 declared reference (`narrow_wrong_way_v2`) for bicycle/scooter cells; Increment 5 pairwise reference fixture otherwise | S, f |
| `CC-SHARED` | per-pair minimum surface separation; unresolved mutual-yield steps | `T-SEP`, `T-DEAD` | Increment 5 declared shared-space reference fixture | S, f |
| `CC-STOP` | passenger conservation residual; occupancy bound; hand-computed dwell error | `T-CONS`, `T-OCC`, `T-DWELL` | Increment 4 declared reference (`transit_low_demand_v2`, `transit_blocked_berth_v2`, `transit_merge_back_v2`) | S, f |
| `CC-UNKNOWN` | forced-classification count | `T-CLASS` | Increment 6 declared ambiguity fixture | S, f |
| `CC-MIXED` | agent/passenger/route conservation residual; per-mode-pair minimum separation; conclusion sign stability | `T-CONS`, `T-SEP`, `T-STABLE` | Increment 7 declared release experiment spec, demand bank, and seed bank | S, f |

## 6. Tolerance catalogue

Every entry is a numeric bound on a named metric or geometric quantity, with the
baseline it derives from. None is a golden-file or trace-hash comparison.

| Id | Quantity | Bound | Baseline |
| --- | --- | --- | --- |
| `T-SEP` | per-mode-pair minimum surface separation, m | `>= -1e-9` (no unresolved overlap; `CONTACT_EPSILON_M` = 1e-9 m) | Phase 1 (`baselines/phase1/baseline.json`) for car/pedestrian cells; the fixture's increment-declared reference otherwise |
| `T-REG` | collision-event count on the fixture, records | `<= the frozen Phase 1 count for the same fixture` | Phase 1 baseline `baselines/phase1/baseline.json` and its acceptance fixtures |
| `T-CONS` | agent / passenger / route conservation residual | `= 0` | Phase 1 for car/pedestrian; Increment 4 cohort ledger; Increment 7 release spec |
| `T-DEAD` | unresolved mutual-yield steps over the declared run horizon | `= 0` | the pairwise/shared-space fixture's Increment 5 declared horizon |
| `T-O1` | `|predicted - executed minimum clearance|`, m | `<= 0.10` | Increment 2 declared reference (fine-step executed clearance) |
| `T-O2` | close-pass minimum body-to-body clearance, m | `>= authored clearance band - 0.01` | Increment 2 declared reference (authored clearance band) |
| `T-O3` | boundary-crossings without an emitted event, count | `= 0` | Increment 2 declared reference |
| `T-H1` | minimum separation in the opposing corridor, m | `>= -1e-9` | Increment 2 declared reference (`narrow_wrong_way_v2`) |
| `T-H2` | occupied-opposing-corridor bypasses, count | `= 0` | Increment 2 declared reference |
| `T-DWELL` | `|realized dwell - hand-computed dwell formula|`, s | `<= 1e-9` | Increment 4 declared hand-computable dwell fixture |
| `T-OCC` | bus occupancy, passengers | `0 <= occupancy <= capacity` | Increment 4 declared reference |
| `T-TTC` | TTC reporting resolution, s; PET reporting resolution, s; applicability status presence | `resolution <= 1e-6`; status present whenever a value is out of range | `DEF-005` reporting constants (`TTC_TIME_TOLERANCE_S`, `SEPARATION_RESOLUTION_M`) |
| `T-CLASS` | forced-classification count (a non-`unknown` label on an ambiguous trajectory), records | `= 0` | Increment 6 declared ambiguity fixture |
| `T-GROUP` | stranded group members, count | `= 0` | Increment 5 declared group reference fixture |
| `T-STABLE` | material-conclusion verdict flips between `S` and `f` | `= 0` flips, or each flip explicitly reported as sensitive | Increment 7 declared release experiment spec |
| `T-RT` | path-to-world-to-path coordinate round-trip error, m | `<= 1e-9` | Increment 1 declared analytic path reference (`narrow_isolated_curve_v2`) |
| `T-DIM` | realized body dimension vs authored template, m | `<= 1e-12` | the authored version-2 mode template |
| `T-ENV` | command-envelope violations, count | `= 0` | the sampled profile envelope of the mode template (Increments 1/3) |
| `T-SPD` | steady-state free-speed error vs authored constant desired speed, m/s | `<= 1e-6` after the declared settling distance | Increment 1/3 declared analytic straight reference |
| `T-ART` | max segment pose error vs high-resolution reference, m | `<= 0.05` | Increment 3 declared high-resolution articulated reference integrator |
| `T-OFF` | off-tracking error vs reference, m | `<= 0.05` | Increment 3 declared high-resolution reference |
| `T-SWEPT` | missed segment contacts vs the all-pairs reference query, count | `= 0` | Increment 3 declared all-pairs reference query |

## 7. Fixtures

### 7.1 Existing Phase 1 fixtures (baseline)

| Id | Path | Covers |
| --- | --- | --- |
| `walking_guide_v1` | `scenarios/walking/walking_guide_v1.json5` | car independent (walking skeleton), baseline trace manifest |
| `straight_approach_v1` | `scenarios/benchmarks/straight_approach_v1.json5` | car independent longitudinal |
| `car_following_v1` | `scenarios/benchmarks/car_following_v1.json5` | car-car following |
| `perpendicular_conflict_v1` | `scenarios/benchmarks/perpendicular_conflict_v1.json5` | car-car crossing |
| `offset_junction_v1` | `scenarios/benchmarks/offset_junction_v1.json5` | car-car crossing, staggered geometry |
| `red_light_compliance_v1` | `scenarios/benchmarks/red_light_compliance_v1.json5` | car control |
| `four_leg_signal_v1` | `scenarios/benchmarks/four_leg_signal_v1.json5` | car-pedestrian crossing and control |
| `pedestrian_crossing_v1` | `scenarios/benchmarks/pedestrian_crossing_v1.json5` | car-pedestrian crossing |
| `mixed_interaction_v1` | `scenarios/benchmarks/mixed_interaction_v1.json5` | car-pedestrian crossing, two crosswalks |
| `four_leg_pedestrian_{ew,ns}_priority_v1` | `scenarios/experiments/four_leg_pedestrian_{ew,ns}_priority_v1.json5` | car-pedestrian mixed release comparison |

### 7.2 Checked-in fixtures

The Increment 1 narrow-mode fixtures and the Increment 2 `CC-OVERTAKE` and
`CC-OPPOSE` fixtures. Each path is checked in and loads through the CLI; later
increments revise this section the same way, moving a planned path here when it
checks the path in.

| Id | Path | Covers |
| --- | --- | --- |
| `narrow_isolated_straight_v2` | `scenarios/phase2/inc1/narrow_isolated_straight_v2.json5` | narrow `CC-NARROW` independent cell, both modes: free-flow straight |
| `narrow_isolated_curve_v2` | `scenarios/phase2/inc1/narrow_isolated_curve_v2.json5` | narrow `CC-NARROW` independent cell, both modes: free-flow constant-curvature curve |
| `narrow_isolated_braking_v2` | `scenarios/phase2/inc1/narrow_isolated_braking_v2.json5` | narrow isolated braking, both modes: stop line held by a red head |
| `narrow_following_v2` | `scenarios/phase2/inc1/narrow_following_v2.json5` | narrow-narrow `following`, both modes |
| `narrow_signal_v2` | `scenarios/phase2/inc1/narrow_signal_v2.json5` | narrow isolated signal, both modes: yield, hold, and proceed |
| `narrow_crossing_v2` | `scenarios/phase2/inc1/narrow_crossing_v2.json5` | narrow `crossing`, both modes: traverse an authored conflict region |
| `narrow_passing_v2` | `scenarios/phase2/inc2/narrow_passing_v2.json5` | narrow-narrow `CC-OVERTAKE`: bicycle and scooter pass on one shared bikeway |
| `motor_passing_narrow_v2` | `scenarios/phase2/inc2/motor_passing_narrow_v2.json5` | motor-over-narrow `CC-OVERTAKE`: a passenger car overtakes a slower bicycle |
| `motor_lane_change_v2` | `scenarios/phase2/inc2/motor_lane_change_v2.json5` | motor-over-motor `CC-OVERTAKE`: configured change of lane around a slower leader |
| `narrow_wrong_way_v2` | `scenarios/phase2/inc2/narrow_wrong_way_v2.json5` | contextual wrong-way `CC-OPPOSE`: a permitted opposing choice, a prohibited-but-connected one, a disconnected refusal, and an occupied opposing corridor |

### 7.3 Planned fixtures

Each row is the path and owning increment for one class of cell. The pairwise
convention fixes one file per supported `(pair, family)` coordinate.

| Id / pattern | Path | Owned by |
| --- | --- | --- |
| `heavy_isolated_v2` | `scenarios/phase2/inc3/heavy_isolated_v2.json5` | Increment 3 |
| `heavy_following_v2` | `scenarios/phase2/inc3/heavy_following_v2.json5` | Increment 3 |
| `articulated_reference_v2` | `scenarios/phase2/inc3/articulated_reference_v2.json5` | Increment 3 |
| `articulated_conflict_v2` | `scenarios/phase2/inc3/articulated_conflict_v2.json5` | Increment 3 |
| `transit_low_demand_v2` | `scenarios/phase2/inc4/transit_low_demand_v2.json5` | Increment 4 |
| `transit_capacity_v2` | `scenarios/phase2/inc4/transit_capacity_v2.json5` | Increment 4 |
| `transit_variable_dwell_v2` | `scenarios/phase2/inc4/transit_variable_dwell_v2.json5` | Increment 4 |
| `transit_merge_back_v2` | `scenarios/phase2/inc4/transit_merge_back_v2.json5` | Increment 4 |
| `transit_blocked_berth_v2` | `scenarios/phase2/inc4/transit_blocked_berth_v2.json5` | Increment 4 |
| `pair_<a>__<b>__<family>_v2` | `scenarios/phase2/inc5/pair_<a>__<b>__<family>_v2.json5` | Increment 5 |
| `shared_space_<a>__<b>_v2` | `scenarios/phase2/inc5/shared_space_<a>__<b>_v2.json5` | Increment 5 |
| `group_*_v2` | `scenarios/phase2/inc5/group_<name>_v2.json5` | Increment 5 |
| `density_ramp_v2` | `scenarios/phase2/inc5/density_ramp_v2.json5` | Increment 5 |
| `ambiguous_interaction_v2` | `scenarios/phase2/inc6/ambiguous_interaction_v2.json5` | Increment 6 |
| `mixed_release_v2` | `scenarios/phase2/inc7/mixed_release_v2.json5` | Increment 7 |
| `mixed_release_saturated_v2` | `scenarios/phase2/inc7/mixed_release_saturated_v2.json5` | Increment 7 |

### 7.4 Fixture resolution for a supported pairwise cell

Read top to bottom; the first row that matches names the cell's fixture. The
`<family>` token is the family slug (`following`, `crossing`, `merging`,
`overtaking`, `opposing`, `shared_space`); the `<a>__<b>` token is the pair in
the §8 label order.

1. `unknown` family → `scenarios/phase2/inc6/ambiguous_interaction_v2.json5`
   (Increment 6).
2. `shared_space` family → `scenarios/phase2/inc5/shared_space_<a>__<b>_v2.json5`
   (Increment 5).
3. `stop_service` family → bus-pedestrian
   `scenarios/phase2/inc4/transit_low_demand_v2.json5`; bus-bus
   `scenarios/phase2/inc4/transit_blocked_berth_v2.json5`; bus with any other
   wheeled mode `scenarios/phase2/inc4/transit_merge_back_v2.json5`
   (Increment 4).
4. car-car `following` → `scenarios/benchmarks/car_following_v1.json5`
   (Phase 1); narrow-narrow `following` →
   `scenarios/phase2/inc1/narrow_following_v2.json5` (Increment 1); any other
   wheeled `following` → the Increment 5 pairwise fixture, with
   `scenarios/phase2/inc3/heavy_following_v2.json5` as Increment 3's heavy-mode
   fixture.
5. car-car `crossing` → `scenarios/benchmarks/perpendicular_conflict_v1.json5`
   and `scenarios/benchmarks/offset_junction_v1.json5` (Phase 1); car-pedestrian
   `crossing` → `scenarios/benchmarks/pedestrian_crossing_v1.json5`,
   `scenarios/benchmarks/mixed_interaction_v1.json5`, and
   `scenarios/benchmarks/four_leg_signal_v1.json5` (Phase 1); narrow `crossing` →
   `scenarios/phase2/inc1/narrow_crossing_v2.json5` (Increment 1); articulated
   `crossing` → `scenarios/phase2/inc3/articulated_conflict_v2.json5`
   (Increment 3); any other `crossing` → the Increment 5 pairwise fixture.
6. narrow-narrow `overtaking` →
   `scenarios/phase2/inc2/narrow_passing_v2.json5`; motor/articulated overtaking
   a narrow mode → `scenarios/phase2/inc2/motor_passing_narrow_v2.json5`;
   motor/articulated overtaking a motor/articulated mode →
   `scenarios/phase2/inc2/motor_lane_change_v2.json5` (Increment 2).
7. a pair containing `bicycle` or `scooter` under `opposing` (wrong-way) →
   `scenarios/phase2/inc2/narrow_wrong_way_v2.json5` (Increment 2).
8. any remaining supported cell → the Increment 5 pairwise fixture
   `scenarios/phase2/inc5/pair_<a>__<b>__<family>_v2.json5`.

The exact file names for the planned paths are the owning increment's to
finalize; this matrix fixes the cell, the fixture *slug*, and the increment, and
a later increment revises this document when it checks a path in.

## 8. Pairwise matrix

Rows are the 28 unordered mode pairs (21 cross-mode plus 7 same-mode), in
`passenger_car, pedestrian, bicycle, scooter, bus, rigid_truck,
tractor_semitrailer` order. Columns are the eight families. Each cell is
`S:<class>` (supported), `I:<reason>` (impossible), or `D:<increment>`
(deferred). Every supported cell resolves through §5 and §7.4.

| pair | following | crossing | merging | overtaking | head-on / opposing | shared-space | stop-service | unknown |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| passenger_car__passenger_car | S:CC-FOLLOW | S:CC-CROSS | S:CC-MERGE | S:CC-OVERTAKE | S:CC-OPPOSE | I:WWS | I:NOBUS | S:CC-UNKNOWN |
| passenger_car__pedestrian | I:FD | S:CC-CROSS | I:FD | I:OP | I:FD | S:CC-SHARED | I:NOBUS | S:CC-UNKNOWN |
| passenger_car__bicycle | S:CC-FOLLOW | S:CC-CROSS | S:CC-MERGE | S:CC-OVERTAKE | S:CC-OPPOSE | I:WWS | I:NOBUS | S:CC-UNKNOWN |
| passenger_car__scooter | S:CC-FOLLOW | S:CC-CROSS | S:CC-MERGE | S:CC-OVERTAKE | S:CC-OPPOSE | I:WWS | I:NOBUS | S:CC-UNKNOWN |
| passenger_car__bus | S:CC-FOLLOW | S:CC-CROSS | S:CC-MERGE | S:CC-OVERTAKE | S:CC-OPPOSE | I:WWS | S:CC-STOP | S:CC-UNKNOWN |
| passenger_car__rigid_truck | S:CC-FOLLOW | S:CC-CROSS | S:CC-MERGE | S:CC-OVERTAKE | S:CC-OPPOSE | I:WWS | I:NOBUS | S:CC-UNKNOWN |
| passenger_car__tractor_semitrailer | S:CC-FOLLOW | S:CC-CROSS | S:CC-MERGE | S:CC-OVERTAKE | S:CC-OPPOSE | I:WWS | I:NOBUS | S:CC-UNKNOWN |
| pedestrian__pedestrian | S:CC-FOLLOW | S:CC-CROSS | S:CC-MERGE | I:OP | S:CC-OPPOSE | S:CC-SHARED | I:NOBUS | S:CC-UNKNOWN |
| pedestrian__bicycle | I:FD | S:CC-CROSS | I:FD | I:OP | I:FD | S:CC-SHARED | I:NOBUS | S:CC-UNKNOWN |
| pedestrian__scooter | I:FD | S:CC-CROSS | I:FD | I:OP | I:FD | S:CC-SHARED | I:NOBUS | S:CC-UNKNOWN |
| pedestrian__bus | I:FD | S:CC-CROSS | I:FD | I:OP | I:FD | S:CC-SHARED | S:CC-STOP | S:CC-UNKNOWN |
| pedestrian__rigid_truck | I:FD | S:CC-CROSS | I:FD | I:OP | I:FD | S:CC-SHARED | I:NOBUS | S:CC-UNKNOWN |
| pedestrian__tractor_semitrailer | I:FD | S:CC-CROSS | I:FD | I:OP | I:FD | S:CC-SHARED | I:NOBUS | S:CC-UNKNOWN |
| bicycle__bicycle | S:CC-FOLLOW | S:CC-CROSS | S:CC-MERGE | S:CC-OVERTAKE | S:CC-OPPOSE | I:WWS | I:NOBUS | S:CC-UNKNOWN |
| bicycle__scooter | S:CC-FOLLOW | S:CC-CROSS | S:CC-MERGE | S:CC-OVERTAKE | S:CC-OPPOSE | I:WWS | I:NOBUS | S:CC-UNKNOWN |
| bicycle__bus | S:CC-FOLLOW | S:CC-CROSS | S:CC-MERGE | S:CC-OVERTAKE | S:CC-OPPOSE | I:WWS | S:CC-STOP | S:CC-UNKNOWN |
| bicycle__rigid_truck | S:CC-FOLLOW | S:CC-CROSS | S:CC-MERGE | S:CC-OVERTAKE | S:CC-OPPOSE | I:WWS | I:NOBUS | S:CC-UNKNOWN |
| bicycle__tractor_semitrailer | S:CC-FOLLOW | S:CC-CROSS | S:CC-MERGE | S:CC-OVERTAKE | S:CC-OPPOSE | I:WWS | I:NOBUS | S:CC-UNKNOWN |
| scooter__scooter | S:CC-FOLLOW | S:CC-CROSS | S:CC-MERGE | S:CC-OVERTAKE | S:CC-OPPOSE | I:WWS | I:NOBUS | S:CC-UNKNOWN |
| scooter__bus | S:CC-FOLLOW | S:CC-CROSS | S:CC-MERGE | S:CC-OVERTAKE | S:CC-OPPOSE | I:WWS | S:CC-STOP | S:CC-UNKNOWN |
| scooter__rigid_truck | S:CC-FOLLOW | S:CC-CROSS | S:CC-MERGE | S:CC-OVERTAKE | S:CC-OPPOSE | I:WWS | I:NOBUS | S:CC-UNKNOWN |
| scooter__tractor_semitrailer | S:CC-FOLLOW | S:CC-CROSS | S:CC-MERGE | S:CC-OVERTAKE | S:CC-OPPOSE | I:WWS | I:NOBUS | S:CC-UNKNOWN |
| bus__bus | S:CC-FOLLOW | S:CC-CROSS | S:CC-MERGE | S:CC-OVERTAKE | S:CC-OPPOSE | I:WWS | S:CC-STOP | S:CC-UNKNOWN |
| bus__rigid_truck | S:CC-FOLLOW | S:CC-CROSS | S:CC-MERGE | S:CC-OVERTAKE | S:CC-OPPOSE | I:WWS | S:CC-STOP | S:CC-UNKNOWN |
| bus__tractor_semitrailer | S:CC-FOLLOW | S:CC-CROSS | S:CC-MERGE | S:CC-OVERTAKE | S:CC-OPPOSE | I:WWS | S:CC-STOP | S:CC-UNKNOWN |
| rigid_truck__rigid_truck | S:CC-FOLLOW | S:CC-CROSS | S:CC-MERGE | S:CC-OVERTAKE | S:CC-OPPOSE | I:WWS | I:NOBUS | S:CC-UNKNOWN |
| rigid_truck__tractor_semitrailer | S:CC-FOLLOW | S:CC-CROSS | S:CC-MERGE | S:CC-OVERTAKE | S:CC-OPPOSE | I:WWS | I:NOBUS | S:CC-UNKNOWN |
| tractor_semitrailer__tractor_semitrailer | S:CC-FOLLOW | S:CC-CROSS | S:CC-MERGE | S:CC-OVERTAKE | S:CC-OPPOSE | I:WWS | I:NOBUS | S:CC-UNKNOWN |

The `unknown` column is pair-independent: the classifier may emit `unknown` for
any pair, and the single Increment 6 ambiguity fixture (`T-CLASS`) checks that an
ambiguous trajectory is preserved as `unknown` rather than forced into a
neighbouring family. It appears once per pair so the row count stays complete.

## 9. Mixed-mode cells

| Cell | Fixture | Quantity compared | Tolerance | Baseline | Presets |
| --- | --- | --- | --- | --- | --- |
| `MIX-1` all seven modes, nominal demand, one bus stop | `scenarios/phase2/inc7/mixed_release_v2.json5` | agent/passenger/route conservation residual; per-mode-pair minimum separation | `T-CONS`, `T-SEP` | Increment 7 declared release experiment spec | S, f |
| `MIX-2` all seven modes, saturated demand | `scenarios/phase2/inc7/mixed_release_saturated_v2.json5` | conservation residual; per-mode-pair minimum separation; queue/delay bounds outside a labelled density range | `T-CONS`, `T-SEP`, plus the Increment 5 `density_ramp_v2` operating-range label | Increment 7 declared release spec and Increment 5 density ramp | S, f |
| `MIX-3` full mixed-mode conclusion stability | the `MIX-1`/`MIX-2` release comparison report | material-conclusion verdict flips between `S` and `f` | `T-STABLE` | Increment 7 declared release spec, demand bank, and seed bank | S, f |

`MIX-1` and `MIX-2` are scenario-only variants of one compact intersection, as
`PHASE_2_PLAN.md` Increment 7 requires; `MIX-3` is the comparison over their
common-random-number seed bank.

## 10. Coverage audit

A reader confirms this checklist against the tables above. Counts are also in
`docs/benchmark-matrix.json`.

- **Modes:** 7/7 present in §4.1, each with a disposition (`supported`) and an
  independent cell class.
- **Pairs:** 28/28 present in §8 — `C(7, 2)` = 21 cross-mode plus 7 same-mode.
  Each pair appears exactly once as a row.
- **Families:** 8/8 present in §4.2 and §8 — the seven plan families plus
  `unknown`. Each family appears exactly once as a column.
- **Cells:** 28 x 8 = 224, each with exactly one disposition.
- **Dispositions:** supported 157, impossible 67, deferred 0.
- **Family disposition counts:**
  following 22 S / 6 I; crossing 28 S / 0 I; merging 22 S / 6 I;
  overtaking 21 S / 7 I; head-on/opposing 22 S / 6 I; shared-space 7 S / 21 I;
  stop-service 7 S / 21 I; unknown 28 S / 0 I.
- **Row check:** every pair row sums to 8 dispositions.
- **Column check:** every family column sums to 28 dispositions.
- **Class check:** every `S:<class>` in §8, §4.1, and §9 is defined in §5, and
  every class's tolerances are defined in §6.
- **Baseline check:** every tolerance in §6 names a baseline, and every baseline
  is the Phase 1 baseline (car/pedestrian cells) or a named increment reference.
- **Numeric check:** every tolerance is a bound on a metric or geometric
  quantity (`<=`, `>=`, or `= 0`); none is a golden-file or trace-hash equality.
- **Impossible check:** every `I:<code>` in §8 is defined in §4.3.

### 10.1 Why no cell is deferred

Increment 5's deliverable is "pairwise fixtures for every material mode pair and
interaction family", and Increment 7 adds the full pairwise matrix. Every cell
that is physically possible is therefore owned by a named fixture, so it is
`supported`; a cell that is not possible is `impossible`. No cell is
`deferred`. The `D:<increment>` form is retained so a future revision can defer
a specific cell with a named owner without changing this document's grammar.

## 11. Deferred beyond this taxonomy

Phase 2 explicitly defers the following. They are not cells of this taxonomy and
are named here so the deferral boundary is auditable: motorcycle dynamics, rider
balance, bicycle lean, falls, dooring, parking maneuvers, curb activity;
individual passenger trip chains, transfers, wheelchair boarding, and vehicle
interiors; autonomous-vehicle stacks and V2X; perception-error models beyond the
Phase 1 boundary; field calibration, statistical safety models, and rare-event
acceleration; multi-block routing, design optimization, GPU simulation, and
distributed execution. See `PHASE_2_PLAN.md`, *Phase 2 product slice* /
*Deferred*.

## Sources

- `PHASE_2_PLAN.md` — mode list, motion and interaction models, interaction
  taxonomy, validation ladder, fidelity presets, delivery increments.
- `docs/schema-v2-contract.md` — what version 2 defers to which increment.
- `baselines/phase1/baseline.json`, `baselines/phase1/README.md` — the Phase 1
  baseline and its presets.
- `scenarios/**` — the existing Phase 1 fixtures.
- `DEF-005-metric-definition-v2` — the reported metric names and reporting
  constants the tolerances bound.
