# A stock history the project passes on (ADR 0324 refusal)

`router.ts` calls `memoryHistory()` directly, but then hands the history to
`install`, which writes a `utils` onto it before `createRouter` reads
`config.history.utils` (`dist/routers/factory.jsx:141,187`). A history the
project uses beyond `get` and `set` is not provably stock, so the premise is
not applied and the site `App.tsx` settles under a stock history stays open
(`SC9012` at the callee's argument). Declarations as in `router-params`.

The router under `node_modules/@solidjs/router` is the published
`@solidjs/router@2.0.0-next.26` copied byte for byte (`package.json`, `LICENSE`
and every file of `dist/`; the README is omitted, which nothing reads), MIT
licensed. Its digest (`scripts/router-digest.mjs`) is the one the dialect records
for the audited release, `e67af539…04d6`, so the premise's byte check passes
here and the fixture's outcome is its own claim, not a digest mismatch.
