# Review: moving the audited Solid 2 triple from `2.0.0-rc.17` to `2.0.0-rc.21`

Date: 2026-11-04. Owner request: follow the latest release candidate. `solid-js`,
`@solidjs/signals` and `@solidjs/web` `2.0.0-rc.21` are `next` on npm, git head
`2105ffb8ca2fced55f5fac0763177fc93f97c2c2`, the tip of `solidjs/solid` `next` that
day. rc.18, rc.19 and rc.20 were published and not read.

Integrities (each equal to the sha512 of the registry tarball):

- `solid-js` `sha512-0Z8/qiJKnK6A2g8QspGthwxR0W8WauMOD2/LQtNmxBT8gpZkKaXmo/fdek/+cB+84w4HYejWJixSAn4fcAI0Uw==`
- `@solidjs/signals` `sha512-F1TUMO/nd1XC//+tcaWxibOOgXBHQGmNQnAeiDnpLh+1smnlwIgKF6pc20evlyfQwqf0gP6rH4sNEqCPBNcpgg==`
- `@solidjs/web` `sha512-1E9dvR3/+5fQTI3oxxACBX2Rg4rezXgrljerhWDHR5uQZ04p5Z8qXROI7dIDJKyANtsnLKYbM2OyI/p/FmjGUA==`

Method, in three layers:

1. Every diagnostic raise site of the dev builds compared by its enclosing code:
   15 of them differ. Every `dist/*.js:N` citation in the dialect, the IR and the
   rule pages mapped to rc.21 by its text: 233 identical, 16 changed, all read.
2. 135 probes, one Node process each, with a try and an after-marker, on the
   rc.17 and rc.21 dev bytes: 24 differ, all listed below.
3. The fuzzer's grids on rc.21 against rc.17: the outcome counts of `mx-census`,
   the all-pairs sample and the per-code tier are equal but for the rows the
   changes below explain, and Chrome agrees with every oracle sample.

## Changes that touch a checker premise

| change | measured | what moved |
| --- | --- | --- |
| `untrack` keeps `strictPending`: an unlabelled `untrack` in an open strict window throws `PENDING_ASYNC_UNTRACKED_READ` for a pending node the same owner made (#4630) | a memo, derived store, projection or hook-made memo of the body, nested `untrack` too; rc.17: `NotReadyError` | SC5001 (ADR 0619) |
| A strict-read window and the effect-callback window are tied to the event that was current (`strictReadActive`, `effectCallbackEvent`, #4237): a DOM event dispatched from inside starts outside them | read; Node has no `window.event`, so the probes do not show it | no proof moves: a native dispatch already withholds handler proofs (ADR 0356) |
| `createOptimistic(fn)` and `createOptimisticStore(fn, seed)` setters throw `OPTIMISTIC_WRITE_BEFORE_LOAD` before the first answer | rc.17: the write stood | unclaimed: open work |
| `CAUGHT_REACTIVE_READ`: a compute swallowed a read's throw and completed | memo, effect compute, an error, a leaf's catch; a rethrow or `finally` is silent | unclaimed: open work |
| `onFlush(callback)`, new | a microtask, no owner, every path; warns `ON_FLUSH_IN_OWNED_SCOPE` under a children-capable owner | modelled (ADR 0622); SC3002 (ADR 0625) |
| `from(source)`, new at the root | runs only in the pass of a computation that returns it, tracked and owned | modelled with a conditional owner (ADR 0622) |
| `LOADING_ON_ACCESSOR`, new | read and measured through the grid | SC5007 (ADR 0624) |
| `ASYNC_MEMO_ELEMENT`, new | SSR only | out of scope |
| `createOwner` is declared only in `solid-js/internal`; it is still exported from the root at runtime | TS2305 at the root | export table, fuzzer imports |
| A store proxy keeps the source's prototype (#4670): `Object.setPrototypeOf(store, …)` now throws | rc.17: silent | no rule |
| A lone dynamic SSR spread defers behind a thunk (#4251) | compiler output | trace records `ssr-render-callback` (ADR 0196) |

## Decision and changes (ADR 0194)

rc.21 becomes the audited triple with rc.17's answers, plus `Solid2::RC21`: the
`TripleExports` digit answers `onFlush`, `from` and
`Dialect::untrack_keeps_own_pending_throw` for a whole rc.21 triple; a mixed
install keeps them in a scoped gap or silent. rc.17 becomes an older read release.
Runtime-configuration vetoes gain the rc.21 declaration spans. The feedback hook
gains the rc.21 reader profile. `setLinkClaim`'s gap is re-read and stays.

Recorded, not modelled: `@solidjs/router` stays audited at `2.0.0-next.41`;
`2.0.0-next.43` needs its own audit before its digest is admitted.

## Not verified

- The event-scoped windows in a browser: read, not probed.
- Observe builds were not probed; prod has no guards.
