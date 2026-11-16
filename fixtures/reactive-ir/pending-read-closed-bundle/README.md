# pending-read-closed-bundle

**Claim (ADR 1009).** ADR 0667 opens a possibly async read written in a function that is an object member, because nothing says where such a function runs. A function handed from a closed bundle has a place: it is the value of a plain data property of a `const` object literal, the object is not exported, and every reference to the object is a non-computed read of that one key handed whole to a primitive, never a write. Then the primitive's callback position is the only thing that runs it, and a read in it (or in a closure, an IIFE or a helper's callback written inside it) takes that position's role.

Violations (SC5001, PENDING_ASYNC_FORBIDDEN_SCOPE): `Settled`, `Tracked`, `NestedClosure`, `NestedLiteral`, `TwoProperties`.

Obligations (`invocation-context-unproven`): `ObjectPassed`, `PropertyWritten`, `ComputedKey`, `LetObject`, `Method`, `MemberAlsoCalled`, `MemberAliased`, `NestedBundle`. Each has a key of its own: a write of a key anywhere in the file opens every member of that name.

No violation: `TrackedCompute`, the same bundle handed to a memo's compute, which may suspend.

Runtime: rc.21 Chrome, oracle case `fixtures/runtime-oracle/pending-read-closed-bundle`. Premises: the closed program is the `private` manifest.
