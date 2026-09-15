---
context_rev: 1
status: resolved
updated: 2026-09-15T12:45:53Z
summary: Anti-overlap cap fires once per executed pass on the passed body; no lateral gate.
---

Area [[IDX-001-hekate]].

# Scope

Durable reconnaissance for the Increment 2 passing work (TAS-106, TAS-129).
Verified at commit 98ba8d3.

# Finding

`nearest_leader` (`crates/hekate-sim/src/sim.rs:3153`) selects a longitudinal
leader by path identity plus forward progress, excluding only the body an
active lateral maneuver is passing (`sim.rs:3158-3160`). It carries no lateral
gate, so a body the passer has already drawn level with on the same facility is
still selected as the leader.

When a completed same-facility pass puts the passer's rear level with the passed
body's front, the passed body's bumper gap therefore clamps to zero and the
kernel's anti-overlap position cap fires for one step on the *passed* body, not
on the passer. The cap fires once per executed same-facility pass.

# Evidence

`crates/hekate-sim/tests/inc2_passing_fixtures.rs` module docs record the
relationship ("a completed within-facility pass also engages the kernel's
anti-overlap position cap for one step on the passed body"), skip exactly the
step on which the run's cap counter advanced in the motion-envelope assertion,
and assert the counter directly: zero for a change of lane, one per executed
pass otherwise. The landed model fixture in `crates/hekate-sim/tests/narrow_passing.rs`
reports the same one-cap-per-executed-pass relationship. The maneuver clears no
corridor through the cap and records no contact.

# Consequence

The cap is a documented last-resort backstop, not a maneuver reliance, but any
envelope or speed-limit assertion over a same-facility pass must exempt the one
capped step on the passed body. Related to TAS-029.

# Seam

`crates/hekate-sim/src/sim.rs` (`nearest_leader`, the anti-overlap position cap
in `command_motion`), `crates/hekate-sim/tests/inc2_passing_fixtures.rs`,
`crates/hekate-sim/tests/narrow_passing.rs`.
