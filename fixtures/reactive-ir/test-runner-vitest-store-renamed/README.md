# test-runner-vitest-store-renamed

ADR 1082's twin: `test-runner-vitest-store` with one edit, the slot's `package.json` `name` set to `vitest-fork` (not published bytes; the edit is the point). The declarations are vitest's, but the package is what its manifest names, so the registration is not a reviewed runner's and the dispatch stays uncertifiable (`function-run-unplaced`, `owner-varies-by-caller`), as before ADR 1082.
