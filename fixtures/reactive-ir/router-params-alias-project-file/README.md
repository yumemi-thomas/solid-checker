# The router specifier resolves to a project file (ADR 0324 refusal)

`tsconfig.json` maps `@solidjs/router` to `router-shim.ts`, so the import is
not the installed package and the audited release says nothing about it. The
premise is not applied (the import's release is unknown, which the router
notice reports) and the site `App.tsx` settles under the installed router stays
open (`SC9012` at the callee's argument). No router package is installed.
