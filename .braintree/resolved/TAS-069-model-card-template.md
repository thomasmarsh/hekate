---
context_rev: 1
priority: P1
updated: 2026-09-13T17:03:37Z
summary: Author a reusable model-card template and re-express the two Phase 1 model cards in it.
---

Parent [[TAS-058-agent-components-and-controller-stages]].

# Outcome

A reusable model-card template states the required sections for a Phase 2 model
— state, sampled parameters, constants, decision inputs, bounds, tie-breaks,
emergency backstop, assumptions, parameter sources, validated ranges, known
failure modes, and incompatible fidelity settings — and both Phase 1 cards
conform to it.

# Done when

- The template is checked in and names every required section from `PHASE_2_PLAN.md`.
- Both Phase 1 model cards conform to the template section order.
- The existing model-card drift test passes against the template inventory.

# Context

No gate; independent documentation deliverable of [[TAS-058-agent-components-and-controller-stages]].

# Result

`docs/model-card-template.md` (new) is the checked-in inventory of record: it
names the twelve required sections in order — state; sampled parameters;
constants; decision inputs; bounds; tie-breaks; emergency backstop; assumptions;
parameter sources; validated ranges; known failure modes; incompatible fidelity
settings — each with a sentence stating what that section must contain. Both
Phase 1 cards now carry every one of those sections in the template order, and
the drift test derives its inventory from the template instead of a hardcoded
list. The three `Done when` criteria hold.

- **The template is checked in and names every required section from
  `PHASE_2_PLAN.md`.** `docs/model-card-template.md` lists the twelve `##`
  headings in the plan's order: the seven Phase 1 fields plus the five Phase 2
  fields (assumptions, parameter sources, validated ranges, known failure modes,
  incompatible fidelity settings). The template is the single inventory of
  record; the drift test reads it rather than restating it.
- **Both Phase 1 model cards conform to the template section order.** The IDM
  card in `crates/tangle-sim/src/control.rs` and the waypoint card in
  `crates/tangle-sim/src/pedestrian.rs` keep their existing state, parameters,
  constants, decision inputs, equations/steering, bounds, tie-breaks, and
  emergency-backstop sections and add the five required sections with truthful
  content for the existing models. The vehicle card's heading is normalized to
  `## Emergency backstop` (matching the template) and its intra-card anchor link
  updated; no equation, bound, tie-break, or backstop content is changed. The
  card index in `crates/tangle-sim/src/controller.rs` now names the template as
  the inventory of record.
- **The model-card drift test passes against the template inventory.**
  `crates/tangle-sim/tests/model_cards.rs` now `include_str!`s the template,
  parses its `##` headings into the required-section list, and asserts each card
  states every section in order (a `find` cursor that advances past each
  heading), still asserting each card names its model family and replaceable
  interface. `cargo test -p tangle-sim --test model_cards` passes (4 tests).

Evidence: `cargo test -p tangle-sim --test model_cards` passes with the
template-derived inventory, and the same ordered scan reports a section missing
when one is dropped (verified by removing `## Validated ranges` from the vehicle
card); `cargo test --workspace` passes with golden traces and baselines
unchanged; `scripts/check-dependency-direction.sh` reports `dependency
direction OK`; `braintree check` passes while the node is still `proposed`.

Handoff: after this node moves to `resolved`, `braintree check` reports the
expected `next-resolved-node` diagnostic for
[[TAS-058-agent-components-and-controller-stages]] because its `next` names this
resolved child; TAS-058 stays open and is owned by the coordinator.
