# pending-memo-layers

**Claim (ADR 1096).** A read of a pending async memo inside exact in-place layers that a memo's compute runs is that memo's tracked read: it suspends the memo, as the read written directly in the compute does. The layers are a named closure called once, an IIFE, or a helper that calls its callback in its body. `local_access.rs` `computation_layer_call` climbs with `climb_in_place_layers`, which crosses no primitive's callback. It requires an exact climb (no parameter list, no helper that also hands its callback on, no await, nothing withheld, no unmodelled slot, not the bound), and the outermost layer's call must be a tracked read of a computation's own callback that suspends. The read's row is placed at that call, and `pending_accounting` records `SuspendsInComputation` and `NotRenderEffect` for it.

Clean: `MemoClosure`, `MemoIife`, `MemoHelper`, `MemoClosureHelper`.

Twins: `ApplyClosure` (an effect's apply is untracked), `MemoUntrackClosure` (the closure under `untrack`), `MemoClosureKept` (the closure is also kept and called later). None of them is ended by this proof. A leaf owner's callback is refused too, as a direct read there is a violation (`async-unsuspendable-call-attributed`'s `TrackedWrapper` and `SettledWrapper` keep their SC5001 violations). `MemoClosureHanded` (a named closure handed to the helper as a value) stays uncertifiable: the climb follows no reference handed on.

Stubs: `node_modules`, `jsx.d.ts` and `tsconfig.json` copied from `pending-source-layers`; `package.json` makes the program closed.
