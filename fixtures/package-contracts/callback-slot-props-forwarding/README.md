# A primitive callback slot is not, by itself, a callback

`primitive_callback_execution` answers *how* a callback at some primitive
argument would run relative to the exported call. The contract inventory read a
row there as permission to publish an `invoke` claim rooted at whatever
parameter was forwarded into that slot — and the row says nothing about whether
there is a callback in it. Two shapes in the measured ecosystem published
`invoke` claims about values the shipped code never invokes. A demand like that
is not merely imprecise: the certification census refuses it, correctly, and no
evidence can ever discharge it.

This fixture is the Solid 1.x half. Its Solid 2.0 twins are
`callback-slot-derived-store` (the client entrypoint's typings) and
`callback-slot-derived-store-server` (the server entrypoint's), and the set is a
deliberate dialect pair: `1.x` and `2.0` disagree about whether `createStore` and
`createSignal` have a compute form at all, so the same source text earns a
different answer in each.

## `mergeProps` — every argument, one schedule, no premise

The withdrawn row was `(P::MergeProps, _)`: every argument index, always
`tracked`. Its own comment named the missing fact — "JavaScript distributions do
not retain the declaration type that proves an ordinary props object is
non-callable, so preserve the primitive's conservative callable semantics" — and
a positive `invoke` claim is not the conservative reading of an absent
callability answer. Solid 1 memoizes a merge source *if* that source is a
function and copies it otherwise, so the premise the row needed is exactly the
one it did not have.

- `Stylesheet` is `@solidjs/meta@0.29.4`'s export, reduced:
  `props => mergeProps({rel: "stylesheet"}, props)`, declared over a props
  object. It claimed to invoke `props`. `@solidjs/router@1.0.0`'s `A` and
  `Route` are the same shape, and so is every Solid component that forwards its
  props through `mergeProps`.
- `WithDefaults` is the same claim at merge position 0, because the row matched
  every index.
- `WithLazyExtras` is the positive that keeps this a grounding rule rather than a
  blanket exemption: a merge source the declaration proves callable really is
  memoized and invoked, and keeps its `tracked` row.
- `WithOpaqueExtras` is the second answer the premise accepts. `Function` is the
  signature-less function supertype: the compiler proves it callable and leaves
  no signature, arity or parameter type to read (`Callability::UntypedCallable`).
  That is a *positive* proof, not the absence of one, so it roots the claim
  exactly as a read signature does.

Both positives publish `tracked` with `schedule: same-stack`, not `queued`:
`Solid1x::tracked_callback_timing` states `MergeProps => DuringCall`
(`solid-js@1.9.14 dist/solid.js:1329` wraps each function source in a memo, and
`:244-256` runs a memo's compute before `createMemo` returns), so the callback
has already run when the export returns. `tracked` is an attribution word and
carries no schedule; reading `queued` out of it was a promise the runtime breaks.

## `createStore` / `createSignal` — 1.x has no compute form

`(CreateStore, 0)` and `(CreateSignal, 0)` are 2.0's names and 2.0's positions.
1.x has both functions, but as `createStore(store?, options?)` and
`createSignal(value, options?)`: no compute form exists, and the 1.x dialect's
own `callback_executions` table accordingly publishes no row for either
(`createSignal(() => value)` in 1.x *stores* the function as the signal's value
— `Dialect::stores_function_argument_as_value`). Applying 2.0's vocabulary to a
1.x artifact is what made `@solid-primitives/flux-store@0.1.1`'s
`createFluxStore(initialState, createMethods)` claim to invoke `initialState`,
whose only use is `createStore(initialState)`.

- `makeStore` and `makeNamedStore` are that shape at both 1.x arities.
- `makeSignal` isolates the dialect premise from the callability one: its
  parameter is *provably callable* and the row is still withdrawn, because 1.x
  does not invoke it.
- `derive` is the positive control. `createMemo`'s argument 0 is the compute in
  every 1.x overload, unconditionally, so it needs no callability proof and its
  row must survive — otherwise the withdrawal has taken the whole branch with it.

## Stub faithfulness

`node_modules/solid-js/index.d.ts` and `store.d.ts` transcribe
solid-js@1.9.14. Every **argument** signature the proof depends on is
byte-faithful, including `mergeProps<T extends unknown[]>(...sources: T)` and
`createStore`'s conditional tuple: a props object at a merge source is exactly
as admissible here as in a real project, so nothing in this fixture's negatives
comes from a stub that refuses what the real package accepts. Only *return*
types are reduced (`MergeProps<T>` → `unknown`, `StoreReturn<T>` → the tuple),
which nothing here reads back and which cannot create a callback row. The
fixture is clean under `tsc --noEmit`.

## What this does not prove

Withdrawing `createFluxStore`'s row leaves its callback domain **open**, not
closed-empty, so nothing false is asserted in the other direction. But its real
callbacks — `createMethods.getters` and `createMethods.actions`, argument 1
behind a property path — are still not inventoried, because the generator's
`ContractCallback` carries a parameter index and no path. Recorded in
docs/precision-backlog.md.
