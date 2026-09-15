---
status: resolved
context_rev: 1
priority: P1
updated: 2026-09-15T01:22:43Z
summary: Make contextual bicycle and scooter wrong-way decisions reproducible and inspectable.
---

Parent [[TAS-096-contextual-wrong-way-travel]].

# Outcome

An eligible narrow agent makes one deterministic, reason-coded decision about
opposing traversal from perceived rule, route savings, expected delay, facility
type, observed density, urgency, and its stable compliance profile.

# Done when

- The decision reads only the observer-stage context fixed by TAS-083 and does
  not add a detailed visibility-error subsystem.
- Random choice, if required, is keyed by root seed, run ID, stable AgentId, and
  the versioned maneuver stream; draw order and unrelated agents cannot change
  it.
- Decision output contains perceived permission/obligation, selected nominal or
  opposing option, reason code, relevant movement IDs, and the context values
  used.
- No physically connected opposing option yields an explicit rejection before
  any claim or motion; a legal prohibition remains available to a
  non-compliance decision.
- Table-driven and fixed-seed tests cover each reason, threshold ties,
  compliance extremes, reversed declaration order, and unrelated-agent
  isolation.

# Context

Depends on [[TAS-086-validate-increment-2-lateral-and-wrong-way-policy]] at context_rev 1.
Depends on [[TAS-091-resolve-gap-claims-and-maneuver-transitions]] at context_rev 1.
Owns wrong-way observation/decision logic, the maneuver RNG stream, and focused
tests. Do not execute the route or emit public events.

# Result

Complete. Both children resolved:
[[TAS-115-define-contextual-wrong-way-decision-inputs-and]] landed the
reason-coded decision inputs and output (`crates/hekate-sim/src/wrong_way.rs`),
and [[TAS-116-key-the-wrong-way-draw-and-reject-impossible-opp]] landed the
versioned `maneuver` stream, the order-independent per-agent keyed draw
(`maneuver_draw`), the explicit `no_opposing_path` rejection, and the
fixed-seed/reversed-declaration-order/unrelated-agent isolation tests. The
decision reads only the observer-stage context and adds no perception model; the
runtime consumer is [[TAS-098-route-wrong-way-agents-through-ordinary-interactions]].
Validation: `cargo test -p hekate-sim --lib` (215 passed), `cargo clippy
--workspace --all-targets --all-features -- -D warnings` (clean), `cargo fmt
--all --check` (clean), `scripts/check-dependency-direction.sh` (OK), and the
coordinator's `cargo test --workspace`.

# Slices

- [[TAS-115-define-contextual-wrong-way-decision-inputs-and]] Wrong-way decision inputs and reason-coded output.
- [[TAS-116-key-the-wrong-way-draw-and-reject-impossible-opp]] Draw keying and impossible-option rejection.
