---
context_rev: 1
status: resolved
updated: 2026-09-15T12:45:53Z
summary: Endpoint-fold clearance reports the overlap boundary, not the penetration depth.
---

Area [[IDX-001-hekate]].

# Scope

Durable reconnaissance for the Increment 2 predicted-versus-executed clearance
work (TAS-104, TAS-126). Verified at commit 98ba8d3.

# Finding

`tick_minimum_clearance_m` (`crates/hekate-sim/src/metrics.rs:493`) starts from
`min(start, end)` and returns that value immediately when it is `<= 0`
(metrics.rs:495-500): an endpoint overlap is treated as bounding the tick.

The interior turning-point search it uses otherwise is bracketed by
`clearance_rate` (`crates/hekate-sim/src/swept.rs:281`), whose reported value is
the relative displacement projected onto `body_contact_normal`. The module card
states that this rate "is zero where the bodies already overlap"
(swept.rs:66-69), so for any pair that overlaps, the closing predicate is flat
and the search cannot locate the deepest penetration inside the tick. The
reported minimum is then the overlap boundary (0 at the contact instant, or the
shallower endpoint penetration), not the penetration depth.

# Evidence

`tick_minimum_clearance_m` and `clearance_rate` therefore report the overlap
boundary rather than the penetration depth once two bodies overlap, which is
what breaks tolerance T-O1 by about 1.5 m for out-of-matrix fast lateral
crossings: the crossing pair separates by the tick end, so the tick minimum
occurs in the interior while both endpoints sit on or outside the boundary.
TAS-126's production-prediction-versus-executed-clearance probe found it.

# Seam

`crates/hekate-sim/src/metrics.rs` (`tick_minimum_clearance_m`,
`clearance_at`), `crates/hekate-sim/src/swept.rs` (`clearance_rate`,
`body_contact_normal`).
