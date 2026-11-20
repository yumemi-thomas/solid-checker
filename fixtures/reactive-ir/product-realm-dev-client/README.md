# product-realm-dev-client

**Claim (ADR 1145).** In a development build the page also loads the dev server's client, so the package named `vite` is in the realm whatever the files import. `node_modules/vite` is a stand-in that writes `Float32Array.prototype.reverse` and reaches a Solid action; vitest imports Vite, so `setup.ts` imports it. The realm is decided (a page, the Solid plugin, a closure that loads no package) and holds the writer named `vite`: every site keeps its row. The twin `product-realm-dev-client-production` is the same project under a production build, where the page loads no dev client and the sites are cleared.

- **Kept (an uncertifiable SC2002 row each)**: the seven SC2002 sites of `App.tsx`. No silence is named; dropping the silence moves nothing.

Twins, each its own project: `-test-writer` (cleared), `-package-exports` (cleared, a package's entries), `-dev-client-production` (cleared), and the kept ones `-product-writer`, `-second-page`, `-plugin-name`, `-config-writer`, `-computed-load`, `-dev-client`.

`node_modules/fz-*` and `node_modules/vite*` are **not** published bytes: each is written for this fixture. `node_modules/solid-js` and `@solidjs/*` are the manifests of `action-in-owned-scope-prototype-writes`; `.solid-checker/runtime.json` selects `browser`, `development`, `csr`. `vite.config.ts` is not part of the program (`tsconfig.json` excludes it); the checker reads it as configuration.
