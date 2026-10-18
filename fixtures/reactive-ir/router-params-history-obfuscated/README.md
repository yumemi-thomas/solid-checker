# A history that supplies its own `utils` (ADR 0324 refusal)

`createRouter` hands `config.history.utils` to the router, whose
`utils.paramsWrapper` replaces the params object (`dist/routers/factory.jsx:141`,
`dist/index.js:1306`). Here the key is assembled at run time
(`["params", "Wrapper"].join("")`), so no spelling gives it away; the history
is an object literal rather than a stock history, which is what the premise
refuses. The site `App.tsx` settles under a stock history stays open
(`SC9012` at the callee's argument). Declarations as in `router-params`.

The router under `node_modules/@solidjs/router` is the published
`@solidjs/router@2.0.0-next.26` copied byte for byte (`package.json`, `LICENSE`
and every file of `dist/`; the README is omitted, which nothing reads), MIT
licensed. Its digest (`scripts/router-digest.mjs`) is the one the dialect records
for the audited release, `e67af539…04d6`, so the premise's byte check passes
here and the fixture's outcome is its own claim, not a digest mismatch.
