# destructure-store-placement

**Claim (ADR 1106; ADR 1107 gives a slot SC1001's verdict, `destructure-store-slots`).** SC1003 claims a store destructure is a frozen setup-time snapshot. It is a violation only where setup provably runs the destructure: in the component's body, or through layers that run in place there (an IIFE, a `forEach` callback, a project helper that calls its argument, a primitive's in-place slot, a flow component's function child). Where the analysis cannot place its run it is uncertifiable, never silent and never a violation:

| component | outcome |
|---|---|
| `InBody`, `InPlaceLayer` | SC1003 violation |
| `NamedHelper` (a helper the body calls: was silent) | SC1003 uncertifiable (`summary-read-unplaced`: the slot's summary row is not proven direct; SC1001 proves `store.v` there, docs/open-work.md) |
| `InApply` (an effect's apply: was silent) | SC1003 violation (ADR 1107: SC1001's verdict for `store.v` there) |
| `NestedInRef` (a function nested in a named ref callback: was silent) | SC1003 violation (ADR 1107) |
| `InListener`, `InMethod` (a listener, an object's method: were violations) | SC1003 uncertifiable |
| `HandlerHelper` (a helper only a click handler calls) | no row |

Chrome: `fixtures/runtime-oracle/destructure-store-named-helper` (STRICT_READ_UNTRACKED at the destructure). The fuzzer's `store-dstr-object` variant (`scripts/fuzz/flip-read.mjs`) generates the family. Stubs: `handed-literal-prop-read`'s, plus `createStore`.
