# parameter-list-owner

**Claim (ADR 0454).** A site in a function's parameter list (a default value, a pattern, a function written in either) belongs to the function whose list it is and runs only when that function is called without the argument. It is no code of the function or helper around it.

- `*NeverCalled` (negatives, uncertifiable): a helper builds a closure whose default calls the callback (also through `buildForward`, in a branch, returned directly, destructured, as a declaration, through an IIFE) and never calls it; rc.17 throws nothing (`oracle-behaviour-parameter-list-owner`). `SC2001` was a violation.
- `NestedCalledInHandler`, `NestedCalledOmitted`, `NestedCalledSupplied`, `OwnDefaultOmitted` (uncertifiable): the closure is called in a handler, in the memo without the argument (it runs: a known miss, no call site is kept for a function whose list runs code, ADR 0339), with it, and the helper's own list. No proof that a caller omits the argument is built.
- `BodyCall`, `ReadBodyCall`, `LeafDirect` (positives, violations): a call in the helper's body, an accessor it reads in its body, a forbidden call directly in a leaf callback.
- `LeafNestedDefault` (negative): a forbidden call in the default of a closure written in a leaf callback is no call of the callback.
- `Read*` (uncertifiable): a parameter called in the default of a closure the helper builds is a call that may be made (`calls_parameter_in_parameter_list`); `ReadBodyClosure*` are controls, silent as before.
- `ForEachDefault`, `IifeDefault`, `PatternDefault` (uncertifiable): an anonymous callback's own list; its invoker may supply the argument (`forEach` its index, `map` the element), so a write there is no violation. `IifeDefault` omits it and runs: a known miss.

Stub: `createSignal`, `createMemo`, `onCleanup`, `onSettled` of rc.17.
