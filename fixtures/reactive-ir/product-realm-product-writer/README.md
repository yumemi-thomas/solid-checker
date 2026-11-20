# product-realm-product-writer

**Claim (ADR 1145).** A writer the page itself loads is judged as before. `main.tsx` imports `fz-prealm-named` (writes `Float32Array.prototype.reverse`); `setup.ts` imports it and `fz-prealm-any` (a key the scan does not decide). The page loads the named writer and not the any-name one, so the realm keeps the first and drops the second.

- **Kept (an uncertifiable SC2002 row each)**: the seven sites of `App.tsx`. Under a writer by name every member call, constructor, template substitution and converting operation may run the written member (ADR 1114 family 3 slice E), so each stays a site; what changes is the writer it names (`prototype-named-…` where the whole program's writers gave `prototype-any-…`), not whether the site stands. No site is cleared, no silence is named, and dropping the silence moves nothing.
- **Kept**: `main.tsx`'s import of the writer (`package-finding-at-entry`) and the test's and setup's rows, as in `product-realm-test-writer`.

Twins, each its own project: cleared `-test-writer` and `-package-exports` (a package's entries); kept `-product-writer`, `-dev-client`, `-dev-client-production`, `-second-page`, `-plugin-name`, `-config-writer`, `-computed-load`.

`node_modules/fz-*` and `node_modules/vite*` are **not** published bytes: each is written for this fixture. `node_modules/solid-js` and `@solidjs/*` are the manifests of `action-in-owned-scope-prototype-writes`; `.solid-checker/runtime.json` selects `browser`, `development`, `csr`. `vite.config.ts` is not part of the program (`tsconfig.json` excludes it); the checker reads it as configuration.
