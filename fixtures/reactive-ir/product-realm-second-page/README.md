# product-realm-second-page

**Claim (ADR 1145).** A second HTML page may share a module with the first and load a writer the first never loads, so the realm is not decided and every site keeps every writer. `admin.html` names `/admin.ts`, which imports `./App` (the module `index.html`'s page also loads) and `fz-prealm-named`. The setup file loads both writers, as in `product-realm-test-writer`, where the same product is cleared.

- **Kept (an uncertifiable SC2002 row each)**: the seven SC2002 sites of `App.tsx`, as with no realm at all. No silence is named; dropping the silence moves nothing.

Twins, each its own project: `-test-writer` (cleared), `-package-exports` (cleared, a package's entries), `-dev-client-production` (cleared), and the kept ones `-product-writer`, `-second-page`, `-plugin-name`, `-config-writer`, `-computed-load`, `-dev-client`.

`node_modules/fz-*` and `node_modules/vite*` are **not** published bytes: each is written for this fixture. `node_modules/solid-js` and `@solidjs/*` are the manifests of `action-in-owned-scope-prototype-writes`; `.solid-checker/runtime.json` selects `browser`, `development`, `csr`. `vite.config.ts` is not part of the program (`tsconfig.json` excludes it); the checker reads it as configuration.
