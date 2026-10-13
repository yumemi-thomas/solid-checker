# Original receiver evaluated before assignment

Marker's `makeSearchRegex` first executes `search.trim()` on its caller input,
then assigns the transformed string back to `search`. The whole-function
unwritten premise refuses this real read. Protocol 41 extends the affirmative
initial-read fact to the leading receiver of the RHS call chain in the first
plain parameter assignment after the already supported opening const prefix.

The producer follows only identifiers, static property access and callee
expressions. It never follows arguments, computed access, assignments, comma
expressions or later statements. The LHS must be a plain parameter identifier.
The existing exclusions for nested callables, parameter defaults/rest,
arguments/eval and non-plain completion remain. The exact receiver is evaluated
before the assignment stores its result; nothing is inferred about later uses.

The verifier binds the initial property-use location to a reachable uncaptured
call starting at that exact receiver, whose callee parameter has the exact
demanded slot and member path. An earlier property getter or different method
cannot justify a later method call. This proves receiver origin only; all shape,
callability, occurrence and cardinality demands still run independently.
Untyped JavaScript may have no resolved method target symbol, while still
providing an exact lexical callee-parameter path. The latter is the positive
origin premise; absence of a target supplies no behavioral authority.

No contract, receipt or trust format changes. Producer and consumer move in
lockstep to protocol 41 and pin the modified schema digest. Focused tests cover
assignment ordering, chained calls, argument writes, overwritten receivers,
computed/comma expressions, and exact call/path/location mutations. A packed
archive test checks the Marker-shaped positive and rejects a later different
method, a replaced receiver, and an earlier getter followed by a replacement.

The [seven-probe measurement](../package-contract-v2/phase21/2026-09-08-initial-receiver-recovery.md)
recovers all three Marker roots and three complete rows, preserving all ten
Motion/Utils selections and their claims. Both ordinary-CLI negative controls
still refuse. Focused producer/native/verifier tests and full `make verify`
pass (exit 0, TOTAL 243.12s, no failed-step marker). The full corpus was not
rerun for this slice; its protocol-40 aggregate remains the latest full
measurement. No denominator changed.
