# product-realm-package-exports

**Claim (ADR 1145).** A package's `exports` are its entries: there is no page, and `package.json` names `./index.tsx` (the same file as `main`). The import closure of `index.tsx` is itself and loads no package, so the realm holds none of the writers `setup.ts` (vitest's `setupFiles`, in `vite.config.ts`) imports. This is the shape of the kui workspace's packages.

- **Cleared (a typed silence, no row)**: the seven SC2002 sites of `index.tsx`. Dropping `package_action_calls.package_action_calls.product-realm` gives them back.
- **Kept**: `index.test.tsx` (it imports the entry; a test is not the product), `setup.ts`'s imports, and `index.tsx:3`'s SC9012 `standard-member-impostor` row (another consumer of the writer set, `docs/open-work.md`).

Twins, each its own project: cleared `-test-writer` and `-package-exports` (a package's entries); kept `-product-writer`, `-dev-client`, `-dev-client-production`, `-second-page`, `-plugin-name`, `-config-writer`, `-computed-load`.

`node_modules/fz-*` and `node_modules/vite*` are **not** published bytes: each is written for this fixture. `node_modules/solid-js` and `@solidjs/*` are the manifests of `action-in-owned-scope-prototype-writes`; `.solid-checker/runtime.json` selects `browser`, `development`, `csr`. `vite.config.ts` is not part of the program (`tsconfig.json` excludes it); the checker reads it as configuration.
