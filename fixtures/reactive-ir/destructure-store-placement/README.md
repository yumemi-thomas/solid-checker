# destructure-store-placement

**Claim (ADR 1106).** SC1003 claims a store destructure is a frozen setup-time snapshot. It is a violation only where setup provably runs the destructure: in the component's body, or through layers that run in place there (an IIFE, a `forEach` callback, a project helper that calls its argument, a primitive's in-place slot, a flow component's function child). Where the analysis cannot place its run it is uncertifiable, never silent and never a violation:

| component | outcome |
|---|---|
| `InBody`, `InPlaceLayer` | SC1003 violation |
| `NamedHelper` (a helper the body calls: was silent) | SC1003 uncertifiable |
| `InApply` (an effect's apply: was silent) | SC1003 uncertifiable |
| `NestedInRef` (a function nested in a named ref callback: was silent) | SC1003 uncertifiable |
| `InListener`, `InMethod` (a listener, an object's method: were violations) | SC1003 uncertifiable |
| `HandlerHelper` (a helper only a click handler calls) | no row |

Chrome: `fixtures/runtime-oracle/destructure-store-named-helper` (STRICT_READ_UNTRACKED at the destructure). The fuzzer's `store-dstr-object` variant (`scripts/fuzz/flip-read.mjs`) generates the family. Stubs: `handed-literal-prop-read`'s, plus `createStore`.
