# flush-universe-package-hand-off-receiver-key

**Claim (the review of #229, B2, P10; ADR 0643's premise; ADR 0967 amended).** A write through a key the program does not name (`protoKey[protoKeyName] = protoKeyBase`, the name built from two strings) may install a prototype, whatever the files spell. A getter of the literal reads the prototype's getter through `this` in a step `steplib` (its source is not read) runs, so the getter that flushes may run on a package's stack: it keeps its three uncertifiable rows (SC2006 `flush-in-action-unaccounted`, SC2008 `flush-in-effect-unaccounted`, SC3001 `leaf-operation-unaccounted`), never silence, never a violation. Chrome (rc.21) reports FLUSH_IN_ACTION; before this the checker certified it (`scripts/fuzz/patterns/pkghand/proto-key-step`). A value stored under a key the program does not name is stored under any key, so any read through a receiver in code a package may run reaches it.

No file of the program spells `__proto__`: the twins of this shape, and a fixture that spells the key, are other programs (`flush-universe-package-hand-off-receiver-reads-twins`, `flush-universe-package-hand-off-receiver-key-source`).

Published bytes: `node_modules/steplib` is synthetic; `.solid-checker/runtime.json` closes the program boundary.
