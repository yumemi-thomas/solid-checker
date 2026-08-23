# A clearing wrapper is `inline`, not `deferred`

`execution: "inline"` promises the export invokes the callback **before it
returns**. `execution: "deferred"` promises the opposite. The generator used to
answer `deferred` for `untrack`, `createRoot`, `runWithOwner` and 2.0's `flush`
on the grounds that a consumer reads `deferred` as "not tracked here" — which is
true, and is not what the word says. All four run the callback during the call.

`docs/package-contracts.md` ("one word over two axes") states the vocabulary the
other way round: `inline`/`deferred` are the schedule axis and describe only
callbacks the export does not subscribe, so these primitives *stay inline* while
clearing the listener, and the clearing travels separately through the dialect's
`runs_callback_synchronously`. `interproc.rs` said so about itself, in a comment
that called the reconciliation "a contract-emission change with its own
fixtures". This is that fixture.

Nothing observable made the divergence visible until `contract probe` started
measuring timing, at which point every affected row failed. The measurement
attributed 34 failing claims to `deferred → inline`.

| Export | claim | why |
| --- | --- | --- |
| `untrackedWrapper` | `inline` | `untrack(fn)` returns `fn()`'s value; this is `solid-js/web`'s own `use` |
| `rootWrapper` | `inline` | `createRoot` runs its callback synchronously; `@solid-primitives/rootless`' `createSubRoot` |
| `ownerWrapper` | `inline` at parameter **1** | the clearing wrapper's callback slot is not always index 0 |
| `trackedWrapper` | `tracked` | negative: no clearing wrapper, so the tracked claim is untouched |
| `deferredWrapper` | `deferred` | negative: `onCleanup` really does run its callback later |

Measured against this fixture, before → after: `untrackedWrapper`,
`rootWrapper` and `ownerWrapper` moved `deferred` → `inline`; `trackedWrapper`
and `deferredWrapper` did not move. The two negatives are the whole reason the
rule is a rule rather than "answer `inline` for anything wrapped in a call".

## Stub faithfulness

`node_modules/solid-js/index.d.ts` transcribes solid-js@1.9.14's
`types/reactive/signal.d.ts`, dropping only inference machinery
(`EffectFunction`, `NoInfer`, the seed-value overloads) that no claim here
depends on. Every callback parameter keeps a callable type, `untrack` keeps
`Accessor<T>`, `onCleanup` keeps its identity return and `runWithOwner` keeps
`T | undefined`, so no claim in `expected.json` rests on a stub being looser
than the package.
