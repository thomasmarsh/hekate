---
context_rev: 2
priority: P1
updated: 2026-09-13T17:30:17Z
summary: Checked in the Phase 2 benchmark matrix and quantitative tolerances for independent, pairwise, and mixed-mode validation.
---

Parent [[TAS-018-phase-2-increment-0-baseline-extension-contract]].

# Outcome

Per `PHASE_2_PLAN.md` Increment 0 and the validation ladder: a checked-in
benchmark matrix names every independent-mode, pairwise, and mixed-mode cell,
its fixture, and the quantitative tolerance that decides pass or fail, including
which fidelity presets apply.

# Done when

- The matrix covers every Phase 2 mode, every material mode pair, and each interaction family with an explicit supported, impossible, or deferred disposition.
- Every cell names its fixture, the quantity compared, the tolerance, and the fidelity presets.
- Tolerances are metric bounds, not golden-file equality, and each names the baseline it derives from.

# Context

Owned by [[TAS-018-phase-2-increment-0-baseline-extension-contract]]; this is a planning artifact for later increments, not an implementation task.

# Result

`docs/benchmark-matrix.md` (new) is the checked-in Phase 2 benchmark matrix and
tolerance catalogue; `docs/benchmark-matrix.json` (new) is its machine-readable
companion, generated from the same rules and verified cell-for-cell against the
Markdown grid. No scenario, test, or kernel code is added.

## Coverage

- **Modes:** 7/7 — `passenger_car`, `pedestrian`, `bicycle`, `scooter`, `bus`,
  `rigid_truck`, `tractor_semitrailer`; each carries an independent-mode cell
  with disposition `supported` and an owning fixture/increment.
- **Pairs:** 28/28 in the pairwise grid — `C(7, 2)` cross-mode plus 7 same-mode,
  each appearing exactly once as a row.
- **Families:** 8/8 — following, crossing, merging, overtaking, head-on/opposing,
  shared-space, stop-service, and the reserved `unknown` classification, each
  appearing exactly once as a grid column.
- **Cells:** 224 = 28 x 8, each with exactly one disposition.
- **Dispositions:** supported 157, impossible 67, deferred 0. Family counts:
  following 22S/6I, crossing 28S/0I, merging 22S/6I, overtaking 21S/7I,
  head-on/opposing 22S/6I, shared-space 7S/21I, stop-service 7S/21I,
  unknown 28S/0I.
- **Mixed-mode cells:** `MIX-1` (all modes, nominal demand, one bus stop),
  `MIX-2` (saturated demand), `MIX-3` (Standard/Fine conclusion stability),
  owned by Increment 7.

## Done-when verification

1. **Every mode, pair, and family has an explicit disposition.** §4.1 lists all
   seven modes with `supported`; §8 gives all 28 pairs x 8 families with one
   disposition per cell; `I:FD`, `I:OP`, `I:WWS`, `I:NOBUS` name the
   impossible-cell reasons and every unsupported cell uses one of them. The
   matrix's coverage-audit checklist (§10) confirms by count that every mode,
   pair, and family appears exactly once. No cell is `deferred`: Increment 5
   commits a pairwise fixture for every physically possible cell, so each is
   `supported`; §10.1 states this and §11 names what Phase 2 defers outside the
   taxonomy.
2. **Every cell names fixture, quantity, tolerance, and presets.** A supported
   cell is written `S:<class>`; each class in §5 fixes the fixture resolved
   through §7.3, the quantity compared, the numeric tolerance bound and its
   baseline, and the fidelity presets (`F`/`S`/`f` for independent cells, `S`/`f`
   for interaction cells). §7 names the existing Phase 1 fixtures and every
   planned fixture path with the increment that creates it (the pairwise
   convention `scenarios/phase2/inc5/pair_<a>__<b>__<family>_v2.json5` plus the
   named Increment 1/2/3/4/6/7 fixtures). The JSON companion is checked to carry
   the same class for every supported cell and the same reason for every
   impossible cell.
3. **Tolerances are metric bounds, each naming its baseline.** §6 lists 22
   tolerance ids, every one a bound (`<=`, `>=`, or `= 0`) on a named metric or
   geometric quantity; none is a golden-file or trace-hash comparison. §3 fixes
   two baseline sources: the Phase 1 baseline (`baselines/phase1/baseline.json`
   and the Phase 1 fixtures) for every `passenger_car`/`pedestrian` cell, and the
   owning increment's declared reference for new modes (Increment 1 analytic path
   reference, Increment 2 fine-step executed clearance, Increment 3
   high-resolution articulated reference, Increment 4 cohort ledger, Increment 5
   pairwise/group reference fixtures, Increment 6 ambiguity fixture, Increment 7
   release spec). The exact determinism/replay trace-hash checks are explicitly
   separated from the tolerance catalogue as gates, not tolerances (§7.3).

## Evidence

- `docs/benchmark-matrix.md` and `docs/benchmark-matrix.json` are committed; the
  JSON's 224 cells and the Markdown grid agree exactly (checked by parsing both).
- `cargo test --workspace` passes with no code touched.
- `braintree check` passes on the vault before resolution.

Handoff (outside this leaf's write set): after this node is `resolved/`, the
parent [[TAS-018-phase-2-increment-0-baseline-extension-contract]] still names it
in `next`, which `braintree check` reports as one expected `next-resolved-node`
diagnostic. The coordinator owns advancing that parent's `next`.
