# owner-call-leaf-universe

**Claim (batch 11, slice 6; ADR 0969).** Every call of `onCleanup` and of a creation a leaf owner refuses (`createMemo`, `createEffect`, `createRoot`, `render`, ...) is a site of SC3001 (CLEANUP_IN_FORBIDDEN_SCOPE, PRIMITIVE_IN_FORBIDDEN_SCOPE): it ends in a row of the rule, an SC9012, the compiler's discard, or a silence that names the fact the throw needs and the code lacks; with none it gets `leaf-operation-unaccounted`, an uncertifiable row, never a violation. The program opens a leaf scope (`createTrackedEffect`), so the program census (`no-opener-in-program`) does not answer for any call here; each is asked of the owner graph. The boundary is closed (`.solid-checker/runtime.json`), so an export is entered by project call sites only.

| site | outcome | why |
|---|---|---|
| `createMemo` in `Opener`'s `createTrackedEffect` callback | SC3001 violation | the leaf walk's own finding stands at the creation |
| `useDouble`'s `createMemo` and `onCleanup` (called by `Plain`'s body only) | silent, `owner-context-no-leaf` | the function's context has an edge, no leaf bit, and every entry into it is a call the graph holds (the same silence answers PRIMITIVE_IN_EFFECT_CALLBACK as `owner-context-current`: an owner is current on every entry) |
| `S.createMemo` (namespace spelling) in `viaNamespace` | silent, the same silence | the same site as the named import's |
| `useBoth`'s `createMemo`, also called from a leaf callback | the leaf walk's row at the helper call | the context has a leaf bit: the silence does not answer |
| `stored`'s `createMemo` and `onCleanup`, kept in an array | `leaf-operation-unaccounted` (SC3001 uncertifiable, beside the ADR 0710 and SC4001 rows that stand) | the function is entered by a reference that is no direct call: the graph holds no edge for that entry, and a package or later code may run it under a leaf owner |
| the local function `createMemo` in `shadowed` | no site | the callee is not the primitive |
| `render(...)` at module scope | silent, `module-scope` | module evaluation runs it once, on a stack of its own |

SC4001's rows stand where they stood (`stored`'s `onCleanup` keeps its `function-run-unplaced`): an SC4001 obligation does not decline SC3001's code (ruling of batch 11, slice 6), so `leaf-operation-unaccounted` is filed beside it. `node_modules` holds a reduced `solid-js` and `@solidjs/signals` (declarations as rc.21 has them: the package layout gives the callee a package root, which the effect-phase walk asks) and `@solidjs/web`'s `render`.
