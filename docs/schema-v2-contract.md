# Schema version 2 contract

This is the authoritative definition of scenario schema version 2: the
Increment 0 subset and the deterministic mapping from a version-1 document to a
normalized version-2 document, extended in place by the *Increment 1 additions*
and *Increment 2 additions* sections at the end. Later increments cite this
document and add to the shapes it names; they do not reinterpret them.

`PHASE_2_PLAN.md` fixes the intent. This contract fixes the exact version-2
source shapes, the version-negotiation rule, the version-1 field mapping, the
manifest provenance fields, and the explicit deferrals, so the version-2
implementation nodes (`TAS-062` … `TAS-065`) can be written against one settled
target.

## Authority and scope

- `PHASE_2_PLAN.md` sections *Schema version 2*, *Facilities, paths, and free
  space*, *Agent composition*, *Controller boundary*, and *Increment 0* are the
  source of intent; this document is the source of the exact subset Increment 0
  implements.
- Increment 0 covers the schema, the version-1 migration, and provenance only.
  The simulator still runs the Phase 1 modes; version 2 does not change
  compiled behavior in this increment.
- Existing seams this contract pins:
  `crates/tangle-model/src/source.rs:17` (`SUPPORTED_SCHEMA_VERSION`),
  `crates/tangle-model/src/validate.rs:367` (`check_schema_version`, the version
  check that emits `E_SCHEMA_VERSION`), `crates/tangle-model/src/compiled.rs:2440`
  (compiled `schema_version`), and `schemas/scenario-source.schema.json`.

## Version negotiation

- `SUPPORTED_SCHEMA_VERSION` becomes `2`. A normalized version-2 document is the
  only input the compiler consumes.
- The reader parses `schema_version: 1` and `schema_version: 2`. A version-1
  document is routed through the explicit migration (next section) before it
  reaches validation or compilation; it is never compiled by the version-2
  reader directly.
- Any version other than `1` or `2` is rejected with the stable diagnostic
  `E_SCHEMA_VERSION` (`crates/tangle-model/src/validate.rs:205`). The reader
  never guesses a version.
- A missing version-2 field is never silently defaulted. Every version-2 field
  is either required or explicitly optional in the schema; an absent required
  field is a parse or validation failure, not a fallback to a Phase 1 default.
  The migration materializes every resolved value (including Phase 1 defaults)
  into the normalized document, so no default is applied at run time.
- The simulator never changes behavior through hidden parser defaults: it
  consumes normalized compiled data only.

## Version-2 source shapes

Version 2 adds the shapes below beyond the version-1 document. Each row names
the shape, its purpose, and the increment that populates it. Shapes marked
*deferred* are fixed by version 2 but not populated by Increment 0; their exact
field sets are settled by the increment that fills them.

| Shape | Purpose | Populated in |
| --- | --- | --- |
| `mode_templates[]` | Named, validated bundles of body, motion, tactical capabilities, access, occupancy, and profile distributions, so modes are authored data rather than code branches. | Increment 0 (`passenger_car`, `pedestrian`); bicycle/scooter Increment 1; bus/rigid truck/articulated Increment 3 |
| `movements[].direction` | Explicit nominal traversal direction of a movement's reference path, replacing the direction version 1 left implicit in the portal order. | Increment 0 |
| `demand[]` (mode-tagged) | Demand by portal, mode template, destination/route distribution, and time interval; the single demand shape for every mode. | Increment 0 (car and Phase 1 pedestrian); transit demand Increment 4; group demand Increment 5 |
| `facilities[]` | Continuous-width traversable regions and reference paths with usable width, nominal direction, mode access, lateral-use policy, and speed policy. | **Deferred to Increment 1** |
| Facility connectors | Which facilities and traversal directions a connector joins, for route planning and lane changes. | **Deferred to Increment 1** |
| `bus_stops[]` / transit service | Bus stops, waiting areas, service rules, capacity, and boarding/alighting distributions. | **Deferred to Increment 4** |
| `pedestrian_groups[]` | Pedestrian group distributions (cohesion, split/rejoin), as a relationship among ordinary pedestrian agents. | **Deferred to Increment 5** |
| Permissions and obligations | Explicitly authored nominal-direction, lane-use, overtaking, crossing, and stop-service permissions/obligations. | **Deferred** (lane use and overtaking Increment 1–2, stop service Increment 4) |
| Mode templates for other motion/body families | Bicycle, scooter, bus, rigid-truck, articulated-tractor bodies and their dynamics/controllers; capsule and articulated-chain bodies; transit occupancy; group social state. | **Deferred** (see the row for `mode_templates`) |

### Increment 0 shapes, concretely

#### `mode_templates`

Each entry is one validated bundle. Increment 0 populates exactly the two Phase 1
modes; the shapes below are the Increment 0 field set.

- `id` — stable template id, unique across authored objects.
- `body` — `kind` (`box` or `circle`; `capsule` and `articulated_chain`
  deferred), plus the body distributions that fit the kind:
  `length_m`/`width_m` for a `box`, `radius_m` for a `circle` (each a
  `{ min, max }` range).
- `motion` — `holonomic_walking` or `single_body_wheeled`
  (`articulated_wheeled` deferred).
- `tactics` — the supported tactical capabilities; Increment 0 uses
  `follow`, `stop`, and `yield`. `change_lane`, `overtake`, `pass`,
  `reverse_direction`, and `serve_stop` are deferred.
- `access` — `facility_kinds`, the traversable object kinds the mode may use.
  Increment 0 kinds are `path`, `crossing`, and `waiting_area`; facility kinds,
  nominal-direction permissions, and speed policy are deferred.
- `occupancy` — `operator_only` in Increment 0; `fixed` and `transit` are
  deferred.
- `profiles` — profile distributions keyed by parameter name, validated against
  the body/motion family. A `box` + `single_body_wheeled` template requires
  `speed_mps`, `max_accel_mps2`, `comfortable_brake_mps2`, `time_gap_s`, and
  `compliance`; a `circle` + `holonomic_walking` template requires `speed_mps`
  and `compliance`.

Increment 0 normalized templates:

```json5
mode_templates: [
  {
    id: 'passenger_car',
    body: { kind: 'box', length_m: { min: 4.0, max: 5.2 }, width_m: { min: 1.7, max: 2.0 } },
    motion: 'single_body_wheeled',
    tactics: ['follow', 'stop', 'yield'],
    access: { facility_kinds: ['path'] },
    occupancy: 'operator_only',
    profiles: {
      speed_mps: { min: 9.0, max: 15.0 },
      max_accel_mps2: { min: 1.2, max: 2.5 },
      comfortable_brake_mps2: { min: 2.0, max: 3.5 },
      time_gap_s: { min: 1.0, max: 2.0 },
      compliance: { min: 1.0, max: 1.0 },
    },
  },
  {
    id: 'pedestrian',
    body: { kind: 'circle', radius_m: { min: 0.20, max: 0.30 } },
    motion: 'holonomic_walking',
    tactics: ['follow', 'stop', 'yield'],
    access: { facility_kinds: ['path', 'crossing', 'waiting_area'] },
    occupancy: 'operator_only',
    profiles: { speed_mps: { min: 1.0, max: 1.6 }, compliance: { min: 1.0, max: 1.0 } },
  },
]
```

#### `movements[].direction`

Each version-2 movement connector names its reference path and its nominal
traversal direction explicitly. `direction` is `forward` or `reverse` relative
to the authored vertex order of its path. Version 1 left this implicit in the
`from`/`to` portal order; version 2 makes it authored so the normalized document
is self-describing. Every other movement field is carried forward unchanged.

#### `demand`

The version-2 demand shape is mode-tagged and covers both version-1 demand
generators and the walking-skeleton population:

- `id` — stable id.
- `mode` — the mode-template id this demand produces.
- `spawn` — one of:
  - `rate`: `portal`, `rate_per_hour`, `interval_s { start_s, end_s }` (with a
    null `end_s` meaning unbounded), and a `choice` that is either
    `movements [{ movement, weight }]` (vehicle modes) or
    `routes [{ route, weight }]` (pedestrian modes); or
  - `population`: `path`, `count`, `speed_mps`, `spacing_m` — a fixed initial
    population placed at `t = 0`, reproducing the Phase 1 walking skeleton
    exactly rather than approximating it with a rate.

## Deterministic version-1 → normalized version-2 mapping

The migration is a pure function of the version-1 document: no filesystem,
clock, or random input (it is fixed by `TAS-063`). For every valid version-1
document the mapping below is total and deterministic. Fields not named here are
carried forward unchanged: `id`, `coordinate_system`, `paths`, `portals`,
`boundaries`, `regions`, `movements` (plus the explicit `direction`), `crossings`,
`waiting_areas`, `pedestrian_routes`, `conflict_regions`, `rules`, and `signals`.

The version-1 top-level fields `profiles`, `pedestrian_profiles`, `population`,
`demand`, and `pedestrian_demand` are absorbed into `mode_templates` and
`demand`; they are not fields of a normalized version-2 document.

### Population and profile fields → a passenger-car mode template

The migration always emits the Increment 0 `passenger_car` and `pedestrian`
templates, materializing the resolved version-1 profiles (declared or default).

1. **Body, from profiles.** `profile.length_m` → `body.length_m`,
   `profile.width_m` → `body.width_m`; `pedestrian_profile.radius_m` →
   the pedestrian template's `body.radius_m`.
2. **Dynamics and behavior, from profiles.** `profile.speed_mps` →
   `profiles.speed_mps`, `profile.max_accel_mps2` →
   `profiles.max_accel_mps2`, `profile.comfortable_brake_mps2` →
   `profiles.comfortable_brake_mps2`, `profile.time_gap_s` →
   `profiles.time_gap_s`, and `profile.compliance` → `profiles.compliance`.
   The pedestrian template takes `pedestrian_profile.speed_mps` and
   `pedestrian_profile.compliance`.
3. **Population overrides the car template's body and speed.** When the source
   uses the walking-skeleton population (it declares no `demand` and no
   `pedestrian_demand`), the population governs the vehicle body and speed, so
   the migration sets the passenger-car template's `body.length_m` and
   `body.width_m` to the constant `{ min: vehicle_length_m, max: vehicle_length_m }`
   and `{ min: vehicle_width_m, max: vehicle_width_m }`, and
   `profiles.speed_mps` to the constant
   `{ min: vehicle_speed_mps, max: vehicle_speed_mps }`. The population does not
   constrain acceleration, braking, time gap, or compliance, so those keep the
   values from step 2. When the source instead declares demand, the population
   is unused and the template keeps the profile values from steps 1–2.
4. The migration copies `tactics`, `access`, `motion`, and `occupancy` from the
   fixed Increment 0 template literals above; nothing is sampled or defaulted at
   parse time.

### Path plus portal/movement direction → a directional reference path

For each version-1 `MovementSource`, the movement's existing portal order fixes
one direction on its path, and the migration writes it as `movements[].direction`:

- `from` portal attached to the path's `start` → `direction: 'forward'`;
- `from` portal attached to the path's `end` → `direction: 'reverse'`.

