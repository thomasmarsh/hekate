---
context_rev: 1
priority: P1
updated: 2026-09-14T13:26:43Z
summary: Build analytic and high-resolution clearance reference cases.
next: Add analytic and high-resolution reference minima for the straight, shift, curved, and body-shape pair cases.
---

Parent [[TAS-104-bound-predicted-versus-executed-clearance]].

# Outcome

Each accepted maneuver family has a hand-computable or high-resolution reference
minimum for front, rear, side, and swept clearance.

# Done when

- Straight constant-velocity, bounded lateral shift, curved reference, and
  body-shape pair cases have hand-computable or high-resolution reference minima
  for front, rear, side, and swept clearance.
- Every reference value is finite and deterministic.

# Context

Extends [[TAS-104-bound-predicted-versus-executed-clearance]]; reads the sim
seams in [[THO-014-increment-2-sim-lateral-maneuver-seams-frames-an]]. Owns the
reference cases; the production comparison is the sibling slice.
