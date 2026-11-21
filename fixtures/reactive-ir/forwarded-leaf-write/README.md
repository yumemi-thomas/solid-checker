# A write forwarded to a leaf owner's callback (ADR 1155 (a))

The Opus review of ADR 1147 (F1's root, Chrome-confirmed on rc.21, pattern `ctxmemo/s11-hand-direct`): `hand(() => setN(1))`, with `hand` a package read as source
that calls its parameter in `createTrackedEffect`, was SC2001 a violation: ADR 0200's forwarding (`callee_roles::forwarded_compute_slot`) gave the literal the
tracked role, and the write guard's leaf exemption read only a leaf's own literal. rc.21 exempts any write that runs in the leaf callback. With ADR 0200's exact
proof reaching a `CallbackOwner::Leaf` slot, the write takes the leaf scope (`write_roles::forwarded_leaf_scope`).

- Positive (clean): `HandApp` (the forwarding to `createTrackedEffect`); `LengthApp` (r2: a non-arrow wrapper that reads only `arguments.length` runs
  nothing through it).
- Negative: `MemoApp` (forwarded to a memo's compute: the violation stays; Chrome reports REACTIVE_WRITE_IN_OWNED_SCOPE), `TwiceApp` (the parameter also handed to
  a timer: no exact proof, the row it had stays), `ArgsApp` (the review of 1155, F1: a non-arrow wrapper that also calls `arguments[0]()`, in the memo
  that calls it; Chrome reports REACTIVE_WRITE_IN_OWNED_SCOPE, so a write whose leaf slot also has an `arguments[k]()` run is unplaced, r2), `SwapApp` (the parameter
  reassigned: refused).

The forwarding functions are the project's: a package read as source takes the same proof (`callee_roles::forwarded_compute_slot` resolves the callee to its
function either way), held against the real install by patterns `ctxmemo/s11-hand-direct` and the generator's `barm-fwdleaf-tracked`/`-memo`. Stub:
`provider-children-memo-owner`'s.
