# test-runner-vitest-store

**Claim (ADR 1082).** A reviewed runner installed by a linker's store (`node_modules/vitest` a link to `node_modules/.store/vitest-virtual-<slot>/package`, as jandibat-web's install has it) is the runner its manifest names: the compiler gives `it` its real path inside the slot, and the registration is recognised by the `name` of the slot's `package.json` (`PackageReachIndex::name_of`), spelled `node_modules/vitest/…`. The findings equal `test-runner-vitest`'s (same `App.tsx` bytes; its SC4001 violation is that fixture's, ADR 0438).

Published bytes: the slot holds `test-runner-vitest`'s `node_modules/vitest` unchanged (`vitest@5.0.5`, declaration files only; integrity in that fixture's README). Twin: `test-runner-vitest-store-renamed` (the slot's manifest names `vitest-fork`): not the runner, the rows stay uncertifiable.
