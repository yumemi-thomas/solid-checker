# product-realm-tsconfig-paths-applied

**The premise (ADR 1145, review round 1).** The closure follows TypeScript's attested resolution and the page is built by Vite's; the realm holds only where they agree, so each place they can differ ends the read. The same project as `product-realm-tsconfig-paths`, with `resolve: { tsconfigPaths: true }` in `vite.config.ts` and the project the directory's `tsconfig.json`: Vite applies the same `paths`, both resolve `fz-prealm-any` to `./shim.ts`, and the realm is decided. The page loads the shim and not the writer (the test setup loads it by its file), so the seven sites are cleared.

- **Cleared (a typed silence, no row)**: the seven SC2002 sites of `App.tsx`. Dropping `package_action_calls.package_action_calls.product-realm` gives them back.
- **Kept**: the test's and setup's rows.

Twins, each its own project: cleared `-test-writer`, `-package-exports` (a package's entries), `-tsconfig-paths-applied`, `-asset-query`, `-diagnostics-off`, `-solid-scope-inert`; kept `-product-writer`, `-dev-client`, `-dev-client-production`, `-second-page`, `-plugin-name`, `-plugin-unpinned`, `-plugin-missing`, `-config-writer`, `-computed-load`, `-js-sibling`, `-subpath-imports`, `-custom-conditions`, `-tsconfig-paths`, `-url-script`, `-other-vite-config`, `-vite-script-config`, `-solid-scope-dependency`, `-diagnostics-on`, `-diagnostics-auto`, `-diagnostics-declared`.

`node_modules/fz-*` and `node_modules/vite*` are **not** published bytes: each is written for this fixture. `node_modules/solid-js` and `@solidjs/*` are the manifests of `action-in-owned-scope-prototype-writes`; `.solid-checker/runtime.json` selects `browser`, `development`, `csr`. `vite.config.ts` is not part of the program (`tsconfig.json` excludes it); the checker reads it as configuration.
