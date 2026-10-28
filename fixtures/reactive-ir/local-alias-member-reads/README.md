# local-alias-member-reads

**Claim (ADR 0481).** A static member read `thing.key`, where `thing` is a `const` holding a call of a `const` arrow whose expression body is an object literal of static data properties (ADR 0480's arrows) and every reference to `thing` is such a read (none written, deleted, optional or passed), is the `const` the literal's `key` names, for setter and action calls (`callee_symbol`).

Violations: a signal write (`no-write-in-pure-computation`) and an action call (`action-called-in-owned-scope`) in a memo through the member. Clean: the write in a handler. Not an alias: a holder handed to `console.log` (its writes stay as before). A store written through such a member is not followed yet (docs/open-work.md). Stubs as in `local-alias-bindings`.
