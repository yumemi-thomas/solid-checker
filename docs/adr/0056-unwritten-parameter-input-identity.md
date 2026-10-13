# Unwritten parameter input identity

Status: implemented and measured in scoped Table/Pacer certification.

Protocol 39 adds `unwrittenParameters` to the implementation transcript. Each
positive row names an exact parameter slot and declaration location. The
producer reuses its returned-parameter identity proof, rejecting reassignment
(including nested writes), defaults, rest, destructuring, duplicate names,
arguments/eval exposure, async functions and generators. An absent row grants
nothing.

Emission also requires the selected signature's exact parameter declaration and
the demand's source file to match. An imported or aliased implementation may
legitimately have different coordinates; it retains its existing transcript
without this optional premise. The consumer continues rejecting malformed rows.

The consumer validates the row against the implementation signature and accepts
it only for a read operation's root parameter input with no asserted callability
or member path. It additionally requires a reachable, direct, noncaptured,
nonalias property-use record for that exact slot. The witness records both the
declaration and use locations. Existing archive, implementation, resolution,
producer and witness-root bindings authenticate these facts in the receipt.

This proves which value the input denotes. It does not establish a member's
existence, callability, closed behavior, or a conditional branch. Nested member
paths and callable/non-callable demands keep their existing proof rules.

The motivating measurement is `@tanstack/store@0.11.1 shallow`, reached while
recovering Table and Pacer entrypoints. Its generic second parameter remains
open in the declarations but has an affirmative unwritten source identity.
This is not permission to infer runtime behavior from the generic type.

The [scoped measurement](../package-contract-v2/phase21/2026-09-07-unwritten-parameter-recovery.md)
records 15 newly accepted artifact cases with original selections preserved.

## Async binding refinement

The September 9 refinement separates lexical binding identity from returned
value identity. Plain and async implementations may now supply this binding
premise; async functions still cannot supply an unwrapped returned-parameter
premise. All write, declaration, default/rest, arguments/eval and generator
exclusions remain. This changes no wire field or receipt format. Matching
producer/client pins bind the implementation together.

The [async investigation](../package-contract-v2/phase21/2026-09-09-async-parameter-binding.md)
records the tests and recovery of diagnostics `./playwright`, with the three
previous accepted main documents preserved byte-for-byte.
