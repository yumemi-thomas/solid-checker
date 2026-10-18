# A history that a dependency builds (ADR 0324 refusal)

`router.ts` passes `fancyHistory()` of `node_modules/fancy-history`, which
returns `{ ...memoryHistory(), utils: { ...h.utils, paramsWrapper } }`. The
checker cannot read that package's behavior here, and the premise needs every
router creation's history to be a stock history called directly, so it is not
applied: the site `App.tsx` settles under a stock history stays open
(`SC9012` at the callee's argument). The package is a stand-in: only
the declaration that the call's callee resolves through matters.
Declarations as in `router-params`.

The router under `node_modules/@solidjs/router` is the published
`@solidjs/router@2.0.0-next.26` copied byte for byte (`package.json`, `LICENSE`
and every file of `dist/`; the README is omitted, which nothing reads), MIT
licensed. Its digest (`scripts/router-digest.mjs`) is the one the dialect records
for the audited release, `e67af539…04d6`, so the premise's byte check passes
here and the fixture's outcome is its own claim, not a digest mismatch.
