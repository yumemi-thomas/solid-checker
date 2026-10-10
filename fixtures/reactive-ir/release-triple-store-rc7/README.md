# release-triple-store-rc7

The rc.7 triple takes rc.9's store answer (the rc.1-rc.8 release review,
`docs/package-contract-v2/audits/2026-09-26-solid-2-rc1-rc8-release-review.md`
§ 3.1): `@solidjs/signals@2.0.0-rc.7` declares `Store<T> = T`
(`dist/types/store/store.d.ts:4`; the file is byte-identical to rc.8's and
rc.9's), so a write to a store root's own property type-checks and the runtime
still drops it.

`node_modules/@solidjs/signals/index.d.ts` is byte-faithful to rc.7's
`dist/types/store/store.d.ts` and `dist/types/store/index.d.ts:11`;
`node_modules/solid-js/index.d.ts` is the re-export, reduced from rc.7's
`types/index.d.ts:1`. `tsc --noEmit` (5.9.3, `strict`, bundler resolution) over
`store.ts` is clean against these stubs and against the real rc.7 triple.

Before this, rc.7 was analyzed under the audited `Readonly` answer, so nobody
reported the root write. Expected now:

- `profile.name = "Grace"` outside a setter (line 9): **SC2003**.
- `profile.user.name = "Grace"` (line 12): **SC2003**.
- Both writes inside `setProfile(...)`: **none**.
- **SC9014** `unaudited-solid-release`: rc.7 is reviewed with one gap, no
  negative row for its `@solidjs/signals`.

`store.ts` is `store-root-write-rc9`'s, byte for byte.
