# ADR 0029: Close finite creates-census graphs and classify loose null equality

- Status: accepted
- Date: 2026-09-05
- Owners: Type Facts producer and policy-2 certification verifier

## Context

The restricted TypeScript execution profile of ADR 0028 made the runtime veto
available to import-free source artifacts. Twelve of the 26 finite POC samples
still failed before probing because the independent native `creates` census
could not finish. Five apparent missing-transcript cases reduced to one
cross-parser range mismatch: Oxc starts an exported function declaration at
`function`, while typescript-go starts it at `export`. After binding those
ranges through the exact direct-export fact, three candidates completed and two
advanced to a pre-existing coercion refusal.

Two remaining refusal classes require proof decisions rather than plumbing:

1. `getActiveElement` has an exact local call graph that re-enters its own
   exported declaration through iframe traversal. The current visited set
   refuses every cycle.
2. Kobalte's platform helpers use `window.navigator != null`. The producer
   classifies every loose equality as coercing, although ECMAScript loose
   equality against `null` does not apply `ToPrimitive` to the other operand.

A finite probe is still only a contradiction veto. Neither decision may use a
completed sample as the premise that closes `creates: []`.

## Decision

### Exported local-declaration spans

The verifier derives the typescript-go demand range from two independently
parsed facts over the authenticated runtime bytes:

- the Oxc function node, selected by the exact resolved declaration-name span;
- the unique direct export whose declared local name is that function name and
  whose end is the function node's end.

For that inline exported declaration the demand begins at the export wrapper.
For every other function it remains the Oxc function span. Source-text scanning
for the word `export`, containment-only selection, and name-only matching are
not authority. The producer continues to require an exact function-like node
and independently binds its resolved declaration inside the demanded range.

### Finite recursive call graphs

The negative `creates` census treats a call to an exact local declaration
already on the current census stack as `local-recursion-backedge`. It may do so
only after re-establishing all existing local-recursion premises: authenticated
runtime source, exact symbol/file/span identity, an exact function node, and a
binding that has no write or competing declaration.

This is the finite graph fixed point for a zero upper bound. Every frame passes
the complete transcript and uncensused-form checks before its calls are walked;
every edge that is not a back-edge keeps its normal disposition; and iteration
continues after the recursive child returns, so a sibling unresolved or
creating edge still refuses. Re-entering a frame can repeat already-censused
bodies or diverge, but cannot introduce an operation outside that finite graph.
The depth limit continues to count distinct declarations and refuses an acyclic
chain beyond the bound.

The mandatory veto remains mandatory. A cycle fixture therefore uses a boolean
stop argument so its sampled execution finishes; an infinite sample would time
out and withhold certification even though the static census closed.

### Loose equality with an exact null literal

For `==` and `!=`, an exact `null` literal on either side clears the coercion
marker. The ECMAScript comparison algorithm handles null/undefined (including
the browser's HTMLDDA special case) without calling `Symbol.toPrimitive`,
`valueOf`, or `toString` on the other operand. Children are still walked, so an
accessor or call used to produce that operand keeps its own row.

All other loose equalities retain the ordinary per-operand coercion check. In
particular, an identifier spelled `undefined` is not a syntax-level substitute:
it can be shadowed, and no name heuristic is introduced.

Because an empty `uncensusedInvokingForms` list gains this new meaning, the Type
Facts handshake moves from protocol 16 to 17. The table schema does not change;
the wire shape is unchanged and the handshake is the discriminator for the
stronger producer semantics. The Type Facts source-manifest pin and checker
compile-time pin move with the implementation.

## Alternatives considered

Continue refusing cycles. This is sound and simplest, but rejects a finite,
fully enumerated graph solely because depth-first traversal encounters an edge
twice. It leaves `getActiveElement` blocked despite having all premises needed
for a zero-upper-bound fixed point.

Unroll recursion to the depth limit. Refusing at the cutoff is equivalent to
today's outcome. Accepting at the cutoff would infer an unbounded negative fact
from a bounded prefix and is unsound.

Use the runtime sample to show termination or absence. A sample covers only its
inputs and cannot close a universal operation domain. It remains a veto after
the census has decided the claim.

Treat all loose equality as non-coercing, or clear it from static operand types.
Loose comparison between object and string/number/bigint can execute user
coercion hooks. The exact null-literal case is narrower and follows the runtime
algorithm directly.

## Consequences and remaining refusals

Certificates keep their existing published-byte or explicitly named derived-
byte profile meaning. This ADR changes the independent native premise, not what
the runtime veto observes. Receipt witnesses distinguish ordinary local
recursion from `local-recursion-backedge`.

`CallableFunction.call` remains refused because it transfers control through
its receiver and `Object.prototype.toString.call(value)` can also observe a
user-defined `Symbol.toStringTag` getter. Computed element access remains
refused where the compiler cannot bind an exact data property; an array type
does not exclude a Proxy. Imported TypeScript, browser-dependent execution, and
general coercion remain separate profiles or census questions. No result in
this ADR authorizes fake browser globals or suffix-guessing module resolution.
