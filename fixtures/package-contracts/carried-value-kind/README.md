# Accepted dependency semantics cannot launder a false value claim

This fixture keeps the original carried-value adversary at the temporary-v2
acceptance boundary. `laundered-dependency` and
`laundered-typed-dependency` each publish a normalized main document, but the
analyzer sees either document only through the exact import rows in
`.solid-checker/accepted-contracts.json` and its proof-issued receipt.

The untyped dependency leaves its callability leaf open. The typed dependency
can establish callability without closing unrelated callback behavior. A
positive local fact therefore remains available while uncertainty stays at the
exact recursive claim leaf; neither an omitted field nor a missing receipt is
interpreted as negative proof.

The former dependency proposal, evidence-kind trust list, and generated-versus-
explicit distinction were migration machinery. They were deleted in Phase 14:
accepted closure now requires the ordinary proof and receipt boundary,
regardless of where a proposal originated.
