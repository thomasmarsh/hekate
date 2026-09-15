---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T11:30:39Z
summary: Check in presenter fixtures and the negative source guard.
---

Parent [[TAS-111-present-increment-2-corridor-gap-and-rule-overlays]].

# Outcome

One passing and one occupied-opposing fixture render with backend parity, and a
negative guard proves shared presentation modules carry no scenario or mode branch.

# Done when

- One passing fixture and one occupied-opposing fixture open through the version-2
  loader and show parity in golden and shared-scene tests.
- A source-text negative guard plus falsification probe rejects scenario names and
  Increment 2 mode names in shared presentation modules.
- Presenter, TUI, viewer, workspace, dependency-direction, and relevant golden
  tests pass.

# Context

Gated on [[TAS-100-version-the-maneuver-event-and-trace-surface]].
Gated on [[TAS-101-measure-close-passes-with-exact-clearance-evidence]].
Gated on [[TAS-102-record-wrong-way-intervals-and-disaggregated-metrics]].
Gated on [[TAS-106-check-in-increment-2-passing-fixtures]].
Gated on [[TAS-107-check-in-contextual-wrong-way-fixtures]].
Extends [[TAS-111-present-increment-2-corridor-gap-and-rule-overlays]]; reads
[[THO-016-increment-2-event-metric-trajectory-presenter-an]]. Owns the fixtures and
the negative guard; the overlay mapping is the sibling slice.

# Result

Fixtures and guard checked in; no new scenario file was needed, so
`scenarios/**` is untouched and the two fixtures load from the existing
`scenarios/phase2/inc2/` paths. `crates/hekate-present/tests/inc2_fixture_overlays.rs`
opens `narrow_passing_v2` and `narrow_wrong_way_v2` through `load_scenario` (asserting
schema version 2), drives both through `PresentationController` with each step's
records folded, and asserts: the passing fixture carries corridor/target-offset/
predicted-gap/maneuver (the one executed `pass`, `edge attempted`, `reason
slower_leader`, left side, partner named, interval's last frame the `returning`
edge) and no opposing traversal; the occupied opposing corridor carries corridor +
wrong-way for the turned rider (`facility d`, movement `through_d`, reverse vs
forward nominal, `permit`/`legal_permission`, `violating false`, interval open to
the run end) after the entry request the TAS-131 driver records; replay is
byte-identical in both fixtures; Phase 1 carries none of the five and Increment 1
carries none of the four sparse/interval families. The new golden
`tests/golden/present/inc2_tactical_fixtures.seed0.txt` pins the milestone ticks
(18/712/1097 and 121/run end) and the exact shared inspector text; regenerate with
`UPDATE_GOLDENS=1 cargo test -p hekate-present --test inc2_fixture_overlays` (the
script `scripts/regen-goldens.sh` was not in this slice's write set, so it does not
name the new suite yet).

`crates/hekate-present/tests/tactical_no_special_case.rs` mirrors the Increment 1
no-branch guards: every module under `crates/hekate-present/src` (read from disk, so
a new shared module is guarded automatically) and the four app shape/inspector
modules (`raster.rs`, `pixel.rs`, `hud.rs`, viewer `lib.rs`/`main.rs`) are asserted
to carry no Increment 2 scenario id (all eight in `scenarios/phase2/inc2/`) and no
Increment 2 mode id (`bicycle`, `scooter`, `passenger_car`) as a Rust string literal
in production code; `#[cfg(test)]` regions and doc prose are excluded, and the
falsification probe pins that a literal comparison is caught while prose and test
fixtures are not. End-to-end falsification was executed by hand: injecting a mode
literal into `crates/hekate-present/src/tactical.rs` failed the guard with the
module and name, then was reverted.

Backend parity: `apps/hekate-tui/tests/inc2_fixture_sessions.rs` drives the passing
fixture to the pinned pass tick in whole capped frames, selects the maneuvering
body, and asserts the terminal footer renders the five shared summaries verbatim,
and that the occupied opposing fixture opens and draws with the five overlay toggles
in its legend; `apps/hekate-viewer/tests/inc2_fixture_frames.rs` feeds both runs
through the Bevy `CurrentFrame` backend and asserts each capture is the projected
frame carrying the overlays, that `body_visuals` resolves every fixture body, and
that a Phase 1 run captures none.

Verified: `cargo fmt --all --check`; `cargo test -p hekate-present`; `cargo test -p
hekate-tui`; `cargo test -p hekate-viewer`; `cargo clippy -p hekate-present -p
hekate-tui -p hekate-viewer --all-targets -- -D warnings`;
`./scripts/check-dependency-direction.sh`; `tangle check` (195 nodes). The workspace
suite is the resolver's gate and was not run here (per task authority).

Nuance recorded, not a blocker: the done-when's "graceful absence for Phase 1/Inc 1"
holds exactly for Phase 1 (no primitive at all) and for Increment 1's four sparse or
interval families; an Increment 1 version-2 narrow body does carry route state on a
compiled band, so its usable corridor is projected and drawn. That is the corridor
primitive's own contract (TAS-135 documented it as present wherever route state and
a band exist), and the test asserts it explicitly instead of claiming an absence the
code does not have.

Five-gate validation (resolver, `083d6f8`):

| gate | command | exit | wall | result |
| --- | --- | --- | --- | --- |
| 1 | `cargo fmt --all --check` | 0 | 1s | clean |
| 2 | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 0 | 1s | clean |
| 3 | `cargo test --workspace --all-features` | 0 | 349s | 1049 passed; 0 failed; 3 ignored; 97 suites |
| 4 | `./scripts/check-dependency-direction.sh` | 0 | 1s | `dependency direction OK` |
| 5 | `tangle check` | 0 | <1s | `graph check: passed (195 nodes)` |

Done when met: both fixtures open through `load_version_2` (asserting
`schema_version() == 2`) and carry all five overlays with golden parity in
`crates/hekate-present/tests/inc2_fixture_overlays.rs`, plus terminal
(`apps/hekate-tui/tests/inc2_fixture_sessions.rs`) and Bevy
(`apps/hekate-viewer/tests/inc2_fixture_frames.rs`) parity; the source-text
negative guard and its falsification probe in
`crates/hekate-present/tests/tactical_no_special_case.rs` reject scenario and
mode names in shared modules; all five gates are green.
