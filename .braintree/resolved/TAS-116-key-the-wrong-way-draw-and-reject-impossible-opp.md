---
context_rev: 1
priority: P1
updated: 2026-09-14T15:13:25Z
summary: Key the wrong-way draw and reject impossible opposing options.
---

Parent [[TAS-097-make-contextual-wrong-way-decisions-reproducible]].

# Outcome

The wrong-way decision is reproducible and safe by construction: any random draw
is keyed to the versioned maneuver stream, and a physically impossible opposing
option is rejected before any claim or motion.

# Done when

- Random choice, if required, is keyed by root seed, run ID, stable AgentId, and
  the versioned maneuver stream; draw order and unrelated agents cannot change
  it.
- A physically disconnected opposing option yields an explicit rejection before
  any claim or motion; a legal prohibition stays available to a non-compliance
  decision.
- Fixed-seed, reversed-declaration-order, and unrelated-agent isolation tests
  pass.

# Context

Extends [[TAS-097-make-contextual-wrong-way-decisions-reproducible]]; reads
[[THO-015-increment-2-compiled-contract-and-model-seams-sc]]. Owns the draw
keying and the impossible-option rejection; do not add motion or routing.

# Result

Landed the keyed maneuver draw and the explicit impossible-option rejection.

- `crates/tangle-sim/src/rng.rs` (`STREAM_MANEUVER`, `derive_stream`,
  `uniform01`): added `pub const STREAM_MANEUVER: &str = "maneuver"`, documented
  like the other named streams, plus a test that it is independent of `demand`,
  `pedestrian_demand`, `profile`, and `compliance`.
- `crates/tangle-sim/src/wrong_way.rs`:
  - `maneuver_draw(root_seed, agent, ordinal) -> f64` is a pure function of
    `(root_seed, STREAM_MANEUVER, agent, ordinal)` built on the module's
    `derive_stream`/`uniform01`: it returns the agent's `ordinal`-th maneuver
    draw in `[0, 1)`. It is keyed by the root seed and the stable `AgentId` and
    never by how many other agents drew or in what order they were evaluated.
    `RunConfig` carries only a root seed and a step and has no run-id field, so
    the contract's run-id key component is documented as absent rather than
    invented in `RunConfig`.
  - `WrongWayInputs::physical_rejection() -> Option<WrongWayReason>` names the
    physical precondition once (`NoOpposingPath` when the opposing traversal is
    disconnected, else `NoNominalDirection` when the nominal direction is
    `either`); `decide` now delegates to it, so the procedure's order and every
    outcome are unchanged. It makes the distinction explicit and testable: a
    connected `prohibit`ed traversal returns `None` and reaches the draw, where
    a selected opposing option is `NoncompliantChoice`, while a disconnected
    traversal returns `Some(NoOpposingPath)` and rejects before any draw.
- `crates/tangle-sim/src/lib.rs`: re-exported `maneuver_draw` alongside the
  TAS-115 wrong-way surface.

Tests (in-crate, table-driven where natural): the draw is reproducible and in
`[0, 1)` across seeds, agents, and ordinals; it is keyed by seed, agent, and
ordinal; unrelated-agent isolation holds (interleaving and re-evaluating agent
B leaves agent A's draws byte-identical, B's stream is unchanged, and A and B do
not share a substream); a reversed declaration order yields identical per-agent
draw/decision records; a disconnected option reports
`physical_rejection() == Some(no_opposing_path)` and rejects for every draw; a
connected-but-`prohibit`ed option reports `physical_rejection() == None`, takes
the draw, and is `noncompliant_choice` when opposing is selected and
`compliant_choice` otherwise; an `either` direction reports `no_nominal_direction`.

Evidence: `cargo test -p tangle-sim --lib` 215 passed (8 new);
`cargo clippy --workspace --all-targets --all-features -- -D warnings` clean;
`cargo fmt --all --check` clean; `scripts/check-dependency-direction.sh` OK;
`braintree check` passes.

Remaining scope owned elsewhere: the runtime consumer that draws and routes the
selected opposing traversal belongs to
[[TAS-098-route-wrong-way-agents-through-ordinary-interactions]]; events and
metrics belong to [[TAS-099-increment-2-events-metrics-and-output]].
