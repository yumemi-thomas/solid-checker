# release-triple-dynamic-rc7

`dynamic(source, { static: true })` on the rc.7 triple gets the default
`dynamic` model, because rc.7's runtime has no static form. The rc.1-rc.8
release review (`docs/package-contract-v2/audits/2026-09-26-solid-2-rc1-rc8-release-review.md`
§§ 3.2, 6) measured rc.9's static model firing here before the answer followed
the installed `@solidjs/web`.

`@solidjs/web@2.0.0-rc.7` declares
`dynamic(source, _options?: DynamicOptions)` with
`DynamicOptions { deferStream?: boolean }` (`types/index.d.ts:81-90`), so both
`{ static: true }` literals are `TS2353: Object literal may only specify known
properties, and 'static' does not exist in type 'DynamicOptions'`, against this
fixture's stub (`solid-js.d.ts`) and against the real rc.7 triple alike. The
client bundle never reads `_options` (`dist/web.dev.js:2042`).

Expected, each static call exactly as its one-argument twin:

- `StaticEffect`, `DefaultEffect`: **none** (owned by the memo).
- `StaticWrite`, `DefaultWrite`: **SC2001** each (a write in the memo's
  tracked compute).
- `DeferStreamWrite`: **SC2001**. `{ deferStream: true }` is tsc-clean on rc.7
  and was the default form before too.
- `UntrackEffect` (the reference): **SC4001**.
- **SC9014**: rc.7 is reviewed with one gap, no negative row for its
  `@solidjs/signals`.

The same `App.tsx` over the real rc.9 triple is tsc-clean on the `static`
literals and gets rc.9's form: SC4001 on `StaticEffect`, no SC2001 on
`StaticWrite`.

`node_modules/` holds the three package manifests at `2.0.0-rc.7`.