Version-1 validation already guarantees the two portals attach to opposite ends
of the movement's path (`E_MOVEMENT_SELF_LOOP`,
`E_MOVEMENT_PORTAL_PATH_MISMATCH`, `E_PORTAL_DUPLICATE_END`), so the derivation
is total and single-valued. The compiled directional reference path — the
`(path geometry, direction)` pair that `PHASE_2_PLAN.md` calls the directional
facility — is materialized per movement from this field. Increment 0 adds no
authored facility object: the facility's region geometry, usable width, mode
access, lateral-use policy, speed policy, and connectors are deferred to
Increment 1, which attaches them to the reference path this mapping already
orients. A path referenced by no movement keeps its authored vertex order as its
compiled nominal direction, matching Phase 1.

A version-1 `PedestrianRouteSource` is carried forward unchanged; its direction
stays implicit in its `from`/`to` portal order, as in version 1.

### `PopulationSource` / `DemandSource` → version-2 demand

- `demand[]` entries (vehicle): `mode: 'passenger_car'`, and a `rate` spawn
  carrying `portal`, `rate_vph` as `rate_per_hour`, `choice.movements` from
  `routes`, and an unbounded `interval_s` (`start_s: 0.0`, `end_s: null`) that
  reproduces the version-1 whole-run demand.
- `pedestrian_demand[]` entries: `mode: 'pedestrian'`, and a `rate` spawn
  carrying `portal`, `rate_pph` as `rate_per_hour`, and `choice.routes` from
  `routes`. The version-1 vehicle/pedestrian demand split disappears; both
  become mode-tagged version-2 demand.
- `population` (used exactly when the source declares no demand of either mode):
  `mode: 'passenger_car'`, and a `population` spawn carrying
  `count: vehicle_count`, `speed_mps: vehicle_speed_mps`,
  `spacing_m: vehicle_spacing_m`, and a `path` naming the guide path the
  walking skeleton populates. Its body and speed feed the passenger-car
  template as described above.

### Canonical normalized form

The normalized version-2 document is canonical JSON: 2-space indentation,
structure field order, and a trailing newline (the same form
`baseline::Baseline::to_pretty_json` uses). The `migrate` command writes exactly
these bytes, and the normalized hash below is the SHA-256 of exactly these bytes.

## Migration version

The version-1 → version-2 transform is migration version `1`, exposed as
`MIGRATION_VERSION` in `tangle-model`. A source that is already version 2 is
recorded as migration version `0` (no migration applied). Every transform that
changes the mapping increments the number, so a run can be attributed to the
exact transform that produced it.

## Provenance

Four fields identify the inputs of a run. They live on the shared
`ScenarioProvenance` struct (`apps/tangle-cli/src/baseline.rs:88`), which is
embedded as `scenario` in both the run manifest
(`RunManifest` at `apps/tangle-cli/src/run_dir.rs:301`) and the checked-in
baseline manifest (`baselines/phase1/baseline.json`).

| Field | Meaning |
| --- | --- |
| `schema_version` | Source schema version the input document was authored against (`1` or `2`); existing field. |
| `content_sha256` | SHA-256 of the raw source bytes as read, comments and whitespace included; existing field. |
| `normalized_sha256` | SHA-256 of the canonical normalized version-2 bytes the run consumed (migration output for a migrated run, canonical re-serialization for a native version-2 run). |
| `migration_version` | The migration transform applied: `1` for the version-1 → version-2 transform, `0` when the source was already version 2. |

`normalized_sha256` and `migration_version` are new in Increment 0 and are
populated from the actual bytes and the transform that produced them, never from
a hardcoded default. Recording these four fields together lets a later trace
change be attributed to a source edit, a migration change, or the kernel.

## Version-2 validation rules (Increment 0)

- The schema (and its generated JSON Schema) declares every version-2 field as
  required or optional; no field falls back to a Phase 1 default.
- Unknown schema versions fail with `E_SCHEMA_VERSION`.
- A mode template is rejected when its components cannot coexist — for example
  transit dwell without capacity, or articulation parameters on a holonomic
  body — with a stable diagnostic naming the authored template id.
- A demand source references a declared mode template and a declared portal or
  path; its `choice` matches the mode's motion family.
- A movement's `from`/`to` portals both attach to its named path (the existing
  version-1 rule), and `direction` is one of `forward` / `reverse`.

## What version 2 defers to later increments

- **Facilities.** Authored continuous-width traversable regions with usable
  width, mode access, lateral-use policy, speed policy, and free lateral motion
  are deferred to Increment 1. Increment 0 provides only the directional
  reference path described above.
- Facility connectors that join facilities across traversal directions are
  deferred to Increment 1.
- **Transit stops.** Bus stops, waiting areas with service rules, capacity, and
  boarding/alighting distributions, and dwell service are deferred to
  Increment 4.
- **Pedestrian groups.** Group distributions, cohesion, split/rejoin, and the
  named group-decision stream are deferred to Increment 5. Pedestrian agents
  remain independent in Increment 0.
- Non-Phase-1 `mode_templates` (bicycle, scooter, bus, rigid truck, articulated
  tractor-semitrailer), capsule and articulated-chain bodies, transit occupancy,
  and group social state are deferred to Increments 1 and 3.
- Authored permissions and obligations for nominal direction, lane use,
  overtaking, crossings, and stop service are deferred; Increment 0 uses only
  the existing `free`, `yield`, `stop`, and `signal` rules.
- A general navigation mesh is not required by Phase 2 and stays deferred.

## Increment 1 additions: facilities and narrow modes

This section fixes the version-2 source shapes and compiled semantics for
`PHASE_2_PLAN.md` Increment 1 (*Facilities and narrow modes*). It **extends the
Increment 0 contract above in place**: it adds shapes and fields to the
version-2 document and enumerates them here. It does not reinterpret or change
any Increment 0 shape, field meaning, version-negotiation rule, or provenance
field. Every row the Increment 0 document marks *deferred* that Increment 1 now
owns is reconciled in *Reconciling the Increment 0 deferrals* below; the rows
Increment 1 does not own stay deferred with their later incumbents named.

Increment 1 is still schema version `2`. No new schema version is introduced,
the version-negotiation rule is unchanged, and no new field is silently
defaulted: every Increment 1 field is required or explicitly optional in the
schema, exactly as Increment 0 requires. Authored objects stay identified by
stable strings and compile to dense IDs; a compiled object keeps its authored
string for diagnostics.

### Authority and scope

- `PHASE_2_PLAN.md` sections *Schema version 2*, *Facilities, paths, and free
  space*, *Bicycles and scooters*, and *Increment 1* are the source of intent;
  this section is the source of the exact shapes Increment 1 implements.
- This section is the contract the Increment 1 leaves code against:
  [[TAS-074-compile-version-2-facility-and-connector-shapes]] (source and
  compiled geometry), [[TAS-075-add-increment-1-facility-and-narrow-mode-validat]]
  (validation), [[TAS-076-add-bicycle-and-scooter-mode-templates-narrow-bo]]
  (narrow templates and components), [[TAS-077-wire-narrow-mode-spawning-and-shared-stage-bicyc]]
  (controllers and spawning), [[TAS-078-check-in-narrow-mode-isolated-fixtures-and-tests]]
  (fixtures), and [[TAS-079-prove-narrow-mode-reproducibility-and-independen]]
  (reproducibility). It is the extension of the Increment 0 contract owned by
  [[TAS-061-version-2-schema-contract]].
- Increment 1's checked evidence is class `CC-NARROW` in
  `docs/benchmark-matrix.md` (fixtures `narrow_isolated_straight_v2`,
  `narrow_isolated_curve_v2`; tolerances `T-RT`, `T-DIM`, `T-ENV`, `T-SPD`), and
  each narrow mode carries a model card per `docs/model-card-template.md`.
- Existing seams this section pins:
  `crates/tangle-model/src/source.rs:613` (`ModeBodySource`), `:663`
  (`FacilityKind`), `:788` (`AccessSource`), `:919` (`ModeTemplateSource`),
  `:1019` (`ScenarioSourceV2`); `crates/tangle-model/src/components.rs:285`
  (`NominalDirection`), `:299` (`SpeedPolicy`), `:325` (`AgentAccess`), `:93`
  (`AgentBody::Capsule`), `:163` (`AgentFamily::WheeledCapsule`), `:618`
  (`AgentBehaviorProfile`); `crates/tangle-model/src/validate.rs:1604`
  (`required_profile_params`); and `crates/tangle-model/src/mode_template.rs:297`
  (`compiled_access`).

### Increment 1 source shapes

| Shape | Purpose | Populated in |
| --- | --- | --- |
| `facilities[]` | A continuous traversable region plus an optional reference path, with usable width, nominal direction, mode access, lateral-use policy, and speed policy. | Increment 1 |
| `facility_connectors[]` | Directed joins between facilities across traversal directions, for route planning and lateral transitions. | Increment 1 |
| `permissions[]` | Authored nominal-direction, lane-use, overtaking, crossing, and stop-service permissions and obligations. | Shape fixed in Increment 1; `nominal_direction` Increment 1, `lane_use`/`overtake`/`crossing` Increment 2, `stop_service` Increment 4 |
| `mode_templates[]` narrow extensions | Capsule body, access nominal direction and speed policy, and the narrow wheeled profile parameter set, with the `bicycle` and `scooter` templates. | Increment 1 |

#### `facilities[]`

Each entry is one continuous traversable region:

- `id` — stable facility id, unique across all authored objects.
- `region` — id of a declared `regions[]` polygon the facility occupies. The
  polygon is the continuous traversable area; when a facility has a reference
  path, the polygon is the band the path runs through.
- `reference_path` — optional id of a declared `paths[]` polyline giving the
  facility local coordinates. Present for every constrained-wheeled facility;
  when omitted the facility exposes region geometry only and no `(s, d)` frame.
- `width_m` — the facility's usable traversable width in metres, measured
  across the reference path; finite and positive. It is the authored band a body
  must fit inside; the compiled usable lateral interval (below) subtracts the
  body envelope and the mode's lateral clearance from it.
- `nominal_direction` — `forward`, `reverse`, or `either`, relative to the
  authored vertex order of `reference_path`. `forward` and `reverse` require a
  `reference_path`; a facility with no reference path is `either`.
- `access` — `{ modes: [<mode-template id>, ...] }`: the mode templates
  permitted to use the facility. Non-empty; every id names a declared
  `mode_templates[]` entry.
- `lateral_use` — `shared` or `centered`: whether the usable lateral interval is
  one shared space an agent may occupy anywhere within, or whether agents hold
  the reference centerline. Increment 1 selects no lateral position (free
  lateral motion is Increment 2), so the policy is authored data the facility
  selector reads rather than an Increment 1 behavior.
- `speed_policy` — `{ limit_mps: <positive number> }`, or `{ limit_mps: null }`
  for no enforced limit beyond the mode's own motion limits. The null form
  mirrors the unbounded `interval_s.end_s` precedent; a missing field is a parse
  error, never a default. The effective limit on the facility is the more
  restrictive of this value and the permitting mode's `access.speed_policy`.

Normalized example:

```json5
facilities: [
  {
    id: 'bikeway_eastbound',
    region: 'bikeway_band',
    reference_path: 'bikeway_centerline',
    width_m: 2.0,
    nominal_direction: 'forward',
    access: { modes: ['bicycle', 'scooter'] },
    lateral_use: 'shared',
    speed_policy: { limit_mps: null },
  },
]
```

#### `facility_connectors[]`

