# release-triple-dynamic-rc3

`dynamic(source, { static: true })` on the rc.3 triple gets the default
`dynamic` model, because rc.3's runtime has no static form. The rc.1-rc.8
release review (`docs/package-contract-v2/audits/2026-09-26-solid-2-rc1-rc8-release-review.md`
§§ 3.2, 6) measured rc.9's static model firing here (SC4001 on the effect)
before the answer followed the installed `@solidjs/web`.

`@solidjs/web@2.0.0-rc.3` declares `dynamic(source)` with one parameter
(`types/index.d.ts:81`), so both two-argument calls are
`TS2554: Expected 1 arguments, but got 2`, against this fixture's stub
(`solid-js.d.ts`, byte-faithful for `dynamic` and the types it names) and
against the real rc.3 triple alike. Its bundles never read a second argument:
no build before rc.9 contains `options?.static`. The source is the compute of
the lazy tracked memo `dynamic` builds, under the owner that memo creates,
whatever the call passes.

Expected, each static call exactly as its one-argument twin:

- `StaticEffect`, `DefaultEffect`: **none**. The effect is owned by the memo.
  rc.9's model would call `StaticEffect`'s unowned (SC4001).
- `StaticWrite`, `DefaultWrite`: **SC2001** each. The write runs in the memo's
  tracked compute, where the guard throws. rc.9's model would call
  `StaticWrite`'s legal. This is not TypeScript's TS2554: that says the call
  has one argument too many, this says what the rc.3 runtime does with it.
- `UntrackEffect` (the reference): **SC4001**, as on every release.
- No SC9014: the rc.3 triple is audited.

`node_modules/` holds the three package manifests at `2.0.0-rc.3`.
