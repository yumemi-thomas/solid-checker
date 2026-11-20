# product-realm-dev-client

**Claim (ADR 1145).** Vite itself puts code in the page: the dev server's client in a development build, the `modulepreload` polyfill in a production one. So the package named `vite` (and the packages the Vite configs import) is in the realm whatever the files import. `node_modules/vite` is a stand-in that writes `Float32Array.prototype.reverse` and reaches a Solid action; vitest imports Vite, so `setup.ts` imports it. The realm is decided (a page, the Solid plugin, a closure that loads no package) and holds the writer named `vite`: every site keeps its row. `product-realm-dev-client-production` is the same project under a production build.

- **Kept (an uncertifiable SC2002 row each)**: the seven SC2002 sites of `App.tsx`. No silence is named; dropping the silence moves nothing.

Twins, each its own project: cleared `-test-writer`, `-package-exports` (a package's entries), `-tsconfig-paths-applied`, `-asset-query`, `-diagnostics-off`, `-solid-scope-inert`; kept `-product-writer`, `-dev-client`, `-dev-client-production`, `-second-page`, `-plugin-name`, `-plugin-unpinned`, `-plugin-missing`, `-config-writer`, `-computed-load`, `-js-sibling`, `-subpath-imports`, `-custom-conditions`, `-tsconfig-paths`, `-url-script`, `-other-vite-config`, `-vite-script-config`, `-solid-scope-dependency`, `-diagnostics-on`, `-diagnostics-auto`, `-diagnostics-declared`.

`node_modules/fz-*` and `node_modules/vite*` are **not** published bytes: each is written for this fixture. `node_modules/solid-js` and `@solidjs/*` are the manifests of `action-in-owned-scope-prototype-writes`; `.solid-checker/runtime.json` selects `browser`, `development`, `csr`. `vite.config.ts` is not part of the program (`tsconfig.json` excludes it); the checker reads it as configuration.
