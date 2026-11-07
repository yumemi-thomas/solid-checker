# after-await-rowless-ends

**Claim (ADRs 0870-0873).** SC1002 `reactive-read-after-await` gives every read that may run past an await of a tracked async computation a row, where no dominance fact lists it:

- B1 (ADR 0870): an await a branch may skip (`if`, `&&`, `switch`, a destructuring default's value), including a store read in a closure run there. A read before the await, or in the arm without it, has no row.
- B3 (ADR 0871): a helper shared by two computations, one of which read the source first.
- B4 (ADR 0872): a helper whose first segment runs after the computation's first await. Called first, its read before its own await is a dependency (no row).
- B2 (ADR 0873): an async compute a project wrapper hands to `createMemo`. A wrapper that calls its parameter in place (`now(f) { f(); }`) hands over no compute: the read runs on a later task with no tracking claim, as an async IIFE there does.

B1's rows are uncertifiable (`handed-read-after-await`): position cannot tell an arm that leaves from one that falls through. B3, B4 and B2's reads are violations (Chrome warns on the fuzz shapes `aw-shared-after`, `aw-helper-late`, `aw-wrapper`). Stub reduced from rc.21's published typings.
