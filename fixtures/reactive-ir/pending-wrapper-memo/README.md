# pending-wrapper-memo

**Claim (ADR 1102).** A call of a local function that reads a pending async memo (`wrapUse`, ADR 0510), made by a memo's compute, or by exact in-place layers that compute runs (ADR 1096's `computation_layer_call`), is the memo's own suspending read. A non-render computation raises no ASYNC_OUTSIDE_LOADING_BOUNDARY, and whoever reads the memo is a site of its own. `reactive_analysis.rs` synthesized an opaque SC5003 row at such a call (the ADR 0771 attributed read and the placed open row). It no longer does; a direct compute read already had no such row.

Clean: `Direct`, `Closure`, `Iife`.

Twins, not ended by this: `RenderCompute` (a render effect's compute is a render) and `ApplyCall` (an effect's apply is untracked).

Stubs: `node_modules`, `jsx.d.ts` and `tsconfig.json` copied from `pending-source-layers`; `package.json` makes the program closed.
