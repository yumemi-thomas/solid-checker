# product-realm-computed-load

**Claim (ADR 1145).** A load by a specifier nothing proves may load any module, a writer among them, so the realm is not decided. `main.tsx` exports `load = (name: string) => import(name)`; a literal `import("./x")` is followed, a computed one ends the read. Every site keeps every writer.

- **Kept (an uncertifiable SC2002 row each)**: the seven SC2002 sites of `App.tsx`. No silence is named; dropping the silence moves nothing.

Twins, each its own project: `-test-writer` (cleared), `-package-exports` (cleared, a package's entries), `-dev-client-production` (cleared), and the kept ones `-product-writer`, `-second-page`, `-plugin-name`, `-config-writer`, `-computed-load`, `-dev-client`.

`node_modules/fz-*` and `node_modules/vite*` are **not** published bytes: each is written for this fixture. `node_modules/solid-js` and `@solidjs/*` are the manifests of `action-in-owned-scope-prototype-writes`; `.solid-checker/runtime.json` selects `browser`, `development`, `csr`. `vite.config.ts` is not part of the program (`tsconfig.json` excludes it); the checker reads it as configuration.
