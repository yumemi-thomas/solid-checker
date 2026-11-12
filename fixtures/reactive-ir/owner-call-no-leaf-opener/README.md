# owner-call-no-leaf-opener

**Claim (batch 11, slice 6; ADR 0969).** The program census answers for SC3001's two codes at a call whose function no package the build does not read may run: the project names no primitive that opens a leaf scope (`createTrackedEffect`, `onSettled`), so no leaf owner exists to forbid `onCleanup` or a creation (`no-opener-in-program`, premise `ProgramCensus`). Here `api.run()` is a method of a local object, called directly: no owner edge enters a method (the owner graph answers nothing for it), so only the census speaks, and the `onCleanup` is silent. The boundary is closed (`.solid-checker/runtime.json`).

The census is a fact about the whole program and about what may run its functions, so it is never silence by itself; the variants each take one premise away:

- `variants/with-opener`: the same program with one `createTrackedEffect`. The census no longer answers and the method's `onCleanup` keeps an uncertifiable SC3001 row (`function-run-unplaced`, the ADR 0963 net's, which the census had gated off).
- `variants/package-opener` (development build, `dependencySources: ["dep-leaf"]`): the project names no leaf primitive, but `dep-leaf`, analyzed as source, opens a leaf scope and runs the callback it is handed. The census does not answer (the build's primitives include the package's: with a `dep-leaf` that calls the callback directly the same program is silent), so the callback's `onCleanup` is `leaf-operation-unaccounted`.
- `variants/package-unread` (standard build): the same program, the package's source not read (`package-source-unavailable`, SC9005). Nothing here shows what the package does, and a package the build does not read may run the function (ADR 0967): the census does not answer, and the `onCleanup` is `leaf-operation-unaccounted`.

SC4001's `owner-callers-unenumerated` / `function-run-unplaced` rows stand beside SC3001's where the owner graph has them: an SC4001 obligation does not decline SC3001's code. `node_modules` holds the reduced `solid-js`, `@solidjs/signals` and `@solidjs/web` of `owner-call-leaf-universe`; `dep-leaf` is synthetic (a package that opens a leaf scope itself).
