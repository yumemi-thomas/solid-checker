# release-triple-store-fresh-rc3

Defect 1 of the rc.1-rc.8 release review
(`docs/package-contract-v2/audits/2026-09-26-solid-2-rc1-rc8-release-review.md`
§ 5): the store typing belongs to `@solidjs/signals`, and a fresh install of the
audited `solid-js@2.0.0-rc.3` does not install the audited signals.

Every `solid-js@2.0.0-rc.N` depends on `@solidjs/signals: ^2.0.0-rc.N`, a
range, so `npm install solid-js@2.0.0-rc.3 @solidjs/web@2.0.0-rc.3` resolves
`@solidjs/signals@2.0.0-rc.9` today. This fixture is that tree: `solid-js` and
`@solidjs/web` at rc.3, signals at rc.9. `solid-js`'s `createStore` is a
re-export (`node_modules/solid-js/index.d.ts`, reduced from rc.3's
`types/index.d.ts:1`), so the `Store<T>` a project sees is signals rc.9's
`Store<T> = T` (`node_modules/@solidjs/signals/index.d.ts`, byte-faithful to
rc.9's `dist/types/store/store.d.ts` and `dist/types/store/index.d.ts:13`).

`tsc --noEmit` (5.9.3, `strict`, bundler resolution) over `store.ts` is clean
against this fixture's stubs and against the real fresh install
(`solid-js@2.0.0-rc.3`, `@solidjs/signals@2.0.0-rc.9`, `@solidjs/web@2.0.0-rc.3`).
The runtime drops the root write outside a setter (probe H).

Before the installation was judged per package, the checker read only the
`solid-js` version, answered rc.3's `Readonly`, and reported `certified` with
no finding. Expected now:

- `profile.name = "Grace"` outside a setter (line 9): **SC2003**. The
  vocabulary takes the store answer from the resolved signals.
- `profile.user.name = "Grace"` (line 12): **SC2003**, as on every release.
- Both writes inside `setProfile(...)`: **none**.
- **SC9014** `unaudited-solid-release`: signals rc.9's own gaps, and the
  three packages are not one reviewed release.

`store.ts` is `store-root-write-rc9`'s, byte for byte.
