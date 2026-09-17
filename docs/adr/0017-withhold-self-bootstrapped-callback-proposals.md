# 0017 — Withhold callback proposals from the primitive-defining package

Status: accepted and implemented; measured outcomes in docs/2026-09-04-published-js-probe-unlock.md
Date: 2026-09-04

After ADR 0016, the real offline Kobalte graph refuses solid-js 1.9.14's
onMount, demand
`sha256:09373314370a8a0dec0ad51e653394a54ac5b5abf20bd35715b76dd1df5ea7f3`:
`callback parameter has no exact direct-call or resolved-argument flow`.
Its implementation forwards fn through local createEffect and untrack. The
generator recognizes those names through its path bootstrap; the verifier has
no independent positive premise for the complete execution chain.

Extend ADR 0005's generation scope to callbacks. For the exact set of
primitive-defining package names, emit callback knowledge as unknown and omit
the corresponding operations and closure proposals. This is deliberately
coarse: current summaries do not distinguish independently inferred callbacks
from bootstrap-derived callbacks, including transitive derivations. Direct
callbacks in these packages also remain undescribed. Ordinary consuming
packages retain their callback proposals and all native proof requirements.

The package-name predicate can only remove claims, never establish identity,
callback absence or execution. Consumers distinguish the result by an explicit
unknown callback domain; acceptance of the remaining document does not certify
that domain. No scheduled creates veto is waived. Sandbox scheme 6, runtime
bytes, census and all native proof gates remain unchanged.

Retaining today's proposal merely repeats a known unsupported assertion.
Adding a positive self-axiom would introduce ADR 0005's unresolved circular
trust. Precise derivation provenance could retain independent direct callbacks,
but must survive every summary composition; guessing from the final callback's
timing or export name cannot do that. Withholding the whole affected domain is
the defensible immediate disposition. Recovering individual callbacks requires
independent implementation evidence or explicit derivation provenance.

Pin both scopes using the same callback summary, and measure the real graph
and full corpus before recording any acceptance or snapshot moves.
