# write-scope-optimistic-store-rc0

**Claim.** On an installation whose `@solidjs/signals` is `2.0.0-rc.0`, a
`createOptimisticStore` setter in a memo or effect compute is legal, so
`SC2001` (`reactive-write-in-owned-scope`) does not report it; a `createStore`
setter and a signal setter there are reported as on every release. This is N5
of the release table (`releases::OptimisticStoreSetterGuard`,
`Dialect::optimistic_store_setter_guarded`).

rc.0's `createOptimisticStore` returns `fn => storeSetter(wrappedStore, fn)`
over a store whose nodes take the optimistic engine's write path
(`@solidjs/signals@2.0.0-rc.0` `dist/dev.js:7181-7213`), which never reaches
`setSignal`'s guard, and rc.0 has no setter-entry guard at all; rc.1 adds
`devGuardStoreSetterWrite` to every store setter's entry (`dist/dev.js:3264`,
`:6619-6620`). Probed on every published rc.0-rc.9 dev client build:

| case | rc.0 | rc.1-rc.9 |
| --- | --- | --- |
| `createOptimisticStore` setter (plain or derived) in a memo compute | legal | throws `REACTIVE_WRITE_IN_OWNED_SCOPE` |
| `createOptimisticStore` setter in an effect compute | legal | throws |
| `createStore` setter in a memo compute | throws | throws |

Prod builds carry no guard on any release.

A write carries its setter's provenance only when the setter is bound from the
primitive's own tuple. A store setter the checker knows only by its type (an
alias binding) could be either kind, so on rc.0 it is not reported: for the
optimistic alias that is a correct silence, for the `createStore` alias a miss,
never a violation the runtime does not raise.

Expected findings (`computes.ts`):

- `setState(...)` and `setCount(5)` in memo computes: **SC2001**, violation.
- `setOptimistic` and `setDerivedOptimistic` in a memo compute, `setOptimistic`
  in an effect compute, and both aliases: **none**.

The twin is `write-scope-optimistic-store-rc3`: the same `computes.ts` byte for
byte over rc.3, where all seven writes are reported.

**Stubs.** Generated from the published rc.0 tarballs by the same procedure as
`write-scope-roots-rc3`'s: `node_modules/@solidjs/signals/index.d.ts` copies
every declaration the cases use byte for byte from
`@solidjs/signals@2.0.0-rc.0` `dist/types/`, dropping only the JSDoc block
before each (rc.0 declares `createStore` in `store/store.d.ts` and returns its
`StoreReturn<T>`, both copied); `node_modules/solid-js/index.d.ts` re-exports
them as `types/index.d.ts:1` does and copies `createOptimistic`,
`createOptimisticStore` and their local types byte for byte from
`types/client/hydration.d.ts`; `node_modules/solid-js/types.d.ts` is
`types/types.d.ts` whole; `node_modules/@solidjs/web/jsx-runtime.d.ts` is lines
8, 131, 138, 141 and 149-151 of `types/jsx.d.ts`. `computes.ts` type-checks
cleanly against these stubs and against the real rc.0 install (`tsc --noEmit`,
TypeScript 5.9.3, `strict`), and the checker reports the same findings against
both.
