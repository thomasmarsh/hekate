# Schema version 2 contract

This is the authoritative definition of scenario schema version 2: the
Increment 0 subset and the deterministic mapping from a version-1 document to a
normalized version-2 document, extended in place by the *Increment 1 additions*
section at the end. Later increments cite this document and add to the shapes it
names; they do not reinterpret them.

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
  `crates/tangle-model/src/validate.rs:302` (the version check that emits
  `E_SCHEMA_VERSION`), `crates/tangle-model/src/compiled.rs:1369`, `:1688`
  (compiled `schema_version`), and `schemas/scenario-source.schema.json`.

## Version negotiation

- `SUPPORTED_SCHEMA_VERSION` becomes `2`. A normalized version-2 document is the
  only input the compiler consumes.
- The reader parses `schema_version: 1` and `schema_version: 2`. A version-1
  document is routed through the explicit migration (next section) before it
  reaches validation or compilation; it is never compiled by the version-2
  reader directly.
- Any version other than `1` or `2` is rejected with the stable diagnostic
  `E_SCHEMA_VERSION` (`crates/tangle-model/src/validate.rs:244`). The reader
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
  `crates/tangle-model/src/source.rs:613` (`ModeBodySource`), `:653`
  (`FacilityKind`), `:665` (`AccessSource`), `:682` (`ModeTemplateSource`),
  `:780` (`ScenarioSourceV2`); `crates/tangle-model/src/components.rs:285`
  (`NominalDirection`), `:299` (`SpeedPolicy`), `:325` (`AgentAccess`), `:93`
  (`AgentBody::Capsule`), `:163` (`AgentFamily::WheeledCapsule`), `:615`
  (`AgentBehaviorProfile`); `crates/tangle-model/src/validate.rs:1497`
  (`required_profile_params`); and `crates/tangle-model/src/mode_template.rs:290`
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
  (`CompiledPath::length`, `crates/tangle-model/src/compiled.rs:476`).
- `position_at(s)` — the world point at arc length `s`, clamped to
  `[0, length]` (`CompiledPath::position_at`,
  `crates/tangle-model/src/compiled.rs:481`).
- `heading_at(s)` — the tangent heading `theta(s)` in radians,
  counter-clockwise from the world x axis (`CompiledPath::heading_at`,
  `crates/tangle-model/src/compiled.rs:496`).
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
