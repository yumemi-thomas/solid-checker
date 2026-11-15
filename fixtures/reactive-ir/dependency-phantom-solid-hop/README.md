# dependency-phantom-solid-hop

**Claim (ADR 0981).** The phantom import one hop deep: `dist/index.js` (the entry) imports `./routing.js`, which imports `solid-js`, and the manifest names no Solid package.

The program is `fixtures/runtime-oracle/g2-holder-none` (the check of #227, G2) and the package is the `dep-router` of the `dependency-source-integrity-*` fixtures with no `dependencySources` pin: the default selection reads the closure (entry, `routing.js`, `router.jsx`) and finds the import. SC1001 violation at `location.search`. Chrome: `fixtures/runtime-oracle/g2-holder-none` is this program with this package, pinned: STRICT_READ_UNTRACKED at the read.
