# write-scope-optimistic-store-rc0

On `@solidjs/signals@2.0.0-rc.0` a `createOptimisticStore` setter in a memo or effect compute is legal (its write path never reaches `setSignal`'s guard; rc.1 added `devGuardStoreSetterWrite`), so `SC2001` does not report it while `createStore` and signal setters there are reported; only tuple-bound setters carry provenance. Twin `write-scope-optimistic-store-rc3`. Stubs are byte-for-byte from the published rc.0 tarballs (`dist/types/`, `types/client/hydration.d.ts`, `types/types.d.ts`, `types/jsx.d.ts` lines 8, 131, 138, 141, 149-151; JSDoc dropped).
