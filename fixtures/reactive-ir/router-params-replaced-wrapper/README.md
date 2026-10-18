# The project sets `paramsWrapper` (ADR 0324 refusal)

`paramsWrapper` replaces the object `useParams()` returns
(`@solidjs/router@2.0.0-next.26` `dist/index.js:1306`). A project that
spells it (or `queryWrapper`, `wrapParams`) anywhere outside the router's own
files leaves the premise unapplied in the whole project, so the site
`router-params/App.tsx` settles stays open here (`SC9012` at the callee's
argument). Declarations as in `router-params`.

The router under `node_modules/@solidjs/router` is the published
`@solidjs/router@2.0.0-next.26` copied byte for byte (`package.json`, `LICENSE`
and every file of `dist/`; the README is omitted, which nothing reads), MIT
licensed. Its digest (`scripts/router-digest.mjs`) is the one the dialect records
for the audited release, `e67af539…04d6`, so the premise's byte check passes
here and the fixture's outcome is its own claim, not a digest mismatch.