Each connector joins an upstream facility traversal to a downstream facility
traversal, so a connector is a directed edge in the facility graph. Both named
facilities have a `reference_path`:

- `id` — stable connector id, unique across all authored objects.
- `from` — `{ facility, direction }`: the facility the connector leaves and the
  traversal direction it leaves along (`forward` or `reverse`). A `forward`
  traversal leaves at the reference path's `end`; a `reverse` traversal leaves
  at its `start`.
- `to` — `{ facility, direction }`: the facility the connector enters and the
  traversal direction it enters along (`forward` or `reverse`). A `forward`
  traversal enters at the reference path's `start`; a `reverse` traversal enters
  at its `end`.

The connector's geometry is the coincidence of `from`'s leaving end and `to`'s
entering end; continuity is a validation rule (below). `direction` is
`forward`/`reverse` (never `either`), because a connector joins one physical
traversal. Portals and movements continue to route an agent into and out of the
facility graph at the periphery.

#### `permissions[]`

The permission/obligation shape is one authored deontic statement:

- `id` — stable id, unique across all authored objects.
- `kind` — `nominal_direction`, `lane_use`, `overtake`, `crossing`, or
  `stop_service`.
- `holder` — the mode-template id the statement binds.
- `target` — the id of the object the statement is about; the object kind is
  fixed by `kind`.
- `effect` — `permit`, `prohibit`, or `obligate`.

Owning increment per kind:

| `kind` | Meaning | Target object | Populated in |
| --- | --- | --- | --- |
| `nominal_direction` | Whether `holder` may, may not, or must travel against a facility's or movement's nominal direction. | `facility` or `movement` | Increment 1 |
| `lane_use` | Permitted lateral position / lane use on a facility. | `facility` | Increment 2 |
| `overtake` | Permission or obligation to overtake or pass on a facility. | `facility` | Increment 2 |
| `crossing` | Permission or obligation at a crossing. | `crossing` | Increment 2 |
| `stop_service` | Stop-service obligation and priority. | `bus_stop` | Increment 4 |

Increment 1 fixes the shape in full and populates only `nominal_direction`. A
permission statement is authored data that compiles into a mode's direction
permission; it never makes a physically impossible route possible, and physical
possibility (the connector graph and facility geometry) is decided separately.

#### `mode_templates` narrow extensions

Increment 1 extends `mode_templates` (whose Increment 0 field set is above) for
the narrow wheeled family:

- `body.kind: 'capsule'` — a segment of `length_m` with a constant `radius_m`,
  each a `{ min, max }` metre range, mapping to the existing
  `AgentBody::Capsule` and the `wheeled_capsule` agent family. `box` and
  `circle` keep their Increment 0 meaning; `articulated_chain` stays deferred.
- `access.facility_kinds` gains one kind, `facility`, naming the continuous
  region facilities above. A `bicycle` or `scooter` template's
  `access.facility_kinds` is `['facility']`. The Increment 0 kinds `path`,
  `crossing`, and `waiting_area` keep their meaning.
- `access.nominal_direction` — `forward`, `reverse`, or `either`: the direction
  the mode may travel on its facilities, mapping to
  `AgentAccess::nominal_direction` (Increment 0 fixed this component as
  `either` because nothing authored it; Increment 1 reads it).
- `access.speed_policy` — `{ limit_mps: <positive number> | null }`, mapping to
  `AgentAccess::speed_policy`.
- The `capsule` + `single_body_wheeled` profile parameter set. Alongside the
  Increment 0 wheeled parameters `speed_mps`, `max_accel_mps2`,
  `comfortable_brake_mps2`, `time_gap_s`, and `compliance`, the narrow wheeled
  family requires `steering_rate_max_rad_s` (maximum steering/heading rate in
  radians per second, finite and positive) and `lateral_clearance_m` (preferred
  lateral clearance from the facility edge in metres, finite and non-negative,
  so a zero clearance is allowed). `lateral_clearance_m` uses a non-negative
  range rule distinct from the strictly-positive physical ranges, exactly as
  `compliance` uses its `[0, 1]` rule. Every other family keeps its Increment 0
  parameter set unchanged.

The Increment 1 normalized narrow templates (provisional engineering defaults,
not calibrated; the fixtures and model cards own the checked-in values):

```json5
mode_templates: [
  {
    id: 'bicycle',
    body: { kind: 'capsule', length_m: { min: 1.6, max: 1.9 }, radius_m: { min: 0.30, max: 0.40 } },
    motion: 'single_body_wheeled',
    tactics: ['follow', 'stop', 'yield'],
    access: { facility_kinds: ['facility'], nominal_direction: 'either', speed_policy: { limit_mps: null } },
    occupancy: 'operator_only',
    profiles: {
      speed_mps: { min: 3.5, max: 6.5 },
      max_accel_mps2: { min: 0.8, max: 1.5 },
      comfortable_brake_mps2: { min: 1.5, max: 3.0 },
      time_gap_s: { min: 0.8, max: 1.4 },
      steering_rate_max_rad_s: { min: 0.6, max: 1.2 },
      lateral_clearance_m: { min: 0.20, max: 0.50 },
      compliance: { min: 0.8, max: 1.0 },
    },
  },
  {
    id: 'scooter',
    body: { kind: 'capsule', length_m: { min: 1.0, max: 1.2 }, radius_m: { min: 0.25, max: 0.35 } },
    motion: 'single_body_wheeled',
    tactics: ['follow', 'stop', 'yield'],
    access: { facility_kinds: ['facility'], nominal_direction: 'either', speed_policy: { limit_mps: null } },
    occupancy: 'operator_only',
    profiles: {
      speed_mps: { min: 4.0, max: 7.5 },
      max_accel_mps2: { min: 1.0, max: 2.0 },
      comfortable_brake_mps2: { min: 2.0, max: 3.5 },
      time_gap_s: { min: 0.8, max: 1.4 },
      steering_rate_max_rad_s: { min: 0.8, max: 1.5 },
      lateral_clearance_m: { min: 0.20, max: 0.50 },
      compliance: { min: 0.6, max: 1.0 },
    },
  },
]
```

The field set above is the contract; the numbers are provisional engineering
defaults the model cards must label as such, not fitted to observations.

### Compiled coordinate contract

This is what the geometry leaf
([[TAS-074-compile-version-2-facility-and-connector-shapes]]) must expose for a
facility that has a reference path. A compiled facility carries its dense
`FacilityId` and its authored id string, the compiled region polygon, and:

- `length` — the reference arc length in metres
  (`CompiledPath::length`, `crates/tangle-model/src/compiled.rs:565`).
- `position_at(s)` — the world point at arc length `s`, clamped to
  `[0, length]` (`CompiledPath::position_at`,
  `crates/tangle-model/src/compiled.rs:570`).
- `heading_at(s)` — the tangent heading `theta(s)` in radians,
  counter-clockwise from the world x axis (`CompiledPath::heading_at`,
  `crates/tangle-model/src/compiled.rs:585`).
- `tangent_at(s)` — the unit tangent `(cos theta(s), sin theta(s))`.
- `normal_at(s)` — the unit left normal: the tangent rotated `+90` degrees,
  `(-sin theta(s), cos theta(s))`.
- `curvature_at(s)` — the signed curvature `kappa(s) = d theta / ds` in `1/m`,
  positive for a left-hand (counter-clockwise) turn. On a straight reference it
  is zero everywhere; the compiled reference defines a deterministic vertex
  rule for a polyline and reports it. The `(s, d) <-> world` round trip is exact
  within `T-RT` (`<= 1e-9 m`) on straight and curved references.
