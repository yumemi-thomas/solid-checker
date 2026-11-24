# assigned-member-entries

**Claim (ADR 1207).** A member a function's own code assigns once and then calls runs the assigned function at each call (`write_roles::settled_local_members`): the receiver is an immutable, unexported `const` bound to a closed object literal, declared in a function, every reference of it in that function's own code the object of a non-computed member; each key has one plain `=` of a function literal (an arrow, or a function that names no receiver), and that write dominates every call of the key (written before it, every conditional arm around the write holds the call, no `finally` between, the write in no call's argument, no `switch` or labeled jump in the function). Those calls are the assigned function's entries (`member_call_entries`, ADR 1205), and the package hand-off walk takes them as its exact target (`settled_local_member_target`). The program steps an action, runs an effect's apply and opens a leaf scope, so no program-wide reason ends any site.

Clean: `AssignedLoop` (write and optional call in a counted loop's later pass of an async memo), `AssignedAfterAwait` (after an await, the call in a branch).

Uncertifiable (fail closed): `TwoWrites`, `ConditionalWrite`, `CapturedCall` (a call in a closure), `WrittenThroughParameter` (fuzz `handoff-assign-param`), `BeforeAwait`.

Stub: `member-call-entries`'s. Chrome: `runtime-oracle/assigned-member-entries`.
