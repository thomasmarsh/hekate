# Schema version 2 contract (Increment 0)

This is the authoritative definition of the Increment 0 subset of scenario
schema version 2 and the deterministic mapping from a version-1 document to a
normalized version-2 document. Later increments cite this document and add to
the shapes it names; they do not reinterpret them.

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
  `crates/tangle-model/src/source.rs:11` (`SUPPORTED_SCHEMA_VERSION`),
  `crates/tangle-model/src/validate.rs:244` (the version check that emits
  `E_SCHEMA_VERSION`), `crates/tangle-model/src/compiled.rs:1358`, `:1658`
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
`ScenarioProvenance` struct (`apps/tangle-cli/src/baseline.rs:76`), which is
embedded as `scenario` in both the run manifest
(`RunManifest` at `apps/tangle-cli/src/run_dir.rs:294`) and the checked-in
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