- usable lateral interval `[d_min(s), d_max(s)]` — the signed offsets `d` for
  which a body of lateral envelope width `h` (a capsule's or circle's
  `2 * radius_m`, a box's `width_m`) plus its lateral clearance `c` fits inside
  the facility band of width `W = width_m` centred on the reference path:
  `|d| + h/2 + c <= W/2`, so `d_min = -(W/2 - h/2 - c)` and
  `d_max = +(W/2 - h/2 - c)`. The interval is empty when `h + 2c > W`, which is
  the "facility too narrow for the eligible body" rejection.
- adjacency / connectors — the directed connectors that leave or enter the
  facility, each naming the connected `(facility, direction)`.
- the three direction properties, kept separate and never substituted for one
  another: the **nominal** direction (the authored `nominal_direction`), the
  **permitted** direction (a mode's `access.nominal_direction` plus any
  `permissions[]` statement that binds it), and the **physically possible**
  direction (whether a connected traversal actually exists in that direction in
  the connector graph).

The lateral offset `d` is positive to the left of the direction of travel, and
the world position is `p(s, d) = position_at(s) + d * normal_at(s)`. The
deterministic projection back is `s` from the nearest point on the reference
and `d = (q - position_at(s)) . normal_at(s)`, so a world pose projects to route
coordinates and reconstructs without drift.

### Increment 1 validation rules

`validate_v2` enforces the Increment 1 rules; the exact stable diagnostic codes
are owned by [[TAS-075-add-increment-1-facility-and-narrow-mode-validat]]:

- **Containment** — a facility region lies inside the traversable world.
- **Usable width** — a facility is wide enough for the largest body of every
  mode in its `access.modes`, plus that mode's lateral clearance.
- **Curvature against turning limits** — a facility's reference curvature does
  not exceed the turning limit of a mode permitted to use it.
- **Connector continuity** — a connector's `from` leaving end and `to` entering
  end coincide within tolerance.
- **Directional reachability** — a declared facility traversal is reachable
  through the connector graph, and its direction is physically realizable.
- **Mode-to-facility access** — every mode in a facility's `access.modes` lists
  the `facility` kind in its template `access.facility_kinds`, and a mode using
  a facility is permitted to.
- **Legal versus physically possible** — an illegal route (a `permissions[]`
  `prohibit`) is reported separately from a physically impossible route (no
  connector, an over-curved facility, or a too-narrow facility).
- **Spawn clearance** — a spawn point on a facility leaves room for the largest
  eligible body plus its lateral clearance.

A mode template that no longer serves a declared facility is rejected with a
stable diagnostic naming it; it is never silently dropped, and no rule falls
back to a Phase 1 or parser default.

### Reconciling the Increment 0 deferrals

This is how every row the Increment 0 document marks *deferred* that Increment 1
now owns is reconciled; rows it does not own stay deferred with their later
incumbents named. No Increment 0 shape, field meaning, version-negotiation rule,
or provenance field is changed.

| Increment 0 deferred row or sentence | Increment 1 disposition |
| --- | --- |
| `facilities[]` row — *Deferred to Increment 1* | Owned: `facilities[]` above fixes the field set. |
| Facility connectors row — *Deferred to Increment 1* | Owned: `facility_connectors[]` above. |
| Permissions and obligations row — *Deferred (lane use and overtaking Increment 1-2, stop service Increment 4)* | Shape fixed here as `permissions[]`; `nominal_direction` populated in Increment 1, `lane_use`/`overtake`/`crossing` in Increment 2, `stop_service` in Increment 4. |
| `mode_templates[]` row — *bicycle/scooter Increment 1* | Owned: the narrow `mode_templates` extensions above. |
| `mode_templates[]` row — *bus/rigid truck/articulated Increment 3* | Not owned; stays deferred. |
| `body.kind` — *`capsule` and `articulated_chain` deferred* | `capsule` owned in Increment 1; `articulated_chain` stays deferred (Increment 3). |
| `motion` — *`articulated_wheeled` deferred* | Not owned; stays deferred (Increment 3). |
| `tactics` — *`change_lane`, `overtake`, `pass`, `reverse_direction`, `serve_stop` deferred* | Not owned by Increment 1; `reverse_direction`/`change_lane`/`overtake`/`pass` in Increment 2 and `serve_stop` in Increment 4. The narrow templates keep `follow`, `stop`, `yield`. |
| `access` — *facility kinds, nominal-direction permissions, and speed policy are deferred* | Owned: the new `facility` kind, `access.nominal_direction`, and `access.speed_policy` fields above. (A facility also authors its own `speed_policy`; the effective limit is the more restrictive of the two.) |
| `occupancy` — *`fixed` and `transit` deferred* | Not owned; stays deferred (Increments 3 and 4). |
| Transit stops row — *Deferred to Increment 4* | Not owned; stays deferred. |
| Pedestrian groups row — *Deferred to Increment 5* | Not owned; stays deferred. |
| A general navigation mesh — *stays deferred* | Not owned; stays deferred. |

The four provenance fields (`schema_version`, `content_sha256`,
`normalized_sha256`, `migration_version`) are unchanged: Increment 1 adds no
provenance field and no schema version.

### What Increment 1 still defers

- Free lateral position selection, lane and facility transitions, gap
  prediction, manoeuvre commitment, overtaking and passing, close-pass
  evidence, and contextual wrong-way selection are Increment 2.
- Heavy (`bus`, `rigid_truck`) and articulated (`tractor_semitrailer`) bodies,
  dynamics, and controllers are Increment 3.
- Transit stops, transit occupancy, service, and boarding are Increment 4.
- Pedestrian groups and group social state are Increment 5.
- A general navigation mesh stays deferred; Phase 2 does not require one.

## Increment 2 additions: lateral motion, passing, and wrong-way travel

This section fixes the version-2 source shapes, the compiled policy and corridor
semantics, the maneuver state machine and policies, the wrong-way decision, and
the evidence contract for `PHASE_2_PLAN.md` Increment 2 (*Lateral motion,
passing, and wrong-way travel*). It **extends the Increment 0 and Increment 1
contract above in place**: every shape below is additive, no existing shape or
field changes meaning, and every new field is required or explicitly optional
with a documented absence rule, so a scenario that authors none of them behaves
exactly as Increment 1. Increment 2 is still schema version `2`: no new
`schema_version`, no new provenance field, and no silently defaulted field. The
rows Increment 1 deferred that Increment 2 now owns are reconciled in
*Reconciling the Increment 1 deferrals* below; the rows it does not own stay
deferred with their later incumbents named.

### Authority and scope

- `PHASE_2_PLAN.md` sections *Continuous lateral motion*, *Overtaking and close
  passing*, *Wrong-way movement*, *Safety, operations, and outputs*,
  *Reproducibility and fidelity*, *Decisions for implementation*, and
  *Increment 2* are the source of intent; this section is the source of the
  exact shapes Increment 2 implements.
- This section is the contract the Increment 2 leaves code against:
  [[TAS-084-parse-increment-2-lateral-and-rule-source-shapes]] (source shapes and
  the checked schema), [[TAS-085-compile-increment-2-policy-and-traversal-semantics]]
  (compiled policy and traversal), [[TAS-086-validate-increment-2-lateral-and-wrong-way-policy]]
  (validation), [[TAS-087-continuous-lateral-motion-and-gap-machinery]] with
  [[TAS-088-add-route-relative-lateral-agent-state]],
  [[TAS-089-integrate-bounded-single-body-steering]],
  [[TAS-090-predict-maneuver-corridors-and-clearance]], and
  [[TAS-091-resolve-gap-claims-and-maneuver-transitions]] (route-relative state,
  bounded steering, prediction, claims and transitions),
  [[TAS-092-passing-and-lane-transition-behavior]] with
  [[TAS-093-enable-same-facility-narrow-user-passing]],
  [[TAS-094-enable-motor-vehicle-overtaking-of-narrow-users]], and
  [[TAS-095-complete-lane-transitions-and-safe-aborts]] (passing and lane
  transitions), [[TAS-096-contextual-wrong-way-travel]] with
  [[TAS-097-make-contextual-wrong-way-decisions-reproducible]] and
  [[TAS-098-route-wrong-way-agents-through-ordinary-interactions]] (contextual
  wrong-way travel), [[TAS-099-increment-2-events-metrics-and-output]] with
  [[TAS-100-version-the-maneuver-event-and-trace-surface]],
  [[TAS-101-measure-close-passes-with-exact-clearance-evidence]], and
  [[TAS-102-record-wrong-way-intervals-and-disaggregated-metrics]] (events,
  metrics, output), and [[TAS-103-increment-2-acceptance-evidence-and-presenters]]
  with its evidence, card, performance, and presenter children. It is the
  extension of the Increment 1 contract owned by
  [[TAS-073-extend-the-version-2-schema-contract-with-increm]].
- Increment 2's checked evidence is the `CC-OVERTAKE` and `CC-OPPOSE` rows of
  `docs/benchmark-matrix.md` — tolerances `T-O1`, `T-O2`, `T-O3`, `T-H1`, `T-H2`
  and the fixtures `narrow_passing_v2`, `motor_passing_narrow_v2`,
  `motor_lane_change_v2`, and `narrow_wrong_way_v2` — together with the bicycle
  and scooter cards per `docs/model-card-template.md`.
- Existing seams this section pins, by name rather than by line because
  Increment 2 edits these files: `TacticKind`, `PermissionKind`,
  `PermissionEffect`, `PermissionSource`, `FacilitySource`, `LateralUse`,
  `FacilityDirection`, `AccessSource`, and `ScenarioSourceV2` in
  `crates/tangle-model/src/source.rs`; `TacticalCapability`,
  `TacticalCapabilities`, `AgentAccess`, `NominalDirection`, and
  `AgentBehaviorProfile` in `crates/tangle-model/src/components.rs`;
  `CompiledFacility`, `CompiledFacilityConnector`, `FacilityTraversal`,
  `UsableLateralInterval`, and `CompiledReferencePath` in
  `crates/tangle-model/src/compiled.rs`; `required_profile_params`,
  `mode_turning_limit_curvature`, `validate_permissions`, and
  `CONNECTOR_CONTINUITY_TOLERANCE_M` in `crates/tangle-model/src/validate.rs`;
  `compiled_tactic` and `compiled_profile` in
  `crates/tangle-model/src/mode_template.rs`; the version-1 → version-2
  transform in `crates/tangle-model/src/migrate.rs`; `Commitment`, `Tactic`,
  `TacticReason`, `TacticTarget`, `AbortCondition`, and `MotionCommand` in
  `crates/tangle-sim/src/stage.rs`; `Simulation`, `Simulation::step`,
  `StepOutput::events`, and `Simulation::snapshot` in
  `crates/tangle-sim/src/sim.rs`; `ControllerModels` and `VehicleController` in
  `crates/tangle-sim/src/controller.rs`; `IdmController` and `Constraint` in
  `crates/tangle-sim/src/control.rs`; `AgentId` and `AgentStore` in
  `crates/tangle-sim/src/agent.rs`; `VehicleProfile` in
  `crates/tangle-sim/src/profile.rs` and `NarrowProfile` in
  `crates/tangle-sim/src/narrow.rs`; `EVENT_VERSION`, `Event`, `EventKind`,
  `ViolationKind`, and `Event::order_key` in `crates/tangle-sim/src/event.rs`;
  `InteractionMetrics` and `tick_minimum_clearance_m` in
  `crates/tangle-sim/src/metrics.rs`; `body_clearance_m` and `CONTACT_EPSILON_M`
  in `crates/tangle-sim/src/query.rs`; `time_of_impact` and `band_entry` in
  `crates/tangle-sim/src/swept.rs`; the named streams and `uniform01` in
  `crates/tangle-sim/src/rng.rs`; `RunConfig` and `DEFAULT_STEP` in
  `crates/tangle-sim/src/config.rs`; `SCENE_FORMAT_VERSION` in
  `crates/tangle-present/src/scene.rs`; `METRIC_DEFINITION_VERSION` and
  `EVENT_FAMILY_LABELS` in `apps/tangle-cli/src/run_metrics.rs`;
  `TRAJECTORY_FORMAT_VERSION` in `apps/tangle-cli/src/trajectories.rs`;
  `RUN_MANIFEST_VERSION`, `RunManifest`, and `fidelity` in
  `apps/tangle-cli/src/run_dir.rs`; the trace header in
  `apps/tangle-cli/src/trace.rs`; and replay in `apps/tangle-cli/src/replay.rs`.
- Deliberately out of scope here, as `PHASE_2_PLAN.md` requires: a navigation
  mesh, a detailed visibility-error model, sidewalk-specific behaviour, a
  balance/lean/fall model, a mode-specific crate, and any new schema version or
  provenance field. World pose remains collision and output truth; route
  coordinates are tactical state and are projected back after integration.

### Increment 2 source shapes

Every shape below is a member of the same version-2 document the Increment 0 and
Increment 1 sections define. Normalized field order is the declaration order the
canonical serializer writes.

| Shape | Kind | Absent means |
| --- | --- | --- |
| `mode_templates[].tactics` additions | extended value set of an existing field | the mode keeps the Increment 1 capabilities |
| `mode_templates[].lateral` | new optional object | the mode has no free lateral motion (Increment 1 behaviour) |
| `mode_templates[].profiles.lateral_accel_max_mps2` | new profile parameter | required exactly when `lateral` is present |
| `facilities[].lateral_policy` | new optional object | the facility offers no lateral maneuver target |
| `facility_adjacencies[]` | new optional array | no lateral transitions exist |
| `clearance_bands[]` | new optional array | close-pass observations carry no band durations |
| `maneuver_policy` | new optional object | required by validation whenever a lateral tactic is authored; carries no default |
| `permissions[]` `lane_use`, `overtake`, `crossing` | new semantics for existing kinds | the Increment 1 defaults stated below |

#### `mode_templates[]` — lateral capabilities and maneuver parameters

`tactics` (an existing required field) gains four values; the Increment 0 values
`follow`, `stop`, and `yield` keep their meaning and their compiled mapping:

| authored value | compiled `TacticalCapability` | meaning |
| --- | --- | --- |
| `change_lane` | `ChangeLane` | move laterally to an adjacent facility traversal |
| `overtake` | `Overtake` | displace past a slower leader on the facility |
| `pass` | `Pass` | pass a slower user within the same facility |
| `reverse_direction` | `ReverseNominalDirection` | select a traversal against the applicable nominal direction |

Declaring `change_lane` or `pass` also compiles `ChooseLateralPosition`, the
capability the compiled lateral position selection reads; a template that
declares none of the four compiles exactly the Increment 1 capability set and no
`ChooseLateralPosition`, so an Increment 0 or Increment 1 template compiles
unchanged. The mapping is keyed on the authored tactic, never on the template id.

`lateral` — the mode's own maneuver parameters, an optional object:

- `target_clearance_m` — number, metres, finite and non-negative. The
  body-to-body *signed surface clearance* (the convention `body_clearance_m`
  defines: zero at touching, negative for overlap) that a lateral maneuver by
  this mode targets between its own swept envelope and the envelope of the body
  it is passing or overtaking, at the maneuver's closest approach. Required
  inside `lateral`.
- `horizon_s` — number, seconds, finite and strictly positive. The feasible time
  horizon: a candidate corridor is feasible only while it stays feasible for at
  least this long from the decision instant at the agent's current speed and
  within its bounded motion. Required inside `lateral`.

Omission and coupling rules, all enforced by validation:

- `lateral` carries no identifier target: the template that authors it is the
  target, and `target_clearance_m` and `horizon_s` apply to every maneuver that
  template's agents run.
- `lateral` omitted means the mode has no free lateral motion: its compiled
  lateral policy is absent, no lateral maneuver is selectable, and the mode
  behaves exactly as in Increment 1.
- `lateral` present requires the template's `tactics` to include at least one of
  the four values above, and requires the mode's `motion` to be
  `single_body_wheeled` (the walking family has no steering state).
- A template that declares any of the four tactics requires `maneuver_policy`
  (below) to be present; a template that declares `reverse_direction`
  additionally requires `maneuver_policy.wrong_way`. Nothing falls back to a
  parser or Phase 1 default.

The lateral wheeled profile set for a `single_body_wheeled` template that
declares `lateral` — every parameter finite and strictly positive except where
noted, and read through the existing `required_profile_params` seam:

- `steering_rate_max_rad_s` — the maximum heading rate the mode can hold, radians
  per second. Increment 1 required it for the capsule family; Increment 2
  requires it for every lateral-capable wheeled template.
- `lateral_accel_max_mps2` — the maximum lateral acceleration, metres per second
  squared. New in Increment 2.
- `lateral_clearance_m` — the mode's preferred clearance from the facility edge,
  metres, finite and non-negative (the Increment 1 non-negative range rule).

The plan's steering-angle limit is represented in version 2 by these two
derivative limits rather than by an authored steering angle: version 2 authors no
axle geometry a single-track steering angle could convert into curvature, and a
bicycle, scooter, or car bears no authored wheelbase before Increment 3. The
compiled bounded-steering contract the lateral motion leaf must carry is
therefore exactly: `|heading_rate| <= steering_rate_max_rad_s`,
`|v * heading_rate| <= lateral_accel_max_mps2`, the Increment 1 turning-limit
rule `|kappa| <= steering_rate_max_rad_s / v` that
`mode_turning_limit_curvature` already enforces, and the facility corridor
boundary. The lateral offset rate is derived from the integrated heading
(`d_dot = v * sin(theta_error)`); no target offset is ever written to `d` or to a
world position directly.

Normalized example:

```json5
mode_templates: [
  {
    id: 'bicycle',
    body: { kind: 'capsule', length_m: { min: 1.8, max: 1.8 }, radius_m: { min: 0.35, max: 0.35 } },
    motion: 'single_body_wheeled',
    tactics: ['follow', 'stop', 'yield', 'change_lane', 'overtake', 'pass', 'reverse_direction'],
    access: { facility_kinds: ['facility'], nominal_direction: 'either', speed_policy: { limit_mps: null } },
    occupancy: 'operator_only',
    profiles: {
      speed_mps: { min: 4.5, max: 4.5 },
      max_accel_mps2: { min: 1.2, max: 1.2 },
      comfortable_brake_mps2: { min: 2.0, max: 2.0 },
      time_gap_s: { min: 1.0, max: 1.0 },
      steering_rate_max_rad_s: { min: 0.9, max: 0.9 },
      lateral_accel_max_mps2: { min: 2.0, max: 2.0 },
      lateral_clearance_m: { min: 0.3, max: 0.3 },
      compliance: { min: 1.0, max: 1.0 },
    },
    lateral: { target_clearance_m: 0.75, horizon_s: 4.0 },
  },
]
```

The Increment 1 `bicycle` and `scooter` templates remain valid unchanged: they
author `lateral` nowhere, so they keep the Increment 1 longitudinal behaviour,
and the values above are provisional engineering defaults the model cards must
label as such.

#### `facilities[]` — lateral-use policy

The Increment 1 `lateral_use` field keeps its authored values and gains its
Increment 2 behaviour, which is what free lateral motion reads:

- `shared` — the agent's permitted lateral interval is its whole usable lateral
  interval (the Increment 1 `usable_lateral_interval`), and a lateral maneuver
  may target any offset inside it.
- `centered` — the agent's permitted lateral interval is the single offset `0`
  within `POSITION_TOLERANCE_M = 1e-6 m`; no target offset exists, so a lateral
  maneuver on that facility is infeasible, and a resolved offset outside that
  tolerance is a lane-use violation fact for the evidence stream.

`lateral_policy` — the facility's lateral-use policy, an optional object:

- `passing_side` — `left`, `right`, or `most_clearance`. Required inside
  `lateral_policy`. The side on which a within-facility pass or overtake
  displaces, measured in the maneuvering agent's own direction of travel on the
  facility: `left` is the positive-`d` side, `right` the negative-`d` side, and
  `most_clearance` selects the side whose usable corridor has the greater
  predicted minimum clearance, with an exact tie resolving to `left`.

Omission and validation rules:

- `lateral_policy` carries no identifier target beyond the facility that authors
  it.
- `lateral_policy` omitted means the facility offers no lateral maneuver target:
  an agent may not start a pass, an overtake, or a lateral position change on
  that facility, exactly as in Increment 1. A `change_lane` transition *out of*
  the facility remains available through an authored adjacency, because its
  target is the adjacent facility.
- `lateral_policy` requires the facility to declare a `reference_path` (a
  facility with no `(s, d)` frame has no side to name).
- A `passing_side` for which no permitted body has a non-empty nonzero usable
  interval on that side is rejected: the policy must name a side an eligible
  body can actually occupy.
- `lateral_use: centered` together with `lateral_policy` is rejected: a centered
  facility offers no lateral target for the side to apply to.

Normalized example:

```json5
facilities: [
  {
    id: 'bikeway_eastbound',
    region: 'bikeway_band',
    reference_path: 'bikeway_centerline',
    width_m: 3.0,
    nominal_direction: 'forward',
    access: { modes: ['bicycle', 'scooter'] },
    lateral_use: 'shared',
    lateral_policy: { passing_side: 'most_clearance' },
    speed_policy: { limit_mps: null },
  },
]
```

#### `facility_adjacencies[]` — lateral transitions between facilities

A `facility_connectors[]` entry keeps its Increment 1 meaning exactly: a
directed join whose geometry is the coincidence of the leaving and entering
ends. A lane change between two parallel bands is not an end join, so Increment 2
adds a separate, additive shape for it; it never reinterprets a connector.

Each entry declares that two facility bands lie side by side along a shared
stretch, which is the only way a lateral transition becomes available:

- `id` — stable id, unique across all authored objects.
- `first` — id of a declared facility.
- `second` — id of a declared facility adjacent to `first`.
- `side` — `left` or `right`: the side of `first` on which `second` lies,
  measured in `first`'s authored forward direction (its reference path vertex
  order), so the value does not depend on any agent's direction of travel.

