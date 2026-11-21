# A write forwarded to a leaf owner's callback (ADR 1155 (a))

The Opus review of ADR 1147 (F1's root, Chrome-confirmed on rc.21, pattern `ctxmemo/s11-hand-direct`): `hand(() => setN(1))`, with `hand` a package read as source
that calls its parameter in `createTrackedEffect`, was SC2001 a violation: ADR 0200's forwarding (`callee_roles::forwarded_compute_slot`) gave the literal the
tracked role, and the write guard's leaf exemption read only a leaf's own literal. rc.21 exempts any write that runs in the leaf callback. With ADR 0200's exact
proof reaching a `CallbackOwner::Leaf` slot, the write takes the leaf scope (`write_roles::forwarded_leaf_scope`).

- Positive (clean): `HandApp` (the package), `OwnHandApp` (the same forwarding in the project).
- Negative: `MemoApp` (forwarded to a memo's compute: the violation stays; Chrome reports REACTIVE_WRITE_IN_OWNED_SCOPE), `TwiceApp` (the parameter also handed to
  a timer: no exact proof, the row it had stays).

Package `r1155-hand` is this fixture's own (dependency source, `.solid-checker/runtime.json`); `node_modules/solid-js` and `@solidjs/*` are
`dependency-source-default-include`'s manifests. Stub: `provider-children-memo-owner`'s.
