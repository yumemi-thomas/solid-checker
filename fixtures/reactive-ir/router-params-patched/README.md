# A recorded patch of the router (ADR 0324 refusal)

`package.json` records `patchedDependencies` for
`@solidjs/router@2.0.0-next.26`: the installed files are that release's bytes
with a patch applied, under the same version string. The audit read the
published bytes (ADR 0300 trusts them by identity), so a visible patch record
leaves the premise unapplied, and the site `App.tsx` settles under the
published router stays open (`SC9012` at the callee's argument). Declarations
as in `router-params`.

The router under `node_modules/@solidjs/router` is the published
`@solidjs/router@2.0.0-next.26` copied byte for byte (`package.json`, `LICENSE`
and every file of `dist/`; the README is omitted, which nothing reads), MIT
licensed. Its digest (`scripts/router-digest.mjs`) is the one the dialect records
for the audited release, `e67af539…04d6`, so the premise's byte check passes
here and the fixture's outcome is its own claim, not a digest mismatch.