Rules:

- The adjacency is an undirected side-by-side relation; a lateral transition may
  cross it in either direction, and the traversal direction an agent enters with
  is fixed by its own direction of travel (below), not by the adjacency.
- Two facilities not joined by an adjacency or a connector are not laterally
  connected, however close their polygons are: proximity is never inferred into
  a transition.
- Omitted or empty `facility_adjacencies` means no lateral transitions exist,
  which is the Increment 1 behaviour.
- Validation requires both ids to name declared, distinct facilities that each
  declare a `reference_path`, and the authored `side` to agree with the compiled
  geometry: the bands must touch within `ADJACENCY_TOLERANCE_M = 1e-6 m` along a
  stretch of positive length, on the declared side.

Normalized example:

```json5
facility_adjacencies: [
  { id: 'bikeway_to_roadside', first: 'bikeway_eastbound', second: 'road_eastbound_curb', side: 'right' },
]
```

#### `clearance_bands[]` — configurable close-pass clearance definitions

Each entry is one labeled clearance threshold. Bands are metric definitions for
the scenario that authors them — configurable by jurisdiction or study — never
universal declarations of safety.

- `id` — stable id, unique across all authored objects.
- `threshold_m` — number, metres, finite and strictly positive: the body-to-body
  signed surface clearance threshold the band defines.
- `violation` — boolean, required: whether a pass whose observed minimum
  clearance falls below `threshold_m` is recorded as a lateral-displacement
  violation in this scenario.
- `applies_to_modes` — optional array of `mode_templates[]` ids; omitted means the
  band applies to every mode pair. When present it must be non-empty and every
  id must name a declared template. A band applies to a passing pair when the
  passing agent's or the passed user's mode template id appears in the list.

Rules:

- Bands must be declared in strictly increasing `threshold_m`, and no two bands
  may share a threshold; durations are reported in declaration order, so the
  ordering is what makes the reported set deterministic. A band accumulates the
  duration during which the observed clearance is *below* its threshold, so the
  bands nest.
- The band set is scenario-scoped, so "configured by jurisdiction or study" means
  the scenario that authors the bands.
- When a fixture declares a passing band, the tolerance's "authored clearance
  band" scalar is that band's `threshold_m`; no other value is derived from a
  band.
- Omitted or empty `clearance_bands` means close-pass observations record the
  minimum clearance, its time, and its relative speed, and no band durations or
  violations — the Increment 1 behaviour.

Normalized example:

```json5
clearance_bands: [
  { id: 'motor_close_pass', threshold_m: 1.0, violation: true, applies_to_modes: ['passenger_car'] },
  { id: 'narrow_close_pass', threshold_m: 0.75, violation: true, applies_to_modes: ['bicycle', 'scooter'] },
  { id: 'study_band', threshold_m: 1.5, violation: false },
]
```

#### `maneuver_policy` — commit and wrong-way policy

`maneuver_policy` is one optional object; `commit` and `wrong_way` are optional
objects inside it, and every field inside either is required once that object is
present. Nothing here defaults: a scenario that authors a lateral tactic must
author this policy, and a scenario that authors `reverse_direction` must author
`wrong_way`.

`commit` — the documented unsafe-commit and commitment-loss policy:

- `min_predicted_clearance_m` — number, metres, finite and non-negative: the
  absolute floor on the predicted minimum clearance of any of the four clearance
  facts. A committed maneuver whose predicted clearance falls below it aborts.
- `hold_timeout_s` — number, seconds, finite and strictly positive: how long a
  `preparing` maneuver may wait for a granted claim, and how long a `committed`
  maneuver may hold at or below its target clearance waiting for the corridor to
  reopen, before it aborts.
- Validation requires `min_predicted_clearance_m` to be at most the
  `lateral.target_clearance_m` of every lateral-capable mode; a policy that would
  abort every maneuver before it could commit is rejected rather than silently
  narrowed.

`wrong_way` — the contextual opposing-traversal decision inputs, all read at the
decision instant from the immutable observation:

- `min_time_saving_s` — number, seconds, finite and non-negative: the minimum
  estimated travel-time saving the opposing option must provide over the nominal
  option, where the saving is
  `nominal_remaining_length_m / nominal_expected_speed_mps -
   opposing_remaining_length_m / opposing_expected_speed_mps`, with both
  remaining lengths from the compiled route geometry and the expected speed the
  agent's own desired free-flow speed capped by the effective facility limit.
