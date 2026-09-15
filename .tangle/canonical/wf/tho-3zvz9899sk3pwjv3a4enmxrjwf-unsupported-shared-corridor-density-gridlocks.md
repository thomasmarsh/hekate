---
context_rev: 1
status: resolved
updated: 2026-09-15T12:45:54Z
summary: Unsupported shared-corridor density gridlocks; predict_candidate is 95% of the tick.
---

Area [[IDX-001-hekate]].

# Scope

Durable reconnaissance for the Increment 2 representative performance profile
(TAS-110). Verified at commit 98ba8d3.

# Finding

At the Increment 2 workload's first-draft shared-corridor density (all six
modes on one shared 10 m two-way facility, passenger cars at 300 arrivals/h per
direction, 960 arrivals/h total) the workload does not flow: 2 000 ticks
admitted 28 bodies and released none, the live population grew with the clock,
and per-tick cost grew superlinearly with it (440, 2 120, 5 800, 9 300 us/tick
at 500, 1 000, 2 000, 4 000 ticks on the release binary). 95.7% of that cost is
inside `Simulation::predict_candidate` (`crates/hekate-sim/src/sim.rs:2301`),
because the per-candidate clearance primitives (`closest_point_on_box`,
`body_contact_normal`, `box_box_least_overlap_axis`, `body_clearance_m`) scale
with the candidate population, not with the tick count.

# Evidence

`perf/README.md` records the cost curve, the profile, and the density as
labelled unsupported, where a uniform hour-long sweep is infeasible and no
completed run backs a claim above the flowing density. The reshaped flowing
workload is roughly flat at 1.4-1.7 ms/tick (1 438-1 742 us/tick over
10 000-72 000 ticks); the bounded lateral ablation at seed 11, 5 000 ticks,
three passes, measures 863.8 us/tick enabled against 25.5 us/tick for the
disabled twin (`scenarios/phase2/inc2/mixed_mode_profile_v2_no_lateral.json5`),
so the lateral machinery is 97.1% of the bounded per-tick cost. TAS-110's
result records the counters, the 95.7% figure, and the budgets.

# Consequence

Any future performance claim must name the density it was measured at: the
superlinear regime is a real pathology of a shared corridor, not a fixture
artifact, and the dominant term is the tactical candidate scan. Optimizing
anything but `predict_candidate`'s candidate loop cannot move the pathological
regime.

# Seam

`crates/hekate-sim/src/sim.rs` (`Simulation::predict_candidate`,
`Simulation::performance_counters`), `crates/hekate-sim/src/index.rs`,
`perf/README.md`, `perf/release-bench.json`,
`perf/profiles/mixed_mode_profile_v2-release.*`.
