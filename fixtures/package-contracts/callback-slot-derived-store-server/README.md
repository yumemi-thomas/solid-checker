# A seedless derived `createStore` is a real, `tsc`-clean invocation

The counterexample that killed the arity premise, pinned against the typings
that make it possible. Read `callback-slot-derived-store` first — it carries the
rule; this fixture carries the one shape that fixture's stub cannot express.

`solid-js@2.0.0-rc.3` publishes two different `createStore` declarations:

| entrypoint | plain form's first parameter |
| --- | --- |
| client (`types/client/hydration.d.ts:419-425`) | `NoFn<T> \| Store<NoFn<T>>` — excludes functions |
| server (`types/server/signals.d.ts:136-143`) | `T \| Store<T>` — a function type satisfies `T extends object` |

The runtime is the same either way and does not consult arity:
`createStore(first, second, third)` returns the derived store when
`typeof first === "function"` (`@solidjs/signals` `dist/dev.js:9371`;
`solid-js@2.0.0-rc.3 dist/server.js:896` routes the same shape through
`createProjection`, and `createOptimisticStore` at `:912` delegates to
`createStore`). So `createStore(compute)` at one argument *is* the derived form
and *does* invoke `compute` — and against the server typings it compiles clean.

- `projectSeedless` is that shape. Its `tracked` callback row must survive; a
  premise that required the seed store at argument 1 withdrew it, which is the
  "true claim dropped" this fixture exists to prevent from returning.
- `projectSeeded` is the same call one argument longer, so the claim cannot be
  reading anything off the argument count.
- `plainStore` is the negative at the *same* one-argument shape: an object
  argument selects the plain form on this entrypoint too, and is never invoked.
  The pair is separated by proven callability and by nothing else.
- `derive` is the positive control for the unconditional slots.

Both derived rows publish `tracked` with **no execution point**:
`Solid2::tracked_callback_timing` deliberately states no schedule for
`createStore`, because its derived overload never accepted the probe's call
shape. Attribution is proven and is published; the schedule is not, and is left
unstated rather than guessed.

## Dialect selection

`node_modules/solid-js/package.json` pins `2.0.0-rc.3`. `projectSeedless` and
`projectSeeded` prove the selection took: 1.x has no compute form for
`createStore` at any arity, and its slot table would withdraw both.

## Stub faithfulness

`node_modules/solid-js/index.d.ts` transcribes `types/server/signals.d.ts:136-143`
byte-faithfully on the argument side — including the absence of `NoFn`, which is
the entire point. `ServerStoreOptions` and the option object's remaining fields
are reduced because nothing here reads them back, and a reduced *result* cannot
create a callback row. The fixture is clean under `tsc --noEmit --strict`; the
same source against the *client* typings is `tsc` exit 2, which is why the two
entrypoints need two fixtures.
