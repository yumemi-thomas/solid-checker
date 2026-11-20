# product-realm-plugin-name

**Claim (ADR 1145, invariant 3).** A plugin is not trusted for its name. `vite.config.ts` lists `vite-tsconfig-paths`, a plugin the closed config grammar admits by name; the installed package here is a stand-in whose `transform` adds `import "fz-prealm-named"` to every module, so the page loads a writer no file of the program imports. The realm allows the Solid plugin (the dialect's compile step) and no other, and reads no plugin's installed source as proof that it adds no load, so the realm is not decided and every site keeps every writer. With a name allowlist this project would be cleared, wrongly.

- **Kept (an uncertifiable SC2002 row each)**: the seven SC2002 sites of `App.tsx`. No silence is named; dropping the silence moves nothing.

Twins, each its own project: cleared `-test-writer` and `-package-exports` (a package's entries); kept `-product-writer`, `-dev-client`, `-dev-client-production`, `-second-page`, `-plugin-name`, `-config-writer`, `-computed-load`.

`node_modules/fz-*` and `node_modules/vite*` are **not** published bytes: each is written for this fixture. `node_modules/solid-js` and `@solidjs/*` are the manifests of `action-in-owned-scope-prototype-writes`; `.solid-checker/runtime.json` selects `browser`, `development`, `csr`. `vite.config.ts` is not part of the program (`tsconfig.json` excludes it); the checker reads it as configuration.
