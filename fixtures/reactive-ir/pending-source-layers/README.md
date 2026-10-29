# pending-source-layers

**Claim (ADR 0511).** ADR 0510 proves a read of a memo the mounting parent hands over as an exact prop (`props.data()`) only directly in the apply. The read is now placed through the apply's in-place layers that `runs_nothing_before_in_layers` walks: an IIFE (`PropIife`), a named closure called once (`PropClosure`) and a project helper that calls its callback first (`PropHelper`, and the variant `helper-effect` for a user effect under ADR 0505). Each first run meets the memo pending, so `pending-async-unsuspendable-read` (SC5001) is the claim.

A memo an inert hook of another module creates (`hooks.ts`) and the body takes apart before the effect is pending too (`HookDestructured`; variant `hook-member-effect`, a user effect reading `vals.data()`).

A child of `<Loading fallback={0}>` renders during the parent's tree (`LoadingHolder`; variant `loading-effect`, a user effect under a static tree).

Uncertifiable: `LoadingOn` (an `on` list), variant `loading-tracked-effect` (the tree reads the memo, the boundary shows its fallback and holds the user effect), `HookAfterEffect` (the hook is called after the effect), `HookRunsCode` (the hook runs code after the creation), `PropKept` (the helper keeps the callback and never calls it) and `PropThrowFirst` (the IIFE throws before the read).

Stubs: `node_modules`, `jsx.d.ts` and `tsconfig.json` copied from `pending-source-first-run` (rc.17 published declarations); `Loading` added to `solid-js` from `types/client/flow.d.ts:273`; `package.json` makes the program closed.
