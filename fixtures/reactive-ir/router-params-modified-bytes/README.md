# An install whose bytes are not the audited release's (ADR 0324 refusal)

`node_modules/@solidjs/router/dist/index.js` is the published `2.0.0-next.26`
with `wrapParams` (`dist/index.js:1306`) changed to return `{ id: { run() {} } }`,
so `params.id` is an object at run time. The manifest still says
`2.0.0-next.26`, and nothing in the project's manifests or lockfiles records the
change (a `patch-package` patch, a fork under the same version and a hand edit
look exactly like this). The premise checks the installed bytes (the digest of
the manifest fields and every file of `dist/`, `scripts/router-digest.mjs`)
against the audited release's, finds them different, and is not applied: the
site `App.tsx` settles under the real router stays open (`SC9012` at the
callee's argument) and an `SC9014` notice says the installed bytes differ from
the audited release. Everything else is the published package, as in
`router-params`.
