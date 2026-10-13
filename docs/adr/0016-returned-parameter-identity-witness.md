# 0016 — Prove returned parameter identity without a concrete generic shape

Status: accepted and implemented; measured outcomes in docs/2026-09-04-published-js-probe-unlock.md
Date: 2026-09-04

The onCleanup frontier is an operation output claiming `parameter(0)`, not a
concrete callable or object shape. Its published generic declaration leaves
the value open. The reduced native `returned-parameter-identity` fixture
reproduces the refusal: `operation value root shape has no verifiable premise:
the demand asserts no callability and the producer's root observation is open`.

Add a positive implementation fact on return sites identifying an unchanged
whole input parameter. Emit it only for a direct identifier (through transparent
wrappers), bound to an exact ordinary parameter symbol, with no assignment to
that symbol anywhere in its file. Defaults, rest/destructuring, aliases,
async/generator wrapping and bodies mentioning eval or arguments do not acquire
this fact. Absence proves nothing.

The native verifier may discharge only a whole parameter output from this
identity fact. Require a complete control-flow census, an unconditional
reachable value-return edge, and agreement of every non-unreachable return on
the exact demanded parameter. This proves the value relationship independently
of its concrete type. A mismatched or absent identity must refuse, without
falling back to a closed but unrelated return type. Other output shapes keep
their current proof requirements.

**Amendment (2026-09-05): which incompleteness "complete flow" tolerates.**
The census classifies each unmodelled construct in its `incompleteness` rows: a
`reachability-lower-bound` row is a loop, `try`, or `switch` the census walked
in full, where only the lower bound is missing — control may not enter the
body — and the producer keeps `reachable` across a construct that completes
normally while marking the sites inside `unknown`; a `flow-unaccounted` row
answers neither question. The identity fact is flow-insensitive by
construction (no assignment to the parameter's symbol anywhere in the
implementation, no `eval` or `arguments`), so no construct can change *which
value* a return carries; flow decides only whether a return is reached. The
verifier therefore accepts a census whose every incompleteness row is
`reachability-lower-bound`, still requiring one `reachable` value-return edge
with `reachable` carry and agreement of every non-`unreachable` return
(`unknown` returns inside the construct included). Any `flow-unaccounted` row,
or a marker with no classification, keeps the fact open as before. Measured
on `@solidjs/web@2.0.0-rc.3`'s `claimElement` — a `for` loop over its
handlers, then `return node` — which had refused with "returned parameter
identity needs complete flow and an unconditional return" and blocked five
corpus rows (`@solid-primitives/form` floor/head,
`@solid-primitives/intersection-observer` floor/head, `@solidjs/element`).

**Amendment (2026-09-05): a throw guard does not make the return conditional.**
The producer's census gave the statement after an `if` an `unknown` carry
strength whenever either arm could fail to complete normally, and a guard --
`if (!metadata) throw new Error(…); return fn;`, `@solidjs/web`'s `withMeta` --
is exactly such an arm. An execution that throws returns no value, so the
guard contributes no competing value-return edge and every normal completion
still passes the one return. The census now keeps the entry carry strength
across an `if` whose escaping arm leaves by `throw` alone (no `return` of the
enclosing function inside it, nested callables' returns excluded) when the
other arm is absent or always completes normally; an arm that may `return`
keeps the successor's carry `unknown` as before. Sites inside either arm stay
conditional. The `reach` row is untouched.

Simply accepting an open root is rejected: it would prove arbitrary output
shapes without evidence. Suppressing the return proposal would be sound but
discard a relationship the authenticated implementation can establish. The
new premise is preferable because it proves that relationship explicitly and
leaves generic callability unknown. No census, mandatory veto or sandbox
guarantee is weakened. Sandbox scheme 6 remains unchanged. The Type Facts
producer, Rust client and schema move together to handshake protocol 16; its
new optional return-site parameter fact has no authority when absent, and all
producer/source pins are rebuilt through the Makefile.
