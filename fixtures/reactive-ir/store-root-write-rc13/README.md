# store-root-write-rc13

The audited-release control of ADR 0194: `store-root-write-rc9` on the
`2.0.0-rc.13` triple. B1 of the rc.9 review holds on rc.13 (the rc.13 review,
`docs/package-contract-v2/audits/2026-10-05-solid-2-rc13-vocabulary-review.md`
§ 0): `@solidjs/signals@2.0.0-rc.13` still declares `Store<T> = T`
(`dist/types/store/store.d.ts:4`), so a write to a store root's own property
type-checks and the runtime still drops it.

`tsc --noEmit` over `store.ts` against the **real published** rc.13 install
(not this stub) reports nothing. The stub is byte-faithful to the published
declarations it keeps (`Store`, `StoreSetter`, `StoreOptions`, `NoFn`, the plain
`createStore` overload); `solid-js`'s re-export is reduced as its `index.d.ts`
says.

Expected findings:

- `profile.name = "Grace"` outside a setter (line 9): **SC2003**.
- `profile.user.name = "Grace"` (line 12): **SC2003**.
- Both writes inside `setProfile(...)`: **none**.
- **No `SC9014`**: rc.13 is the audited release, with no open gap. This is
  the one difference from `store-root-write-rc9`, which now carries the notice
  that rc.9 is older than the audited release.
