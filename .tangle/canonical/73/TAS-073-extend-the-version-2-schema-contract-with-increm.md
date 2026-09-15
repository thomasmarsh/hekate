---
status: resolved
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: Extend the version-2 schema contract with Increment 1 facility and mode-template shapes.
---

Parent [[TAS-019-phase-2-increment-1-facilities-narrow-modes]].

# Outcome

`docs/schema-v2-contract.md` gains an Increment 1 section that fixes the exact
version-2 source shapes and compiled semantics for continuous-width facilities,
facility connectors, nominal-direction and permission/obligation shapes, and the
`bicycle`/`scooter` mode templates. It extends the Increment 0 contract owned by
[[TAS-061-version-2-schema-contract]] **in place** (ownership decision recorded
here and in the coordinator report): the Increment 0 document already states that
later increments "add to the shapes it names; they do not reinterpret them", so
the deferred rows in its "What version 2 defers to later increments" section are
reconciled here and re-owned by Increment 1. This leaf does not edit the TAS-061
node file.

# Done when

- The contract enumerates, each with its exact field set and owning increment:
  `facilities[]` (continuous region plus optional reference path, usable width,
  nominal direction, mode access, lateral-use policy, speed policy), facility
  connectors, the nominal-direction/permission/obligation shapes, and the
  `bicycle`/`scooter` `mode_templates` fields (body dimensions, speed,
  acceleration, braking, steering response, lateral-clearance preference,
  facility access, compliance).
- It states explicitly that it extends the Increment 0 contract in place and
  records how every deferred row it now owns is reconciled, without reinterpreting
  an Increment 0 shape.
- The document names the compiled coordinate contract the geometry leaf must
  expose: arc length `s`, signed lateral offset `d`, tangent, normal, curvature,
  usable lateral intervals, adjacency/connectors, and separately authored
  nominal/permitted versus physically possible directions.
- `tangle check` passes and every seam the document cites resolves.

# Context

Per `PHASE_2_PLAN.md` Increment 1 (Facilities and narrow modes) and the sections
*Schema version 2*, *Facilities, paths, and free space*, and *Bicycles and
scooters*. Read [[TAS-061-version-2-schema-contract]] (the Increment 0 contract
this extends; do not edit its node file) and [[TAS-062-version-2-source-shapes]]
(the current version-2 shapes). No gate; first frontier action of
[[TAS-019-phase-2-increment-1-facilities-narrow-modes]]. Owns
`docs/schema-v2-contract.md` only.

# Result

`docs/schema-v2-contract.md` gains a new terminal section, **Increment 1
additions: facilities and narrow modes**, which extends the Increment 0 contract
in place. The Increment 0 text is unchanged except for the title/intro sentence
and refreshed seam line numbers; no Increment 0 shape, field meaning,
version-negotiation rule, or provenance field was reinterpreted.

Sections and field sets added:

- **Authority and scope** — plan sections, the six downstream leaves this
  contract serves, class `CC-NARROW` / `docs/benchmark-matrix.md` evidence and
  `docs/model-card-template.md`, and the existing code seams each shape
  compiles into.
- **Increment 1 source shapes** — exact field sets for:
  - `facilities[]`: `id`, `region`, `reference_path` (optional), `width_m`,
    `nominal_direction` (`forward`/`reverse`/`either`), `access.modes`,
    `lateral_use` (`shared`/`centered`), `speed_policy.limit_mps` (`null` =
    unlimited); with a normalized example.
  - `facility_connectors[]`: `id`, `from {facility, direction}`,
    `to {facility, direction}` with `forward`/`reverse` traversal directions.
  - `permissions[]`: `id`, `kind` (`nominal_direction`/`lane_use`/`overtake`/
    `crossing`/`stop_service`), `holder`, `target`, `effect` (`permit`/
    `prohibit`/`obligate`), plus a per-kind owning-increment table.
  - narrow `mode_templates` fields: `body.kind: 'capsule'` (`length_m`,
    `radius_m`), `access.facility_kinds` gaining `facility`,
    `access.nominal_direction`, `access.speed_policy`, and the
    `capsule` + `single_body_wheeled` profile set adding
    `steering_rate_max_rad_s` and `lateral_clearance_m`; `bicycle`/`scooter`
    normalized templates (values marked provisional).
- **Compiled coordinate contract** — the geometry leaf's required surface:
  `length`, `position_at(s)`, `heading_at(s)`, `tangent_at(s)`, `normal_at(s)`,
  `curvature_at(s)`, usable lateral interval `[d_min, d_max]` from `width_m`
  minus body envelope and clearance, adjacency/connectors, and nominal vs
  permitted vs physically possible directions; sign conventions and the
  projection / `T-RT` round-trip rule.
- **Increment 1 validation rules** — containment, usable width, curvature vs
  turning limits, connector continuity, directional reachability,
  mode-to-facility access, legal vs physically possible, spawn clearance (codes
  owned by TAS-075).
- **Reconciling the Increment 0 deferrals** — row-by-row table.
- **What Increment 1 still defers** — Increment 2/3/4/5 scope and the
  navigation mesh.

Deferred rows now owned by Increment 1: `facilities[]`, facility connectors,
`permissions[]` (shape plus `nominal_direction`), the bicycle/scooter
`mode_templates`, the `capsule` body kind, and the `mode_templates` `access`
facility-kind/nominal-direction/speed-policy sentences. Rows explicitly left
deferred: `articulated_chain` body, `articulated_wheeled` motion, heavy and
articulated templates (Increment 3), `lane_use`/`overtake`/`crossing`
permissions (Increment 2), `stop_service` permission and transit occupancy/stops
(Increment 4), pedestrian groups and social state (Increment 5), and the general
navigation mesh.

Acceptance:

- `tangle check` — `graph check: passed (118 nodes)` while this node was still
  in `proposed/`.
- Seam resolution — 22 file:line citations verified with `sed -n '<line>p'`
  (source.rs 17/613/653/665/682/780, components.rs 93/163/285/299/325/615,
  compiled.rs 476/481/496/1369/1688, validate.rs 302/1497, mode_template.rs 290,
  baseline.rs 88, run_dir.rs 301); all 7 wikilinks resolve to node files under
  `.tangle/`; the `CC-NARROW` / `T-RT`/`T-DIM`/`T-ENV`/`T-SPD` names resolve
  in `docs/benchmark-matrix.md`.
- `git status --porcelain` — only `docs/schema-v2-contract.md` changed; no
  source, schema, or other node file touched. Docs only, so no crate tests were
  required or run.

Post-move note: after this file moves to `resolved/`, `tangle check`
transiently reports `next-resolved-node` because
[[TAS-019-phase-2-increment-1-facilities-narrow-modes]] still names TAS-073 in
its `next`. That is expected; the coordinator advances the parent's `next`.
TAS-019 and every other parent/sibling node were left untouched.
