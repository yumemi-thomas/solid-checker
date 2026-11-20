# product-realm-config-writer

**Claim (ADR 1145).** The bundler configuration is read in a closed grammar: its imports are the config helper and the Solid plugin. A config that imports anything else (here a bare `import "fz-prealm-named"`, which runs the writer in the build process) may do what the grammar does not read, so the realm is not decided and every site keeps every writer.

- **Kept (an uncertifiable SC2002 row each)**: the seven SC2002 sites of `App.tsx`. No silence is named; dropping the silence moves nothing.

Twins, each its own project: cleared `-test-writer`, `-package-exports` (a package's entries), `-tsconfig-paths-applied`, `-asset-query`, `-diagnostics-off`, `-solid-scope-inert`; kept `-product-writer`, `-dev-client`, `-dev-client-production`, `-second-page`, `-plugin-name`, `-plugin-unpinned`, `-plugin-missing`, `-config-writer`, `-computed-load`, `-js-sibling`, `-subpath-imports`, `-custom-conditions`, `-tsconfig-paths`, `-url-script`, `-other-vite-config`, `-vite-script-config`, `-solid-scope-dependency`, `-diagnostics-on`, `-diagnostics-auto`, `-diagnostics-declared`.

`node_modules/fz-*` and `node_modules/vite*` are **not** published bytes: each is written for this fixture. `node_modules/solid-js` and `@solidjs/*` are the manifests of `action-in-owned-scope-prototype-writes`; `.solid-checker/runtime.json` selects `browser`, `development`, `csr`. `vite.config.ts` is not part of the program (`tsconfig.json` excludes it); the checker reads it as configuration.
