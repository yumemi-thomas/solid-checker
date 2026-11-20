# product-realm-url-script

**The premise (ADR 1145, review round 1).** The closure follows TypeScript's attested resolution and the page is built by Vite's; the realm holds only where they agree, so each place they can differ ends the read. `main.tsx` imports `fz-prealm-any/index.js?url` and puts the URL in a script element: the page runs the installed writer, and no import names it. A query on a script module or a package (`?url`, `?raw`, `?inline`, `?worker`) ends the read; only a data file's query (a style sheet's `?inline`, an image's `?url`) is no load (`product-realm-asset-query`).

- **Kept (an uncertifiable SC2002 row each)**: the seven SC2002 sites of `App.tsx`. No silence is named; dropping the silence moves nothing.

Twins, each its own project: cleared `-test-writer`, `-package-exports` (a package's entries), `-tsconfig-paths-applied`, `-asset-query`; kept `-product-writer`, `-dev-client`, `-dev-client-production`, `-second-page`, `-plugin-name`, `-plugin-unpinned`, `-plugin-missing`, `-config-writer`, `-computed-load`, `-js-sibling`, `-subpath-imports`, `-custom-conditions`, `-tsconfig-paths`, `-url-script`, `-other-vite-config`, `-vite-script-config`.

`node_modules/fz-*` and `node_modules/vite*` are **not** published bytes: each is written for this fixture. `node_modules/solid-js` and `@solidjs/*` are the manifests of `action-in-owned-scope-prototype-writes`; `.solid-checker/runtime.json` selects `browser`, `development`, `csr`. `vite.config.ts` is not part of the program (`tsconfig.json` excludes it); the checker reads it as configuration.
