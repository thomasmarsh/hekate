---
context_rev: 1
priority: P1
updated: 2026-09-12T01:30:53Z
summary: Parse and validate the walking-skeleton JSON5 scenario into an immutable compiled scenario.
next: Define the Serde source structs for one path plus two portals and compile them into dense IDs.
---

# Context

Depends on [[DEC-001-phase-1-kickoff-defaults]] at context_rev 1.

# Outcome

`tangle-model` parses a versioned JSON5 scenario containing one guide path and
a portal at each end, validates it, and exposes an immutable compiled scenario
with stable string IDs mapped to dense integer IDs.

# Done when

- The source schema is versioned and parsed with Serde; a JSON Schema is
  generated and checked in.
- Semantic validation rejects duplicate IDs, non-finite coordinates,
  non-positive dimensions or durations, and unreachable portals with stable
  diagnostic codes and source object IDs.
- Compilation exports the string-to-integer ID mapping in run provenance.

Parent [[TAS-001-phase-1-increment-0]].
