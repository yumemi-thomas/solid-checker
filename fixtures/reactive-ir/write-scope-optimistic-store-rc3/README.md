# write-scope-optimistic-store-rc3

The twin of `write-scope-optimistic-store-rc0`: the same `computes.ts`, byte
for byte, over the audited rc.3 triple, where every store setter's entry is
guarded (`devGuardStoreSetterWrite`, `@solidjs/signals@2.0.0-rc.3`), so all
seven owned-scope writes are **SC2001** violations: the plain and derived
`createOptimisticStore` setters in a memo compute, the optimistic setter in an
effect compute, the `createStore` and signal setters, and both aliases. Probed
on every rc.1-rc.9 dev client build (see the rc.0 twin's README); the
comments in `computes.ts` describe rc.0.

**Stubs.** `node_modules/` is `write-scope-roots-rc3`'s, byte for byte.
`computes.ts` type-checks cleanly against it and against the real rc.3 install
(`tsc --noEmit`, TypeScript 5.9.3, `strict`), and the checker reports the same
findings against both.
