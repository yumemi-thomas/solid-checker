# A clearing wrapper is `inline`, not `deferred`

`execution: "inline"` promises the export invokes the callback **before it
returns**. `execution: "deferred"` promises the opposite. The generator used to
answer `deferred` for `untrack`, `createRoot`, `runWithOwner` and `flush` on the
grounds that a consumer reads `deferred` as "not tracked here" — which is true,
and is not what the word says. All four run the callback during the call.

`docs/package-contracts.md` ("one word over two axes") states the vocabulary the
other way round: `inline`/`deferred` are the schedule axis and describe only
callbacks the export does not subscribe, so these primitives *stay inline* while
clearing the listener, and the clearing travels separately through the dialect's
`runs_callback_synchronously`. `interproc.rs` said so about itself, in a comment
that called the reconciliation "a contract-emission change with its own
fixtures". This is that fixture.

Nothing observable made the divergence visible until `contract probe` started
measuring timing, at which point every affected row failed.

| Export | claim | why |
| --- | --- | --- |
| `untrackedWrapper` | `inline` | `untrack(fn)` clears the listener, calls `fn`, restores, and returns its value |
| `rootWrapper` | `inline` | `createRoot` runs its callback synchronously under a fresh owner; `@solid-primitives/rootless`' `createSubRoot` |
| `ownerWrapper` | `inline` at parameter **1** | the clearing wrapper's callback slot is not always index 0 |
| `trackedWrapper` | `tracked` | negative: no clearing wrapper, so the tracked claim is untouched |
| `deferredWrapper` | `deferred` | negative: `onCleanup` really does run its callback later |

The two negatives are the whole reason the rule is a rule rather than "answer
`inline` for anything wrapped in a call".

## The 1.x original, and what changed

This fixture replaces the Solid 1.x one deleted by ADR 0110. Four of the five
exports transcribe unchanged — `untrack`, `createRoot`, `runWithOwner` and
`onCleanup` all survive 2.0 with the parameter order each claim depends on, and
`runWithOwner`'s callback is still at index 1, which is the point of
`ownerWrapper`.

`trackedWrapper` is the one that moved. 2.0 splits `createEffect` into a tracked
compute and an untracked effect function, so the 1.x spelling
`createEffect(() => handle())` is now only a deprecated overload returning
`never`. **That overload is not a type error in statement position** — checked
against the published typings with `tsc --noEmit --strict`, which is silent on
it, because discarding a `never` is legal and only a *use* of the result would
fail. So the two-argument form is used here because it is the supported one, not
because `tsc` rejects the alternative. `handle` stays in the compute, which is
the tracked, deferred position the 1.x claim was about.

## Stub faithfulness

`node_modules/solid-js/index.d.ts` transcribes the five declarations from
`@solidjs/signals@2.0.0-rc.3` that `solid-js`' own `types/index.d.ts:1`
re-exports, each cited by file and line in the stub's header. Nothing on the
argument side is reduced: `untrack` keeps its second parameter, `createRoot`
keeps the union admitting a `dispose`-taking callback, `runWithOwner` keeps the
callback at index 1, `onCleanup` keeps `Disposable` on both sides, and
`createEffect` keeps **both** overloads so the deprecated arm resolves here
exactly as it does against the package. Only option-object fields, `Owner`'s
members, and the inference machinery around `createEffect`'s result are reduced,
and a reduced result cannot create a callback row.

`index.ts` type-checks clean under `--strict` against the real
`solid-js@2.0.0-rc.3` install as well as against this stub, so no claim in
`expected.json` rests on the stub being looser than the package.

## What this fixture does not close, and why that is correct

`expected-refusals.json` carries one withheld claim and two declined closures.
None is a defect of the fixture; each is a modelled limit this fixture now pins
beside the claims above.

- **`trackedWrapper`'s owner requirement is withheld.** It must be called under
  an owner, because it registers a computation on one — and schema version 1
  has no operation kind that can carry a free-standing owner requirement. The
  `invoke` operation is published; the requirement is not.
- **`ownerWrapper` declines `creates` for `runWithOwner`, and
  `trackedWrapper` declines it for `createEffect`** — both `dialect-silent`
  against `solid-js`. The dialect's negative authority has no `creates` row for
  either: `runWithOwner` is outside the ten exports of `solid-js`'
  audited document, and `createEffect`'s row was **withdrawn on 2026-09-04**
  because `dist/server.js:868-870` routes it to `serverEffect`, which can reach
  `ctx.serialize` — a create, under the `node`/`worker`/`deno` condition only.
  A `(package, export, domain)` row carries no condition, so the row is
  withheld rather than qualified (`solid_2.rs`, and ADR 0007's open item).

  Silence there is the *stronger* answer: the domain stays open rather than
  being closed on a claim the audit will not make. `untrack`, `createRoot` and
  `onCleanup` do carry rows, which is why only these two decline — so this
  fixture also pins that the difference between them is the table, not the
  wrapper shape.
