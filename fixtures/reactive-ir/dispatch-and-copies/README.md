# dispatch-and-copies

**Claim (ADR 0516).** A callback reached through a `const` copy or an object literal's member handed whole to a call, and a setter or action invoked through `Function.prototype` (`f.call(t, …)`, `f.bind(t)(…)`, `f.apply(t, [ … ])`) or a callee that always yields it (`(note(), f)(…)`), is judged as the direct form at the written call.

Violations: `LeafByCopy` (`pending-async-unsuspendable-read`: a leaf callback handed by a copy runs in the leaf scope); `CopyInTimer`, `SlotInTimer` (`missing-owner`: rc.17 runs a timer callback with no owner); `DispatchInMemo` (`no-write-in-pure-computation`, twice).

Unchanged: `LeafCopyAlsoClicked` (also run from a handler, so not the leaf's). Silent: `PrototypeDefault` (`{}` inherits `toString`, so the default never applies); `CopyInRoot` (the copy and the slot run owned under `createRoot` and `createMemo`); `DispatchInHandler` (writes and an action call in a handler).

Uncertifiable: `GeneratorHanded` (a scheduler calling a generator runs none of its body); `ApplyUnknown` (`.apply` with a value that may not be array-like throws before the call); `OrAssign` in the memo (`||=` may call the slot's old value; the handler's call is clean, every operand's call there being legal).

Stubs: `write-cleanup-handle-call`'s `solid-js.d.ts` and `node_modules` stubs (rc.17 surface, as recorded there).
