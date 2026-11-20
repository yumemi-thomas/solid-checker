# product-realm-dev-client-production

**Claim (ADR 1145).** `product-realm-dev-client` under `--runtime-build production`: the page loads no dev client, so the package named `vite` (a writer, imported by the test setup only) is not in the realm and the product's sites are cleared.

- **Cleared (a typed silence, no row)**: the seven SC2002 sites of `App.tsx`. Dropping `package_action_calls.package_action_calls.product-realm` gives them back.
- **Kept**: the test's and setup's rows.

Twins, each its own project: `-test-writer` (cleared), `-package-exports` (cleared, a package's entries), `-dev-client-production` (cleared), and the kept ones `-product-writer`, `-second-page`, `-plugin-name`, `-config-writer`, `-computed-load`, `-dev-client`.

`node_modules/fz-*` and `node_modules/vite*` are **not** published bytes: each is written for this fixture. `node_modules/solid-js` and `@solidjs/*` are the manifests of `action-in-owned-scope-prototype-writes`; `.solid-checker/runtime.json` selects `browser`, `production`, `csr`. `vite.config.ts` is not part of the program (`tsconfig.json` excludes it); the checker reads it as configuration.
