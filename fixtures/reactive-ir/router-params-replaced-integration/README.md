# The project names a `RouterIntegration` (ADR 0324 refusal)

A custom `RouterIntegration` is what supplies `utils.paramsWrapper`
(`types.d.ts:285`, `routers/history.d.ts:14`). A project that names the type
leaves the premise unapplied in the whole project, so the site
`router-params/App.tsx` settles stays open here. Declarations as in
`router-params`.

The router under `node_modules/@solidjs/router` is the published
`@solidjs/router@2.0.0-next.26` copied byte for byte (`package.json`, `LICENSE`
and every file of `dist/`; the README is omitted, which nothing reads), MIT
licensed. Its digest (`scripts/router-digest.mjs`) is the one the dialect records
for the audited release, `e67af539…04d6`, so the premise's byte check passes
here and the fixture's outcome is its own claim, not a digest mismatch.
