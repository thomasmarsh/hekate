---
status: proposed
context_rev: 1
updated: 2026-09-15T01:22:43Z
summary: Phase 2 Increment 2 session: closure gaps, a future timestamp, timeouts, heading frames, and a compiled-adjacency gap.
tangle_revision: 0.6.0+g169bad5
---

Area [[IDX-002-tangle-feedback]].

# Feedback

Attempted: Orchestrated Phase 2 Increment 2 (TAS-020) leaf by leaf as the coordinator: verified the groomed tree with one scout, dispatched one fresh worker per leaf, verified each, advanced parent routes, and committed. Resolved the contract workstream (TAS-082, leaves 083-086), the lateral-machinery workstream (TAS-087, leaves 088-091), and two passing leaves (TAS-093, TAS-094); TAS-095 is active with a recorded remaining slice.
Friction: Repeated compiler-forced closure paths fell outside declared write sets (four leaves); one worker stamped a future `updated`; two workers timed out at 30 minutes; the wheeled heading-frame convention was undocumented and surfaced as a misleading predictor verdict; the contract's `file:line` citations had rotted; and `CompiledFacilityAdjacency` cannot express the contract's geometric handoff.
Improvement: See the findings below; the highest-value change is naming the full compile/golden closure and every resolved-sibling compiler seam in the leaf brief before dispatch, because four of eleven leaves hit that gap.

## Finding 1 — compiler-forced closure paths outside the declared write set

Attempted: dispatched TAS-084 (parse), TAS-086 (validate), TAS-090 (predict), and TAS-094 (motor overtaking) with write sets copied from their node `# Context`.
Friction: each needed a resolved sibling's file to keep the workspace green or the component-driven behavior real. TAS-084's new `TacticKind` variants made the exhaustive `compiled_tactic` match non-exhaustive (`mode_template.rs`, TAS-085's file). TAS-086's tightened contract invalidated a TAS-085 test fixture and needed the shared `required_profile_params` seam to take the `lateral` coupling. TAS-090 could not construct `CompiledFacility` in tests (no public constructor). TAS-094 found that `compiled_profile` attached only `lateral_accel_max_mps2` for a lateral box, dropping `steering_rate_max_rad_s`/`lateral_clearance_m` that `validate_v2` required — TAS-089 had recorded a `profile.rs` gap that pointed at the wrong file. Three workers had to stop and ask; one fix was a real compiler defect.
Improvement: the coordinator should compute the write set as the compile-and-golden closure before dispatch and name, in the brief, every resolved-sibling file the change can force (`mode_template.rs`, `components.rs`, `serialize`/exhaustive matches, generated schema, goldens, lockfile), plus the "gates my artifact enters" line. When a leaf's Done when needs a compiled component, the brief should confirm the compiler already produces it or name that seam in the write set.

## Finding 2 — future `updated` stamp (recurrence of FBK-026 Finding 4)

Attempted: TAS-083 stamped its resolved node `updated: 2026-09-14T00:55:00Z` while the host clock read `2026-09-14T00:50:52Z`.
Friction: the clamp rule then forces every later edit to carry the future stamp or move it backwards. This is the third session to record it.
Improvement: the worker handoff template should require `date -u +%Y-%m-%dT%H:%M:%SZ` for `updated`, and the coordinator should verify and correct a future stamp at integration (done this session).

## Finding 3 — the documented timed-out-worker recovery worked

Attempted: TAS-091 and TAS-093 each hit the 30-minute window with uncommitted partial work.
Friction: recovery is still costly (an extra run each), but this session used the now-documented procedure: inspect the partial diff, run the touched crate's tests, and decide. TAS-091's partial was green and needed only a narrow finishing run (Result + status move); TAS-093's partial was red on one reverse-direction case.
Improvement: the recovery subsection is effective and should stay. Add one branch the procedure lacks: when the partial state is red on one localized, understood case and the rest is green, authorize a narrow repair run that keeps the partial slice, rather than the blanket "revert and re-scope"; a blanket revert discards valuable near-complete work. Record which branch was taken and why.

## Finding 4 — undocumented heading frame surfaced as a predictor verdict

Attempted: TAS-093's reverse-direction pass never completed.
Friction: `AgentStore.heading_rad` stores the reference-tangent heading, while `bounded_steering_step` and the prediction corridor are expressed in the travel-heading frame; nothing cross-referenced the two. For a reverse traveller they differ by pi, so the controller saw a pi heading error and the failure surfaced as `Infeasible { limiting: BandEdge }`, not a frame error, making diagnosis indirect and costing a full timed-out run.
Improvement: require every heading field's doc comment to name the frame it stores and the frame each consumer expects, or convert at one typed boundary. A predictor verdict that rejects every candidate for an entire travel direction is a signal to check frame conventions first.

## Finding 5 — contract `file:line` citations rot silently

Attempted: TAS-083 extended `docs/schema-v2-contract.md` and trusted its existing seam citations.
Friction: 12 of the 22 `file:line` citations had drifted to blank lines or unrelated symbols after Increment 1 landed; nothing checks a document's citations (`tangle check` validates graph structure only).
Improvement: cite `path (Symbol)` and add a line only when the citing leaf owns the file, or add a repo doctor that fails when a `file:line` citation does not contain its named symbol.

## Finding 6 — compiled adjacency cannot express the contract's geometric handoff

Attempted: TAS-095 implemented the contract's "ownership moves at the geometric handoff" for a lateral facility transition.
Friction: `CompiledFacilityAdjacency` carries a side but no band separation/offset, so the destination reference's offset from the source is unknown and the body centre at the handoff sits outside the destination's usable interval; both the steering step and the predictor reject the entry transient. TAS-095 was left active with a recorded next rather than approximated.
Improvement: compile a cross-band offset (a per-adjacency shared-boundary lateral coordinate) into `CompiledFacilityAdjacency`, or add a kernel helper that derives one from the two reference geometries at compile time, so the handoff predicate, corridor, and predictor read one compiled boundary. A contract clause that names a geometric handoff should name the compiled datum that realizes it.

## Finding 7 — mixed lateral-capability scenario panicked

Attempted: TAS-094 ran a v2 scenario mixing a lateral mode with a lateral-incapable mode on one facility.
Friction: `resolve_maneuvers` Phase 2 assumed every route-state agent carried a target clearance and panicked when one did not; TAS-093's tests never hit the mix.
Improvement: the leaf brief for a shared-stage change should name the mixed-capability adversarial case as an acceptance input, so a stage invariant is exercised against agents that do not carry the new state.

## Finding 8 — structural leaf rules are not checked

Attempted: Phase A verified every leaf's `next` form (plain sentence vs single wikilink), gate placement, and `# Context` structure by hand.
Friction: `tangle check` validates graph integrity, not the `next`/gate structural rules the refresh brief required, so the scout had to hand-roll grep checks.
Improvement: add a `tangle check` lint for the leaf-`next` and gate-structure rules (plain action sentence for a leaf, single direct-child wikilink for a coordinator, `Gated on` only in `# Context`), so a refresh scout does not reimplement them.
