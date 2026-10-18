# Stock histories keep the router premise (ADR 0324)

`createRouter` takes its integration from `config.history`
(`@solidjs/router@2.0.0-next.26` `dist/routers/factory.jsx:141,187,207`), and the
integration's `utils.paramsWrapper` replaces the params object
(`dist/index.js:1306`). The premise therefore holds only while every router
creation names no history, or a stock one (`browserHistory`, `hashHistory`,
`memoryHistory`, `routers/history.js:91,131,178`, none of which sets
`paramsWrapper`) called directly and never given a `utils`. `router.ts` builds
three routers that way: no history, a direct call, and a `const` the project
only `get`s and `set`s. The site `App.tsx` `Screen` feeds is settled (no `SC9012`
at the callee's argument). Declarations as in `router-params`.

The router under `node_modules/@solidjs/router` is the published
`@solidjs/router@2.0.0-next.26` copied byte for byte (`package.json`, `LICENSE`
and every file of `dist/`; the README is omitted, which nothing reads), MIT
licensed. Its digest (`scripts/router-digest.mjs`) is the one the dialect records
for the audited release, `e67af539…04d6`, so the premise's byte check passes
here and the fixture's outcome is its own claim, not a digest mismatch.
