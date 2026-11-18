# flush-universe-fake-timers-store

**Claim (ADR 1086).** `flush-universe-fake-timers` with vitest installed by a linker's store (`node_modules/vitest` a link to `node_modules/.store/vitest-virtual-<slot>/package`, the layout jandibat-web's install has). The fake-timer census (ADR 0967, F1) recognised `vi.useFakeTimers` by the path segments of its declaration (`node_modules/vitest`), which the slot's real path does not have: the program counted as installing no fake timers, and the nine flush rows a scheduler's callback holds were certified on `FreshStack` (a false clean: the test advances the clock inside an action's step, FLUSH_IN_ACTION). The census now reads the declaration's path as the slot's manifest names it (`solid_facts::package_reach::installed_spelling`), and the findings equal `flush-universe-fake-timers`'s.

Published bytes: the slot holds that fixture's `node_modules/vitest` unchanged (its README records the package, version and integrity).
