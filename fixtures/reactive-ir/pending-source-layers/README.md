# pending-source-layers

**Claim (ADR 0511).** ADR 0510 proves a read of a memo the mounting parent hands over as an exact prop (`props.data()`) only directly in the apply. The read is now placed through the apply's in-place layers that `runs_nothing_before_in_layers` walks: an IIFE (`PropIife`), a named closure called once (`PropClosure`) and a project helper that calls its callback first (`PropHelper`, and the variant `helper-effect` for a user effect under ADR 0505). Each first run meets the memo pending, so `pending-async-unsuspendable-read` (SC5001) is the claim.

Uncertifiable: `PropKept` (the helper keeps the callback and never calls it) and `PropThrowFirst` (the IIFE throws before the read).

Stubs: `node_modules`, `jsx.d.ts` and `tsconfig.json` copied from `pending-source-first-run` (rc.17 published declarations); `package.json` makes the program closed.
