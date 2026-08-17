# solid2-precision

Solid 2.0 precision regressions for exact callback execution, cleanup-return
value domains, assignment-target reads, and owner-backed `onSettled`.

Each claim is pinned from both sides:

- **Synchronous callback proof.** The literal callback of the built-in
  `Array.prototype.filter` after a dominating await carries both the accessor
  call and the store member read. A `Promise#then` callback, a project-defined
  `filter` with the same spelling, an unresolved helper's callback, and a
  callback built by a wrapper call (`filter(makePredicate(fn))`) all stay
  silent, because none of them proves the function runs before the await
  resumes.
- **Cleanup returns.** Contextual, explicit, parenthesized, and `as`-cast
  primitive returns are proven SC3004. `unknown`, `any`, and an unconstrained
  generic stay SC9002. A provably `undefined` identifier return is legal *and*
  is not a returned cleanup, so it produces nothing at all — including no
  SC4004 on the unowned callback that contains it.
- **Member cleanup returns.** A static member expression is classified from
  its complete-expression value domain: `teardown.dispose` is a cleanup and
  `teardown.count` is SC3004. Optional/union, `any`, and computed members stay
  SC9002 because the returned value is not proven closed at that span.
- **Returned calls are classified from the result, not the callee.** Every
  callee in `ReturnedCallCleanupReturns` is itself callable, so a callee-shaped
  fact certifies all of them; the call-result domain separates them. A produced
  `number` is SC3004 in both return spellings (block-bodied and expression-
  bodied), a produced function is a cleanup, `(() => void) | undefined` and
  `void` are legal without being cleanups, and `any` stays an SC9002
  obligation. The unowned `onSettled(() => { return makeCount(); })` pins the
  other half: reading the callee reported a false SC4004 there, while
  `onSettled(() => makeThunk())` must still report one.
- **Assignment targets.** A plain `store.name = next` is a write only; the
  compound and update forms also read; and a computed key or destructuring
  default inside a target stays a read.
- **Owner-backed `onSettled`.** An inline owned callback reports SC3001 without
  a duplicate SC4002. A wrapper-built callback, an out-of-band call, and an
  unowned returned cleanup keep SC4002/SC4004.
- **Leaf scope needs a literal callback and its own synchronous extent.** The
  leaf rules fire only for a call written directly in a function literal in the
  owner's callback argument. A wrapper-built callback
  (`onSettled(wrap(() => …))`), a callback handed over as an identifier
  reference (`onSettled(settledCleanup)`), and a call in a nested function the
  literal callback only builds are all silent for SC3001 — the first two still
  report the genuinely unowned SC4002.