- `max_opposing_density_per_km` — number, agents per kilometre, finite and
  non-negative: the largest density of opposing-travelling bodies observed within
  the target traversal ahead for which the opposing option stays eligible, where
  the density is that observed count divided by the traversal's length in
  kilometres.
- `urgency` — number in `[0, 1]`: the scenario's willingness to accept a
  violating opposing traversal, used in the non-compliance draw below.

Neither object carries an authored default; the omission rules above are the only
absence semantics. `maneuver_policy` carries no identifier target: it is
scenario-scoped policy that applies to every mode, facility, and object in the
document.

Normalized example:

```json5
maneuver_policy: {
  commit: { min_predicted_clearance_m: 0.25, hold_timeout_s: 2.0 },
  wrong_way: { min_time_saving_s: 10.0, max_opposing_density_per_km: 40.0, urgency: 0.5 },
},
```

#### `permissions[]` — lane use, overtaking, and crossing semantics

Increment 1 fixed the statement shape (`id`, `kind`, `holder`, `target`,
`effect`) and the target kind each `kind` names. Increment 2 fixes the meaning of
the three kinds it populates; `stop_service` stays shape-only until Increment 4.

| `kind` | target names | `permit` | `prohibit` | `obligate` |
| --- | --- | --- | --- | --- |
| `nominal_direction` | `facility` or `movement` | the holder may travel against the nominal direction as well as with it | the holder may not travel against the nominal direction (the default) | the holder must travel against the nominal direction (contraflow); travelling with it is then the violation |
| `lane_use` | `facility` | the holder may select any offset in its usable interval and may cross into a laterally adjacent facility it is permitted on | the holder must hold the reference centerline (offset `0` within `POSITION_TOLERANCE_M`) and may not cross any band boundary out of the facility | the holder must hold the offset nearest the side opposite the facility's `lateral_policy.passing_side`, except while a committed maneuver is displaced |
| `overtake` | `facility` | the holder may pass or overtake on the facility where capability, geometry, and the corridor allow | the holder may not: a `preparing` pass or overtake on that facility is ineligible with reason `no_permission`, and an avoidable boundary crossing is prevented | the holder must initiate a pass or overtake against an applicable slower leader whenever every other eligibility condition holds |
| `crossing` | `crossing` | the holder may traverse the crossing | the holder may not traverse it and must yield or hold short of the crossing region | the holder must traverse the crossing once its governing rule or control permits and the region is free |

Resolution rules, which the compiled policy and validation both read:

- A statement applies to the pair `(holder, target)` where `holder` is the
  agent's mode template id and `target` is the object the agent is traversing or
  about to traverse; `kind` fixes which question the statement answers.
- Absent statements mean: nominal-direction travel is permitted and opposing
  travel is not; lateral position is whatever the facility's `lateral_use` and
  `lateral_policy` allow; passing and overtaking are permitted wherever the
  mode's capability and the geometry allow; a crossing is permitted wherever the
  governing rule or control allows. These are the Increment 1 behaviours.
- Specificity has exactly one axis, `(kind, holder, target)`. Two statements that
  agree on it and disagree on `effect`, or two identical statements, are rejected
  with a stable diagnostic; there is no "last one wins" and no silent override.
- When both a facility-targeted and a movement-targeted `nominal_direction`
  statement apply to one traversal, the movement-targeted statement decides: the
  narrower object wins, deterministically.
- A `nominal_direction` statement that targets a facility or movement whose
  nominal direction is `either` is rejected: no opposite direction exists to
  permit, prohibit, or oblige.
- `obligate` on `lane_use` requires the facility to declare
  `lateral_policy.passing_side` as `left` or `right`; `most_clearance` names no
  fixed side and is rejected for that statement.
- An `overtake` statement whose holder does not declare the `overtake` tactic is
  rejected: a permission or obligation with no capable holder is a malformed
  policy, not an inert annotation.
- A `nominal_direction` statement with effect `permit` or `obligate` whose target
  object has no physically possible opposing traversal cannot be realized: the
  permitted set never leaves the physically possible set, so the statement is
  inert for that traversal and never schedules a route the connector graph or a
  lateral adjacency does not support. It is therefore *not* a validation failure
  (an `obligate` on a forward-only facility validates cleanly, as Increment 1
  fixed), and the case is instead closed by the wrong-way decision's first
  precondition: with no opposing traversal the decision's reason is
  `no_opposing_path` and no interval can open.
- A statement never widens physical possibility. A `permit` adds to the permitted
  direction set only; the compiled physically possible set is unchanged, and a
  `prohibit`ed but physically connected traversal remains available as violation
  context to a non-compliant decision, which is what keeps legality and physical
  possibility separate.

Normalized example:

```json5
permissions: [
  { id: 'car_may_pass_bicycle', kind: 'overtake', holder: 'passenger_car', target: 'road_eastbound', effect: 'permit' },
  { id: 'bicycle_may_use_contraflow', kind: 'nominal_direction', holder: 'bicycle', target: 'bikeway_westbound', effect: 'permit' },
  { id: 'scooter_holds_centerline', kind: 'lane_use', holder: 'scooter', target: 'bikeway_eastbound', effect: 'prohibit' },
  { id: 'pedestrian_may_cross', kind: 'crossing', holder: 'pedestrian', target: 'crossing_north', effect: 'permit' },
]
```

The four top-level additions appear in one document fragment, in normalized
declaration order, as:

```json5
facility_adjacencies: [ /* one entry per adjacent pair, as above */ ],
permissions: [ /* statements of every kind, as above */ ],
clearance_bands: [ /* thresholds, ascending, as above */ ],
maneuver_policy: { commit: { /* ... */ }, wrong_way: { /* ... */ } },
```

A version-1 source migrates with `facility_adjacencies: []`,
`clearance_bands: []`, `maneuver_policy` absent, and no `lateral` or
`lateral_policy` anywhere, so a migrated document is Increment 1 behaviour
exactly and the migration's determinism rule is unchanged.

### Compiled semantics

#### Nominal, permitted, and physically possible traversal

The three direction properties Increment 1 named stay separate, and Increment 2
fixes them per `(mode, facility traversal)`:

- **nominal** — the facility's authored `nominal_direction`
  (`CompiledFacility::nominal_direction`), independent of any mode.
- **permitted** — the set of directions the holder may travel, resolved from the
  holder's `AgentAccess::nominal_direction` (Increment 1: the mode's own
  restriction) intersected with the applicable `permissions[]` effect above. A
  `permit` adds the opposing direction; `prohibit` leaves only the nominal
  direction; `obligate` leaves only the opposing direction; no statement leaves
  only the nominal direction (and on an `either` facility, where both directions
  are nominal, the permitted set is both).
- **physically possible** — the directions a connected traversal supports
  (`CompiledFacility::physically_possible_directions`, from the connector graph
  in both directions) plus a lateral adjacency whose continuation direction is
  possible on the adjacent facility.

The compiled scenario exposes the resolved statement set by dense id
(`CompiledScenario` compiles `permissions[]` into an id-resolved form with one
entry per statement, preserving source order, and offers a lookup by
`(kind, holder, target)`), and the permitted set is always a subset of the
physically possible set: a permission never makes an impossible route possible.
`CompiledModeTemplate` carries the resolved lateral policy of a lateral-capable
mode (its target clearance, its horizon, and its bounded-steering limits)
alongside the components Increment 0 and Increment 1 fixed, so the tactical stages
read components and never a template id or a source string. An empty permitted
set on a traversal a permitted mode is routed through is rejected as a *legal*
impossibility with its own diagnostic, distinct from the physically impossible
codes (no connector, an over-curved facility, a too-narrow facility), exactly as
`validate_permissions` already reports a `prohibit` that contradicts a
facility's granted access. Neither diagnostic removes the traversal from the
physically possible set: a prohibited but connected traversal stays available as
violation context to a non-compliant decision and is never silently dropped.

#### Usable corridor, predicted corridor, and clearance facts

- **usable interval** — the Increment 1 geometric interval at arc length `s`:
  the signed offsets where the body envelope plus its lateral clearance fits
  inside the band (`CompiledFacility::usable_lateral_interval`).
- **usable corridor** — the set of route offsets the agent can reach within
  `lateral.horizon_s` from its current state under its bounded motion and inside
  the usable interval at every intermediate `s`. It is a *reachability* set, not
  an occupancy prediction, and it is what the tactical stage checks before
  `following -> preparing`.
- **predicted corridor** — the swept region the candidate maneuver's envelope
  would occupy over the horizon: the agent's compiled body swept between
  consecutive sampled poses, expressed in route coordinates and in world
  coordinates, including every intermediate band boundary of the transition. It
  is what the clearance facts below are computed over.

Both corridors are sampled at the run's lateral-decision cadence and subdivided
finely for the clearance minimum; the sampling is a pure function of the
compiled scenario, the immutable observation, and the resolved fidelity values,
so the same inputs produce the same samples. The lateral-decision cadence and the
maneuver-prediction horizon are run configuration resolved explicitly by the
Fast, Standard, and Fine presets, exactly as the fixed step is, and are recorded
with the run; the prediction horizon must be at least every authored
`lateral.horizon_s`.

The four clearance facts are the vocabulary of feasibility, of the abort
decision, and of the close-pass evidence. Every fact is a *signed surface
clearance* in metres, the same convention `body_clearance_m` and
`CONTACT_EPSILON_M` already use: zero at touching, negative for an overlap:

| fact | definition |
| --- | --- |
| `front_clearance_m` | the minimum signed clearance, over the horizon and the predicted corridor, between the agent's front envelope and the rear envelope of a body ahead (greater route progress on the same traversal) |
| `rear_clearance_m` | the same for a body behind (smaller progress) whose closing speed is positive toward the agent |
| `side_clearance_m` | the minimum signed clearance, over the horizon and the predicted corridor, to a body whose progress interval overlaps the agent's (a side-by-side body) |
| `swept_clearance_m` | the minimum signed clearance over the whole predicted sweep against every candidate body and the facility band edge; it is never greater than any of the facts above, and it is the value the abort decision reads |

Each fact reports the limiting object — a body `AgentId` or the band edge — the
time of its minimum, and the closing speed along the minimum's axis in metres per
second. Feasibility means every fact stays at or above the mode's
`target_clearance_m` throughout the horizon; a committed maneuver additionally
never lets `swept_clearance_m` fall below
`maneuver_policy.commit.min_predicted_clearance_m`. The close-pass observation's
own `min_clearance_m` is the minimum body-to-body swept clearance over its
interval, excluding boundaries; boundary crossings are recorded separately as a
boolean fact.

#### Transition targets and the geometric handoff

For an agent on `(facility, direction)` at route position `s`, the compiled
transition targets are:

- **lateral** — every facility joined to the current one by a
  `facility_adjacencies` entry, each with the side of the crossing mapped into the
  agent's own travel frame and the traversal direction that continues its travel:
  the destination traversal whose reference tangent agrees with the agent's own
  travel direction (a non-negative dot product). A destination whose effective
  direction does not permit that traversal makes the crossing a forbidden
  boundary rather than an impossible route.
