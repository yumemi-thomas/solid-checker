# effect-cleanup-rerun-layers

**Claim (ADR 1195).** A read or a write in function layers run in place inside the cleanup an inline apply returns (an IIFE, a closure bound to a name and called once, a project helper's callback; ADR 0493's climb, no library or primitive callback, no await on the way) runs where the cleanup runs:
- a read there, on the rerun a reached click makes while the component is mounted (ADR 1183), runs in the apply's strict-read window: SC1001 violations (`ReadInClosure`, `ReadInIife`, `ReadInHelperCallback`; uncertifiable on main);
- a write there is legal on every run of the cleanup, as one written in its body is (ADR 1037): clean (`WriteInClosure`, `WriteInIife`, `WriteInHelperCallback`; uncertifiable on main).

Uncertifiable as before: a `forEach` callback (may run zero times), a closure the cleanup stores, a read past an await in an async IIFE, and a write in a helper the component body calls too (`WriteInSharedHelper`).

Chrome: `fixtures/runtime-oracle/effect-cleanup-rerun-layers`. Stubs: `effect-cleanup-rerun-reached`'s. `tsc --noEmit` clean.
