# 2.0's store and signal families carry a compute only in their derived form

The Solid 2.0 half of the pair whose 1.x twin is
`callback-slot-props-forwarding`; read that fixture's README first — it states
the defect and the rule. The dialects deliberately disagree here: 1.x has no
compute form for `createStore` or `createSignal` at all, so its slot table
publishes no row and every case below would be withdrawn for a different reason.
2.0 *does* have one, which is why the premise has to be finer than "the dialect
owns the slot".

`createStore`, `createOptimisticStore`, `createSignal` and `createOptimistic`
each have two forms:

- plain — the first parameter is declared to exclude functions
  (`NoFn<T> | Store<NoFn<T>>` for the store pair, `Exclude<T, Function>` for the
  signal pair);
- derived — the first parameter is the compute.

**The runtime picks between them on `typeof first === "function"` and on nothing
else** (`@solidjs/signals` `dist/dev.js:9371`;
`solid-js@2.0.0-rc.3 dist/server.js:896`, where `createOptimisticStore` at
`:912` delegates straight to `createStore`). So the premise is callability and
only callability: claim exactly when the slot's value is *proven* callable.
There is deliberately no arity premise — an earlier one dropped the true claim
that `callback-slot-derived-store-server` now pins.

- `makeStore` is `@solid-primitives/flux-store@1.0.0-next.2`'s `createFluxStore`
  reduced to its one relevant line: `createStore(initialState)` with an argument
  the declaration proves is not a function. It claimed to invoke `initialState`.
- `makeNamedStore` is the same withdrawal at the arity the derived form also
  occupies, so nothing here can be passing by argument count.
- `projectStore` is the positive: the derived form, compute proven callable. Its
  `tracked` row must survive — and it survives with **no execution point**,
  because `Solid2::tracked_callback_timing` states no schedule for `createStore`
  (its derived overload never accepted the probe's call shape). Attribution is
  proven and is published; the schedule is not, and is left unstated rather than
  guessed as `queued`.
- `makeOptimisticStore` / `projectOptimisticStore` are the same pair one
  primitive over, and the case a shared arity clause used to erase: this
  primitive's plain form takes no options argument at all, so two arguments
  already implies the derived form there. Callability decides both anyway.
- `makeSignal` and `makeDerivedSignal` are the signal pair, identical in arity
  and separated only by whether the argument is proven callable.
  `makeOptimistic` / `makeDerivedOptimistic` are its `createOptimistic` twin,
  and *are* scheduled `same-stack`: 2.0's `optimisticComputed` is `computed`
  plus one field, so the compute runs during the creating call.
- `derive` is the positive control. `createMemo`'s argument 0 is the compute in
  every 2.0 overload, unconditionally, so it needs no callability proof and keeps
  its row.

## Dialect selection

`node_modules/solid-js/package.json` pins `2.0.0-rc.3`, the prerelease this
repository audits. Two rows prove the selection took: `makeDerivedSignal` and
`projectStore` keep claims that the 1.x catalog would withdraw outright.

## Stub faithfulness

`node_modules/solid-js/index.d.ts` transcribes solid-js@2.0.0-rc.3's **client**
entrypoint (`types/client/hydration.d.ts`). `NoFn`, the `createStore` and
`createOptimisticStore` overload pairs, and the `createSignal` /
`createOptimistic` overload sets are byte-faithful on the argument side — above
all the plain forms' `NoFn<T>` and `Exclude<T, Function>`, which are what make
an object-typed argument provably not the compute. Option objects,
`Refreshable`, `NoInfer` and the hydration fields are reduced; nothing here
reads them back, a reduced result type cannot create a callback row, and
`NoInfer` changes inference direction rather than what the parameter admits. The
fixture is clean under `tsc --noEmit --strict`.

## What this does not prove

The client entrypoint is not the only published one. Its `NoFn<T>` makes a
one-argument `createStore(compute)` a type error, which is why the seedless
derived call — the shape the runtime does dispatch, and the one an arity premise
wrongly withdrew — lives in `callback-slot-derived-store-server` against the
*server* entrypoint's typings instead.

An artifact with no types at all (a JavaScript distribution with no inline
annotations and no JSDoc) answers no callability at all, so every conditional
slot here withdraws on it. That is the fail-closed direction and is recorded in
docs/precision-backlog.md as a precision residue, not a defect.
