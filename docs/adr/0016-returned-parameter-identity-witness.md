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

Simply accepting an open root is rejected: it would prove arbitrary output
shapes without evidence. Suppressing the return proposal would be sound but
discard a relationship the authenticated implementation can establish. The
new premise is preferable because it proves that relationship explicitly and
leaves generic callability unknown. No census, mandatory veto or sandbox
guarantee is weakened. Sandbox scheme 6 remains unchanged. The Type Facts
producer, Rust client and schema move together to handshake protocol 16; its
new optional return-site parameter fact has no authority when absent, and all
producer/source pins are rebuilt through the Makefile.
