---
context_rev: 1
status: resolved
updated: 2026-09-15T12:45:06Z
summary: No production caller opens a wrong-way interval; its fixtures and goldens are in-process.
---

Area [[IDX-001-hekate]].

# Scope

Durable reconnaissance for the Increment 2 wrong-way work (TAS-124, TAS-131,
TAS-133). Verified at commit 98ba8d3, where Increment 2 closed.

# Finding

`Simulation::request_wrong_way_entry` (`crates/hekate-sim/src/sim.rs:1231`) is
the only seam that opens an opposing-traversal interval, and no CLI command,
demand model, or routing path calls it. Every caller is a test
(`crates/hekate-sim/tests/wrong_way.rs`,
`apps/hekate-cli/tests/run_metrics.rs`,
`crates/hekate-present/tests/inc2_fixture_overlays.rs`,
`apps/hekate-viewer/tests/inc2_fixture_frames.rs`). The checked-in fixture
`scenarios/phase2/inc2/narrow_wrong_way_v2.json5` therefore reaches the seam
only through an in-process driver, and its metrics and trace goldens are
recorded in-process through `TraceRecorder` rather than by `hekate-cli run`.

`apps/hekate-cli/tests/inc2_trace.rs` states the gap in its own module docs:
the wrong-way fixture needs `Simulation::request_wrong_way_entry`, and neither
that seam nor `Simulation::request_lateral_maneuver` "is exposed by a CLI
command", so the golden is not CLI-run coverage and the suite never claims it
is.

# Consequence

The wrong-way acceptance evidence proves the kernel seam, not a production
path: a CLI-driven wrong-way run, its run-directory manifest, and its golden
are unproven. Any later claim that wrong-way travel is reachable by a user
needs a demand, router, or command path that calls the seam first.

# Seam

`crates/hekate-sim/src/sim.rs` (`Simulation::request_wrong_way_entry`),
`apps/hekate-cli/tests/inc2_trace.rs` (module docs, in-process recording),
`crates/hekate-present/tests/inc2_fixture_overlays.rs`,
`apps/hekate-viewer/tests/inc2_fixture_frames.rs`.
