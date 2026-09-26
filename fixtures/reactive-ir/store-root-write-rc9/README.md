# store-root-write-rc9

B1 of the rc.9 vocabulary review
(`docs/package-contract-v2/audits/2026-09-26-solid-2-rc9-vocabulary-review.md`
§ 4.1): on `solid-js@2.0.0-rc.9` a write to a `createStore` root's own property
type-checks and is still dropped at runtime, so SC2003 reports it.

`@solidjs/signals@2.0.0-rc.9` declares `Store<T> = T`
(`dist/types/store/store.d.ts:4`). `tsc --noEmit` (5.9.3, `strict`, bundler
resolution) over `store.ts` against the **real published** `solid-js@2.0.0-rc.9`
install, not this stub, reports nothing, and neither does this fixture's stub,
which is byte-faithful to the published declarations (see
`node_modules/solid-js/index.d.ts` for what was reduced). The runtime half is
the review's probe H: the direct `store.a = 99` is dropped on rc.9, dev and prod.

Expected findings:

- `profile.name = "Grace"` outside a setter (line 9): **SC2003**. Before this
  fixture, neither `tsc` nor the checker reported it, and `--certify` said
  `certified`. The dialect's rc.9 vocabulary answers
  `store_root_properties_are_readonly() == false`.
- `profile.user.name = "Grace"` (line 12): **SC2003**, as on rc.3.
- Both writes inside `setProfile(...)`: **none**. The store's own setter
  write-enables its proxy on rc.9 exactly as on rc.3 (the review's probe T).
- **SC9014** `unaudited-solid-release` at `node_modules/solid-js/package.json`:
  rc.9 is reviewed with gaps still open, so the analysis cannot certify.

The twin is `store-root-write-rc3`; the two differ only in the installed
version and the declarations copied from it.
