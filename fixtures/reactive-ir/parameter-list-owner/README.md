# parameter-list-owner

**Claim (ADR 0454).** A site in a function's parameter list (a default value, a pattern, a function written in either) belongs to the function whose list it is and runs only when that function is called without the argument. It is no code of the function or helper around it.

- `*NeverCalled` (negatives, uncertifiable): a helper builds a closure whose default calls the callback (also through `buildForward`, in a branch, returned directly, destructured, as a declaration, through an IIFE) and never calls it; rc.17 throws nothing (`oracle-behaviour-parameter-list-owner`). `SC2001` was a violation.
- `NestedCalledInHandler`, `NestedCalledOmitted`, `NestedCalledSupplied`, `OwnDefaultOmitted` (uncertifiable): the closure is called in a handler, in the memo without the argument (it runs: a known miss, no call site is kept for a function whose list runs code, ADR 0339), with it, and the helper's own list. No proof that a caller omits the argument is built.
- `BodyCall`, `ReadBodyCall`, `LeafDirect` (positives, violations): a call in the helper's body, an accessor it reads in its body, a forbidden call directly in a leaf callback.
- `LeafNestedDefault` (negative): a forbidden call in the default of a closure written in a leaf callback is no call of the callback.
- `Read*` (uncertifiable): a parameter called in the default of a closure the helper builds is a call that may be made (`calls_parameter_in_parameter_list`); `ReadBodyClosure*` are controls, silent as before.
- `ForEachDefault`, `IifeDefault`, `PatternDefault` (uncertifiable): an anonymous callback's own list; its invoker may supply the argument (`forEach` its index, `map` the element), so a write there is no violation. `IifeDefault` omits it and runs: a known miss.
- `Owner.tsx`, SC4001 (uncertifiable, never `missing-owner` violations): a call in a closure's, method's, constructor's or module function's list (cleanup, effect, context read, a helper reached only from a default) in a handler or timer, never called or called with the argument (`oracle-behaviour-parameter-list-owner-missing-owner`). Such a call runs in its function's activation, not the module's or the handler's (`list_site` edges are `MayBeUnowned` roots). `HandlerCallsOmitted` runs it unowned: a known miss. `BodyCallsOmitted`, `ComponentOwnDefault` are owned and silent; `HandlerBody`, `LeafBody` stay violations.
- `LeafCallbackOwnDefault`: rc.17 calls an `onSettled` callback with no arguments, so a default in its own list runs in the leaf scope (`oracle-parameter-list-owner-leaf`): uncertifiable (was silent).

Stub: `createSignal`, `createMemo`, `onCleanup`, `onSettled`, `createEffect`, context of rc.17.