- **longitudinal** — the Increment 1 connector targets that leave the current
  traversal's end (`CompiledFacility::outgoing_connectors`), unchanged.

The handoff itself is fixed at exactly one geometric point per transition kind,
so no observer sees a change at two different times:

- a lateral handoff occurs when the agent's body centre crosses the lateral
  boundary between the two bands;
- a connector handoff occurs when the agent's body centre reaches the compiled
  connector coincidence within `CONNECTOR_CONTINUITY_TOLERANCE_M`.

At either handoff, route and facility ownership changes in one step, the agent's
world pose is continuous (no despawn, no re-spawn, no snap to the destination
reference), and its progress is preserved by projecting the unchanged world pose
onto the destination reference. Current and destination leader and follower
constraints stay active through the handoff, and the ordinary spatial index and
collision scan see every intermediate world pose.

A transition into a traversal the applicable rule does not permit is a
**forbidden boundary crossing**. It is prevented when avoidable — the claim is
infeasible with reason `boundary_forbidden` — and when the committed policy
cannot avoid it, it is recorded as a `FacilityTransition` fact with
`permitted: false`. `T-O3` requires zero boundary crossings without that record.

### Maneuver lifecycle

The plan's state machine is fixed here as five states, one active maneuver per
agent, and an edge-triggered record for every transition. Increment 1's
`Commitment` record carries `preparing` and `committed`; Increment 2 adds the
other three states to the same tactical record in
`crates/tangle-sim/src/stage.rs`, and no state is spelled differently anywhere
else.

| state | meaning |
| --- | --- |
| `following` | no lateral maneuver; the agent holds its current offset and runs the Increment 1 longitudinal tactics |
| `preparing` | a target (`target_offset_m`, and for a `change_lane` a `target_facility`) and a candidate corridor are fixed; a claim is sought at the decision cadence |
| `committed` | the claim is granted; the agent displaces laterally toward the target under bounded steering |
| `returning` | the passed obstacle is cleared; the agent returns to its own facility and offset |
| `aborted` | the maneuver ended without reaching its target; the agent returns to its pre-maneuver facility and offset |

Legal transitions, with their guards and their recorded edge:

| transition | guard | record |
| --- | --- | --- |
| `following -> preparing` | a lateral tactic is selected: the capability exists, no applicable permission is `prohibit`, a route benefit exists, the target is inside the usable corridor, and the candidate corridor exists | `Maneuver` with edge `attempted` |
| `preparing -> committed` | the batch arbitration of this step grants this agent's claim for its candidate corridor | `Maneuver` with edge `committed` |
| `preparing -> aborted` | the claim is rejected by arbitration, the target body or target facility disappears, `hold_timeout_s` elapses without a grant, or the corridor becomes infeasible | `Maneuver` with edge `aborted` and the reason |
| `committed -> returning` | the passed obstacle is cleared: the agent's rear envelope point is at least `target_clearance_m` ahead of the passed body's front envelope along the travel direction | `Maneuver` with edge `completed` |
| `committed -> aborted` | `swept_clearance_m` falls below `min_predicted_clearance_m`, the target facility or connector disappears, or the hold timeout elapses | `Maneuver` with edge `aborted` and the reason |
| `returning -> following` | the signed offset is within `SETTLE_TOLERANCE_M = 1e-3 m` of the return target and at least one decision cadence has elapsed at that offset | `Maneuver` with edge `completed` |
| `aborted -> following` | the signed offset is within `SETTLE_TOLERANCE_M` of the pre-maneuver offset, or the agent has handed off to its pre-maneuver facility | `Maneuver` with edge `aborted` |

No other transition is legal; in particular a maneuver is never selected from
`committed` or `returning` (a new maneuver starts only from `following`), a
committed target never changes, and `aborted` has no other exit. Tactical clauses
(`following -> preparing`, `preparing -> committed`, `returning -> following`) are
evaluated at the lateral-decision cadence; safety clauses
(`committed -> aborted`, the bounded braking response, and boundary prevention)
are evaluated every step, so a hazard between two cadences brakes immediately and
aborts at the first step its condition holds.

#### Simultaneous claim arbitration

- Claims are collected from one immutable observation per step and arbitrated as
  a batch before any command is produced; no claim depends on iteration,
  declaration, or discovery order.
- Two claims conflict when their candidate corridors overlap on the same facility
  traversal, or when granting both would put either predicted corridor within the
  other's `target_clearance_m`.
- The winner of a conflict is the first claimant in this lexicographic key,
  computed only from immutable inputs: (1) an agent already `committed` before an
  agent in `preparing` — an existing commitment is never revoked; (2) the smaller
  remaining distance along its own travel direction to the entry of the contested
  corridor; (3) the smaller stable `AgentId`. The third clause is a total order on
  distinct agents, so the key is total and invariant to source declaration,
  candidate discovery, and insertion order.
- A loser aborts with reason `claim_rejected`; a loser's last-moment brake stays
  subject to the same motion limits, and no winner is revoked mid-step, so an
  agent that loses clearance does not hand its corridor to another claimant in
  the same step — that claim is re-arbitrated at the next decision.

#### Commitment loss and the unsafe-commit policy

Once committed, the response to a predicted-clearance loss is fixed and ordered:

1. **brake** — command deceleration within the profile's `comfortable_brake_mps2`
   and never accelerate, while the corridor still admits it;
2. **hold** — hold the committed target and wait while `swept_clearance_m` stays
   at or above `min_predicted_clearance_m`, for at most `hold_timeout_s` at or
   below the target clearance;
3. **abort** — when `swept_clearance_m` falls below
   `min_predicted_clearance_m`, when the hold times out, or when the target
   disappears, the maneuver aborts and the agent returns to its pre-maneuver
   offset and facility under the same bounded steering.

The existing safety position caps remain the last-resort backstop and their
counter stays meaningful; a maneuver must never rely on them to avoid a
collision, and no failure path teleports, overlaps silently, exceeds a motion
limit, or disables a query. Timeouts, a disappearing target, a closing connector,
and a blocked return each reach the deterministic transition named in the table
above rather than an implicit fallback.

### Contextual wrong-way traversal

The nominal direction, the permitted direction, and the physically possible
direction stay separate, and a wrong-way traversal is an ordinary traversal that
the ordinary routing, steering, collision, yielding, and event paths carry; no
flag disables a query and no scripted trajectory exists.

The decision reads exactly these inputs at the decision instant, from the
immutable observation and the compiled policy:

1. the mode's `tactics` including `reverse_direction`;
2. physical connectivity of the opposing traversal — the reverse traversal is in
   `physically_possible_directions`, or a connected opposing traversal exists
   through an adjacency or connector;
3. the object's authored nominal direction, which must not be `either`;
4. the estimated time saving of the opposing option over the nominal option;
5. the observed opposing density within the target traversal ahead;
6. the applicable `permissions[]` effect for `(holder, target)`;
7. the agent's sampled `compliance` and desired speed, and the scenario's
   `maneuver_policy.wrong_way` thresholds;
8. one draw `u` in `[0, 1)` from the versioned `maneuver` stream, keyed by the
   root seed, the run ID, the stable `AgentId`, and the agent's decision ordinal.

The decision procedure is total and deterministic:

- If any of (2) or (3) fails, no opposing option exists: the recorded reason is
  `no_opposing_path` or `no_nominal_direction`, no draw is taken, and no interval
  can open. If the capability in (1) is absent the agent never reaches the
  decision at all.
- If the estimated time saving is below `min_time_saving_s`, the reason is
  `insufficient_time_saving`; if the observed opposing density exceeds
  `max_opposing_density_per_km`, the reason is `opposing_density_too_high`. Both
  are non-random rejections and take no draw.
- Otherwise the agent draws `u` and selects the opposing option when
  `u < urgency * (1 - compliance)`; an exact tie selects the nominal option, so
  a fully compliant agent under any urgency, and any agent under zero urgency,
  keeps the nominal option with reason `compliant_choice`. The draw is taken once
  per decision evaluation, in ascending `AgentId` order, and only after the
  non-random preconditions pass, so an agent without an opposing option cannot
  perturb another agent's draws.
- The selected option's legality follows from (6): `permit` makes the opposing
  traversal legal (reason `legal_permission`), `obligate` makes it the obligated
  direction (reason `legal_obligation`), and an absent or `prohibit`ed statement
  makes it a violation (reason `noncompliant_choice`). No other input changes
  legality.
- An occupied opposing corridor is not a precondition failure: the agent selects
  the traversal and the ordinary claim, prediction, yielding, and collision
  machinery rejects it or makes it a bounded wait, brake, or abort. The corridor
  is never bypassed.
- The decision records the perceived rule, the selected option, the reason code,
  the affected facility and movement ids, and the context values it used (saving,
  density, urgency, compliance), and it adds no visibility-error or perception
  subsystem.

The **violation interval** boundaries are fixed as:

- It **opens** on the first step where the agent's body centre lies inside the
  object's extent and its traversal direction on that object is *against* the
  applicable rule direction, where the rule direction is the object's nominal
  direction unless an `obligate` statement applies, in which case travelling
  *with* it is the violation. The extent is the facility traversal's compiled
  reference `[0, length]`, or the movement's `[0, path length]` between its
  portals; a facility without a reference path has no extent and can carry no
  interval, and an `either` object has no rule direction.
- It **closes** on the first later step where the traversal direction becomes the
  rule direction, the body centre leaves the extent (including a lateral or
  connector handoff and a despawn), the agent enters a different object, or the
  run ends (the close time is then the final simulation time).
- Open and close are the only two records; nothing is emitted per step. A
  rejected decision creates no interval, because the record is emitted only once
  the agent actually traverses.
- A permitted or obligated opposing traversal opens a *legal* opposing interval
  that is recorded with `violating: false` and appears in the opposing-traversal
  metrics, never in the violation families. Nominal travel is not counted at all.

### Increment 2 events and metrics

#### Event payloads

The event union gains four variants. No existing variant's fields, meaning, or
ordering position changes; `EVENT_VERSION` is bumped once for the union (below).

