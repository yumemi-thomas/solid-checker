# ADR 0159: A strict reachability floor is a lower bound

- Status: accepted and implemented (2026-09-29); written with the
  implementation.
- Date: 2026-09-29
- Owners: the certifier's positive facts (`type_facts.rs`
  `ReachabilityFloor::admits_call`, `ReachabilityFloor::admits_unbounded_site`)
- Relation: uses the producer's `ImplementationCall::unconditional`, which ADR
  0152 added at handshake protocol 71. No protocol change.

## Context

`ReachabilityFloor` decides which sites may witness an operation. An
operation whose count states `min: 0` gets the may-execute floor, and a call
with `reach` `reachable` or `unknown` witnesses it. Every other operation --
one with `min >= 1`, and one whose count is unstated -- gets the strict floor,
documented as "only a call the implementation provably reaches counts", and
it admitted exactly `reach: reachable`.

That `reach` is optimistic. The producer keeps both arms of an `if` at the
`if`'s own reachability unless a literal condition decides it, and passes
`reach` straight through `&&`, `||`, `??`, `?:` and optional chains; only
loops, `switch` and `try` lower it to `unknown`, and only code after a
`return` or `throw` is `unreachable`. So `if (flag) callback()` is
`reachable`, and the strict floor accepted it as a lower bound it is not.

## What that let through, measured

The suspicion was that a claim "the callback runs at least once" could be
certified for a conditional call. It cannot, for a reason outside the floor:
the operation-cardinality census proves a per-call count of `0..many` and
refuses every tighter one ("runtime implementation census cannot prove a
tighter operation cardinality", and "Type Facts has no exact arbitrary runtime
loop/reentry bound" for the invocation census). Every `min >= 1` operation is
therefore withheld whatever its witnesses, and no certified document states
one: the generator writes `min: 0` everywhere, and the only `min: 1`
operations in the repository are the audited solid-v2 bundles, which no census
certifies.

What the optimistic floor did affect is the other half of its population: an
operation whose count is **unstated**. It is asked for `operation-reachability`
under the strict floor and no `operation-cardinality` at all, so its witnesses
are the whole of its evidence. `implementation-census-callback-lower-bound`
pins it: a `callbacks` item with no count certified for `guarded`
(`if (flag) callback()`), `shortCircuit` (`flag && callback()`), `chosen`
(`flag ? callback() : undefined`) and `early` (`if (flag) return;
callback()`), and refused only `looped`. An unstated count claims nothing about
how often (`CardinalityMinimum` stays open, and the consumer's
`Cardinality::strength` reads it as possible), so nothing false reached a
consumer; the evidence was simply weaker than the floor's own definition says.
No generated or accepted document carries an unstated count today.

## Decision

- **A call witnesses under the strict floor only when the producer states it
  `unconditional`** (`admits_call`): it runs exactly once on every normal
  completion of its flow owner. A captured call's owner is its enclosing
  callable, which the execution chain already proves separately, so the bound
  composes link by link. The may-execute floor is unchanged.
- **A parameter use or an uncensused invoking form never witnesses under the
  strict floor** (`admits_unbounded_site`): the producer states no lower bound
  for either, and its `reach` is the same optimistic one. A direct call of a
  parameter is still witnessed through its call.
- **A universal claim over uses ranges over every use that may run**, whatever
  the demand's floor. `require_reactive_operation_input` asks that every use of
  the callback parameter be the callee of a proved call; under the strict
  floor it skipped uses whose `reach` was `unknown`, so a use inside a loop
  body went unchecked. It now reads the may-execute set always, which is at
  least as strict under either floor.

Left as they were, deliberately: the return-site `carryReach` checks (already
a lower bound), the carried-callable `reach` of a returned conditional (the
producer states each alternative `unknown`), and the identity witnesses that
read `reach == reachable` to ask whether a value is the caller's rather than
how often something happens.

## Consequences

- `a_callback_claimed_to_run_at_least_once_needs_an_unconditional_call` pins
  all three counts on the tracer: `min: 0` certifies all seven asserted
  exports, `min: 1` none (the cardinality cap), and an unstated count only
  `always` and `afterThrow` (a `throw` before the call leaves every normal
  completion running it). `the_strict_floor_asks_a_call_for_its_lower_bound`
  pins the two predicates.
- Measured: no findings snapshot and no contract-corpus snapshot moved (114
  fixtures, the new one added); `make primitives-checkpoint` before and after
  at the same base shows no export move.
- If a future census proves a count tighter than `0..many`, the strict floor
  is now a lower bound it can rest on; before this it would have certified
  `min: 1` for a call under an `if`.
