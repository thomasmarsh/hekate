---
context_rev: 1
updated: 2026-09-13T18:28:51Z
summary: Extend the version-2 schema contract with Increment 1 facility and mode-template shapes.
next: Extend docs/schema-v2-contract.md with the Increment 1 facility, connector, permission, and bicycle/scooter template shapes.
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
- `braintree check` passes and every seam the document cites resolves.

# Context

Per `PHASE_2_PLAN.md` Increment 1 (Facilities and narrow modes) and the sections
*Schema version 2*, *Facilities, paths, and free space*, and *Bicycles and
scooters*. Read [[TAS-061-version-2-schema-contract]] (the Increment 0 contract
this extends; do not edit its node file) and [[TAS-062-version-2-source-shapes]]
(the current version-2 shapes). No gate; first frontier action of
[[TAS-019-phase-2-increment-1-facilities-narrow-modes]]. Owns
`docs/schema-v2-contract.md` only.
