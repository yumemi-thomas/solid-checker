# implementation-census-argument-returns

The tracer for ADR 0115 and ADR 0116: a `returns` closure over returns that
each hand back the caller's own argument (`parameter`), a fresh array of the
caller's arguments (`argument-array`), or what an invocation of the caller's
argument returned (`invocation-result`), decided from the producer's arms of
every return (handshake protocols 61 and 62).

It is two fixtures in one. As a generator-corpus fixture (`corpus.json`) its
`expected.json` pins what the argument-container walk proposes from syntax.
As a certification tracer
(`the_argument_container_census_certifies_exactly_the_enumerated_containers` in
`contract_certification.rs`) it is planned with each export's containers set by
hand, including three the walk would not propose, and certified against the
real producer with ADR 0115's synthesized veto.

| export | walk proposes | tracer claims | verdict |
| --- | --- | --- | --- |
| `asArray` | `parameter 0`, `[]`, `[0]` | the same | certified |
| `pick` | `parameter 1`, `parameter 2` | the same | certified |
| `pairOrValue` | `parameter 0`, `[0, 1]` | the same | certified |
| `reassigned` | `parameter 0`, `[0]` | the same | refused: the parameter is written, so no arm is the caller's value |
| `withLiteral` | nothing (a plain return) | `parameter 0`, `[0]` | refused: `[value, 1]` holds an element that is no argument |
| `overclaimed` | `parameter 0`, `[0]` | the same and `[]` | refused: no completion hands back `[]` |
| `viaCall` | nothing (a plain return) | `parameter 0`, `[0]` | refused: an arm is a call's result |
| `accessWith` | `parameter 0`, `invocation 0` | the same | certified |
| `access` | `parameter 0`, `invocation 0` | the same | certified |
| `run` | `invocation 0` | the same | certified |
| `wrap` | `[0]` | the same | certified |
| `parenthesized` | `parameter 0`, `[0]` | the same | certified |
| `optionalCall` | `parameter 1`, `invocation 0` | the same | refused: an optional call hands back `undefined` for a nullish callee |

Each refusal withdraws the `return` operations by name, because each one's
positive fact reads the same evidence the closure census does, and the row
still certifies.

`access` and `accessWith` are declared exactly as
`@solid-primitives/utils@7.0.0-next.4` declares them. `access`'s `callbacks`
would refuse on its `!v.length` read (ADR 0100 rule 2); the tracer does not
propose it, and its `returns` certifies.

The walk reads the Oxc facts: a returned conditional's branches
(`ConditionalExpressionFact`'s array-literal branches, facts schema 43), a
returned array literal's elements, and a returned call's callee, each resolved
to the function's own whole parameter by symbol. A parenthesized conditional is
the conditional itself, because the facts are parsed without parentheses. What
it does not read: a logical `||`/`&&`/`??` and a returned `[]` on its own.
