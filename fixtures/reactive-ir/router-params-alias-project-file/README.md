# The router specifier resolves to a project file (ADR 0324 refusal)

`tsconfig.json` maps `@solidjs/router` to `router-shim.ts`, so the import is not the installed package and the audited release says nothing about it: the premise is not applied (the router notice reports the unknown release) and the site stays open (`SC9012`). No router package is installed.