| variant | payload | emitted |
| --- | --- | --- |
| `Maneuver` | `agent: AgentId`, `kind: TacticKind`, `from: ManeuverState`, `to: ManeuverState`, `edge: ManeuverEdge` (`attempted`, `committed`, `completed`, `aborted`), `partner: Option<AgentId>`, `source_facility: FacilityId`, `target_facility: Option<FacilityId>`, `target_offset_m: f64`, `side: PassSide` (`left` or `right` in the agent's travel frame), `reason: ManeuverReason` | once per legal transition above, at the step the transition happens |
| `FacilityTransition` | `agent: AgentId`, `from_facility: FacilityId`, `to_facility: FacilityId`, `from_direction: MovementDirection`, `to_direction: MovementDirection`, `via: TransitionKind` (`lateral` or `connector`), `side: PassSide`, `s_m: f64`, `d_m: f64`, `permitted: bool` | once per handoff, at the handoff step; `permitted: false` is the forbidden-boundary fact |
| `ClosePass` | `agent: AgentId`, `partner: AgentId`, `facility: FacilityId`, `side: PassSide`, `min_clearance_m: f64`, `min_clearance_time_s: f64`, `relative_speed_mps: f64`, `bands: Vec<ClosePassBand>` (`band: ClearanceBandId`, `duration_s: f64`), `violating_bands: Vec<ClearanceBandId>`, `crossed_boundary: bool`, `entered_opposing: bool` | once, when the observation closes on pass completion, abort, or termination |
| `OpposingTraversal` | `agent: AgentId`, `facility: FacilityId`, `movement: Option<MovementId>`, `direction: MovementDirection`, `nominal_direction: NominalDirection`, `perceived_rule: Option<PermissionEffect>`, `reason: WrongWayReason`, `violating: bool`, `entering: bool` | once when the interval opens and once when it closes |

`ManeuverReason` is a closed set of stable codes covering both eligibility
rejections and terminations: at least `capability`, `slower_leader`,
`route_benefit`, `insufficient_width`, `no_permission`, `no_benefit`,
`no_corridor`, `claim_rejected`, `target_gone`, `timeout`, `clearance_loss`,
`boundary_forbidden`, and `settled`. `WrongWayReason` is a closed set including
`no_opposing_path`, `no_nominal_direction`, `insufficient_time_saving`,
`opposing_density_too_high`, `compliant_choice`, `noncompliant_choice`,
`legal_permission`, and `legal_obligation`. Every rejection an eligibility check
can produce has one of these codes, so a rejected precondition has an inspectable
reason.

Applicability: `Maneuver` requires the corresponding tactic in the agent's
compiled capability set; `FacilityTransition` requires a lateral
`facility_adjacencies` entry or a connector; `ClosePass` requires a `pass` or
`overtake` capability and a passed body; `OpposingTraversal` requires a traversal
against a rule direction. A run whose scenario authors none of these shapes emits
none of them, so Phase 1 and Increment 1 event streams are unchanged apart from
the recorded version.

#### Ordering

The four variants are **appended** to `EventKind` after `ControlTransition`, so
every existing kind keeps its `EventKind::order` value and its position in the
within-tick sort; no existing stream reorders. Within a kind, the variant's
stable key is: `Maneuver` by `(kind, target facility, from, to)`; `FacilityTransition`
by `(from facility, to facility)`; `ClosePass` by `(partner, facility)`;
`OpposingTraversal` by `(facility, movement, entering)`. The existing key order
then applies — ascending `AgentId`, then kind order, then the variant key, then
the edge flag — and the sort stays stable, so emission order never depends on
declaration or insertion order. Emission is edge-triggered from the recorded
state, so a retry cannot duplicate a transition or an interval edge.

#### Metrics and output

The metric families are additive; every existing family keeps its name, formula,
unit, applicability, and disaggregation, and `DEF-004` and `DEF-005` remain the
definitions of their own versions:

- overtaking — `overtake_attempts`, `overtake_commits`, `overtake_completions`
  (the `committed -> returning` edges), and `overtake_aborts`, counted per mode
  pair, movement, and facility;
- close passes — `close_passes`, `close_pass_minimum_clearance_m` (the minimum
  and its time and relative speed), one duration series per declared band keyed
  by its stable id, and `close_pass_violations` (observations with a non-empty
  `violating_bands`), disaggregated by mode pair, movement, facility, and
  applicability;
- wrong-way travel — `wrong_way_intervals`, `wrong_way_distance_m`,
  `wrong_way_duration_s`, `wrong_way_exposure_agent_s`, `wrong_way_encounters`
  (unique partners), and `wrong_way_conflicts`, disaggregated by mode, movement,
  facility, participant pair, and the perceived rule, so a legal opposing
  traversal is reported under its rule and never as a violation;
- interactions — the `CC-OVERTAKE` and `CC-OPPOSE` classification labels
  (`overtaking`, `head-on/opposing`) with `unknown` preserved rather than forced,
  and finite-horizon predicted minimum separation alongside the existing sampled
  and swept minima.

Applicability follows the existing rule: a countable family is reported with a
count (zero when absent), while a value metric that cannot apply to a mode,
facility, or pair is `not_applicable` and one that simply did not occur is
`not_observed`; an absent value is never a `0`. Metrics are read from the records
above and from the ordinary world-body queries, never from centre distance, a
lane id, or an uncalibrated surrogate.

Sampled snapshots and the trajectory artifact gain the additive optional columns
`route_s_m`, `route_d_m`, `target_offset_m`, `predicted_min_clearance_m`,
`maneuver_state`, `maneuver_kind`, `perceived_rule`, and
`opposing_direction`. They are present only for agents that have the state, stay
subject to the manifest's sampling policy as high-volume fields, while state
transitions and safety records remain sparse events that are always emitted. The
scene gains backend-neutral overlay primitives for the usable corridor, the
target offset, the predicted gap, the maneuver state, and the wrong-way rule
state, mapped to identifiers only.

#### Definition-version bumps

Each constant below is bumped **exactly once for the additive union it
describes**, by the leaf named for it; a later leaf of the same union must not
bump it a second time, and two bumps of one constant in this increment are a
defect. The numbers themselves are not fixed here: the rule is that each constant
increases by exactly one from its Increment 1 value and the implementing leaf
records the new value together with the versioned rationale.

| surface | constant and location | bump rule | implemented by | consumers that must observe it |
| --- | --- | --- | --- | --- |
| event union | `EVENT_VERSION`, `crates/tangle-sim/src/event.rs` | one bump for the four new variants; no existing variant's fields or meaning change | [[TAS-100-version-the-maneuver-event-and-trace-surface]] | run manifest, canonical trace header, JSONL replay, summaries, inspectors, and the affected Phase 1 goldens, which change only with the versioned rationale |
| trajectory and snapshot columns | `TRAJECTORY_FORMAT_VERSION`, `apps/tangle-cli/src/trajectories.rs` | one bump for the whole additive column union, landed by the first leaf that changes the artifact's columns; the later leaf adds its columns under that same version | [[TAS-088-add-route-relative-lateral-agent-state]] lands it; [[TAS-102-record-wrong-way-intervals-and-disaggregated-metrics]] adds rule-state columns under it | trajectory artifact, replay, aggregate, compare, and convergence readers |
| scene format | `SCENE_FORMAT_VERSION`, `crates/tangle-present/src/scene.rs` | one bump for the additive overlay primitives and inspector text | [[TAS-111-present-increment-2-corridor-gap-and-rule-overlays]] | Bevy and terminal backends, shared-scene tests, and the affected scene goldens |
| metric definition | `METRIC_DEFINITION_VERSION`, `apps/tangle-cli/src/run_metrics.rs` | one bump for the additive metric union (the families above and their dimensions) | [[TAS-101-measure-close-passes-with-exact-clearance-evidence]] lands it; [[TAS-102-record-wrong-way-intervals-and-disaggregated-metrics]] adds its families under it | run summary, metrics artifact, aggregation, comparison, convergence, and experiment reports; `DEF-004` v1 and `DEF-005` v2 stay authoritative for the artifacts already labelled with them |
| run manifest shape | `RUN_MANIFEST_VERSION`, `apps/tangle-cli/src/run_dir.rs` | the provenance block is unchanged and no new definition version is introduced; the manifest records the resolved lateral-decision cadence and prediction horizon beside the step and fidelity label, and its own shape version is bumped once for that addition by the first leaf that lands the recorded values | [[TAS-088-add-route-relative-lateral-agent-state]] in the planned order | manifest readers and run-directory goldens |
| scenario schema | no constant | Increment 2 is schema version `2` in place: no new `schema_version` and no provenance field | — | the reader, migration, and compiler are unchanged in their version negotiation |

No Phase 1 baseline value, tolerance, interaction disposition, or golden trace
content changes except for the recorded definition versions and the additive
union those versions describe.

### Reconciling the Increment 1 deferrals

Every row Increment 1 left deferred that Increment 2 now owns is reconciled here;
the rows it does not own stay deferred with their later incumbents named. No
Increment 0 or Increment 1 field meaning, version-negotiation rule, or provenance
field changes.

| Increment 1 deferral | Increment 2 disposition |
| --- | --- |
| *Free lateral position selection, lane and facility transitions, gap prediction, manoeuvre commitment, overtaking and passing, close-pass evidence, and contextual wrong-way selection are Increment 2* | Owned by this section: `lateral`, `lateral_policy`, `facility_adjacencies[]`, `clearance_bands[]`, `maneuver_policy`, the corridors and clearance facts, the state machine and arbitration, the wrong-way decision and interval, and the evidence union. |
| Increment 1 table row — `tactics`: *`change_lane`, `overtake`, `pass`, `reverse_direction` in Increment 2* | Owned: `TacticKind` gains the four values with their compiled capabilities, and the Increment 1 narrow templates keep `follow`, `stop`, `yield` and compile unchanged. |
| Increment 1 table row — permissions: *`lane_use`/`overtake`/`crossing` in Increment 2* | Owned: the effect semantics, target kinds, resolution, and contradiction rules above. `stop_service` stays shape-only for Increment 4. |
| Increment 1 `facilities[]` sentence — *`lateral_use` is authored data the facility selector reads rather than an Increment 1 behavior* | Owned: `shared` and `centered` gain their free-lateral-motion behaviour, and `lateral_policy.passing_side` fixes the maneuver side. |
| Increment 1 `facility_connectors[]` row — directed end joins | Not reinterpreted: an Increment 1 connector keeps its exact meaning and geometry. Lateral adjacency is the separate additive `facility_adjacencies[]` shape. |
| Increment 1 `mode_templates[]` row — *bus/rigid truck/articulated Increment 3* | Not owned; stays deferred (Increment 3). |
| Increment 1 `body.kind` row — *`articulated_chain` stays deferred (Increment 3)* | Not owned; stays deferred (Increment 3). |
| Increment 1 `motion` row — *`articulated_wheeled` deferred* | Not owned; stays deferred (Increment 3). |
| Increment 1 `occupancy` row — *`fixed` and `transit` deferred* | Not owned; stays deferred (Increments 3 and 4). |
| Increment 1 transit row — *transit stops Increment 4* | Not owned; stays deferred (Increment 4). |
| Increment 1 row — *`serve_stop` in Increment 4* | Not owned; stays deferred (Increment 4). |
| Increment 1 pedestrian-groups row — *Increment 5* | Not owned; stays deferred (Increment 5). |
| Increment 1 navigation-mesh row — *stays deferred* | Not owned; stays deferred. Phase 2 does not require one, and this contract adds no mesh, no sidewalk-specific behaviour, no balance/lean/fall model, and no visibility-error model. |

### Increment 1 limitations this increment depends on

Two limitations Increment 1 recorded are not shape deferrals, but Increment 2
fixtures and evidence touch them; this section changes neither and adds no field
for either.

- Authored `paths[].points` compile to polylines, so an exact analytic curved
  reference is declared the way `narrow_isolated_curve_v2` declares it: the
  authored polyline is the chord approximation of a constant-curvature reference
  reconstructed with `CompiledReferencePath::arc`, and the fixture holds the
  round trip to `T-RT`. A curved-reference Increment 2 case follows that pattern
  rather than expecting a new path form.
- `demand[].spawn.rate.interval_s` is parsed and carried with its Increment 0
  meaning (`start_s`, nullable `end_s`) but is not simulated, as recorded in
  `FBK-028`. A fixture that needs a bounded arrival window must not assume the
  window is honoured until the leaf that owns the demand sampler implements it;
  this contract adds no field for it and no policy reads it.
