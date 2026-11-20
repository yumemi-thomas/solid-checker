# product-realm-dev-client-production

**Claim (ADR 1145).** `product-realm-dev-client` under `--runtime-build production`: the page does not load the dev client, but Vite adds its `modulepreload` polyfill (the package named `vite`) to every page's entry, so the stand-in writer named `vite` stays in the realm in this build too, and the product's sites keep their rows. A realm that dropped `vite` outside a development build would clear them wrongly if a writer lived in that package's browser code.

- **Kept (an uncertifiable SC2002 row each)**: the seven SC2002 sites of `App.tsx`. No silence is named; dropping the silence moves nothing.

Twins, each its own project: cleared `-test-writer` and `-package-exports` (a package's entries); kept `-product-writer`, `-dev-client`, `-dev-client-production`, `-second-page`, `-plugin-name`, `-config-writer`, `-computed-load`.

`node_modules/fz-*` and `node_modules/vite*` are **not** published bytes: each is written for this fixture. `node_modules/solid-js` and `@solidjs/*` are the manifests of `action-in-owned-scope-prototype-writes`; `.solid-checker/runtime.json` selects `browser`, `production`, `csr`. `vite.config.ts` is not part of the program (`tsconfig.json` excludes it); the checker reads it as configuration.
