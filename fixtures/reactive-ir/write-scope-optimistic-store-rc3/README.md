# write-scope-optimistic-store-rc3

Twin of `write-scope-optimistic-store-rc0` (same `computes.ts`) over the audited rc.3 triple, where every store setter's entry is guarded (`devGuardStoreSetterWrite`), so all seven owned-scope writes are SC2001 violations (optimistic setters in a memo/effect compute, `createStore` and signal setters, both aliases). `node_modules/` is `write-scope-roots-rc3`'s byte for byte (published rc.3 typings); `tsc` clean against stubs and real rc.3.
