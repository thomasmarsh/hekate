# Model-card template

A model card documents one replaceable behavioral model: what it reads and
writes, what it assumes, what has evidence, and where it stops being valid.
Every model shipped in Hekate carries a card in the module that implements it,
written as the module documentation so the card travels with the code.

This file is the inventory of record. The drift test in
`crates/hekate-sim/tests/model_cards.rs` reads the `##` headings below and
requires every card to state those sections in this order; a new required
section is an edit here first. Each heading names one section and the sentence
under it states what that section must contain. A card may add model-specific
sections — its equations, a steering law, a waypoint scheme — but it must keep
every required section below, in the order given, relative to the others.

## State

The state variables the model reads and writes: what fixes the agent's position
on its path and the size of its body, its motion state (speed, heading), and the
full output the model returns for one step.

## Parameters

The values sampled per agent from the scenario and carried with the agent,
naming each sampled field and the model symbol it binds. State that the
controller never substitutes its own value for a sampled parameter.

## Constants

The model's own fixed properties: each named constant with its value and units,
distinct from anything sampled, and documented where it is defined.

## Decision inputs

The exact context the kernel passes the model for one step — the constraints or
neighbours the kernel selected, their ordering, and the unit and meaning of each
field — and the statement that the kernel, not the model, owns which inputs
exist.

## Bounds

The hard limits the model's command respects: the acceleration, speed, heading,
or steering bounds, and how a command is clamped when the unconstrained law
exceeds them.

## Tie-breaks

The deterministic rule that resolves simultaneity or an exact tie — among
candidate constraints, neighbours, waypoints, or simultaneous gap claims —
naming the ordering key and the winner of an exact tie.

## Emergency backstop

The safety cap the kernel applies outside the model's own bounds, the physical
constraint it protects, and the counter (`Simulation::…_cap_steps`) that records
every step where it overrode the model's comfortable command.

## Assumptions

The modeling assumptions and scope the card rests on: what is deliberately not
represented, what the model expects the kernel to provide outside it, and any
statement that the model is not calibrated against field observations.

## Parameter sources

The provenance of every sampled parameter: whether it is authored scenario data,
a provisional engineering default, the parser's default, or derived, with the
default envelope when one exists, and an explicit statement that it is not
fitted to observations.

## Validated ranges

The operating ranges the model has evidence for: the sampled envelopes and
densities exercised by checked-in fixtures and the fidelity presets tested, and
the statement that behavior outside them is unvalidated rather than credible.

## Known failure modes

The ways the model is known to break or mislead: clipping or saturation,
degenerate or non-finite parameter cases, unmodeled geometry, and the reliance on
a kernel backstop where the model alone cannot guarantee a safe outcome.

## Incompatible fidelity settings

The fidelity settings the model cannot run under, or the settings it ignores.
Where a model is independent of the fidelity preset, say so explicitly rather
than leaving the section empty, and name any preset that would disable a bound or
cap the model depends on.
