# Fractional Indexing: original input through a local helper

The retained TanStack Solid DB probe in `2026-09-08-defined-input-full.json`
refuses Fractional Indexing 3.4.0's `generateKeyBetween` because a
parameter-rooted read lacks positive original-input identity. Fresh generation
against that report's exact installed package and lock selection confirms the
two proposed read inputs are parameter 0 `.slice` and parameter 1 `.slice`.
They are not reads of the defaulted `digits` parameter. The diagnostic output
is `/private/tmp/fractional-return-evidence-uvLOYt/inspection.json`; it is
an unaccepted proposal, not a certificate.

The published `src/index.js` first calls `validateOrderKey(a, digits,
intDigits)` and the corresponding helper for `b`, each under its non-null
guard. These calls precede the conditional swap of `a` and `b`.
`validateOrderKey` reads its own `key.slice(i.length)` after calling
`getIntegerPart`, which also reads `key.slice`. This supplies a candidate
local call chain to investigate without treating the post-swap bindings as
the original caller inputs.

The existing transcript's resolved call declaration and argument parameter
coordinates do not by themselves prove original value identity at that
transfer. The current initial-parameter-read evidence covers exact direct
property uses; it is not evidence about an argument passed to a helper.
Furthermore, its producer currently rejects an implementation with any
defaulted parameter, which this export has. Neither absence can be treated as
permission, and a helper's resolved declaration alone is not execution proof.

The focused producer regression
`TestLocalHelperArgumentBindingDoesNotEstablishOriginalValue` confirms this
boundary against live Type Facts: calls before and after a store, a sibling
default initializer that stores into the input, and a loop that stores after
each call all retain the exact local callee and argument binding coordinates.
None acquires an unwritten-input or direct original-member-read premise.
All three cases pass (0.063 seconds). These are negative controls for the
proposed composition, not new certification. In particular, skipping
defaulted sibling parameters during validation would be unsound when their
initializers replace the input before the function body begins.

The helper-origin regression's full verification passed with actual exit 0,
TOTAL 137.66 seconds and no failed-step marker. An initial attempt stopped at
Go formatting; the alignment-only correction preceded the passing run.

A bounded extension would need positive evidence for the exact argument
occurrence before any binding write, the exact local runtime callee and
parameter slot, and the helper's original-parameter member read. Both call
and read must satisfy the operation's execution floor, and all locations must
belong to the authenticated artifact. Defaulted siblings must not be used to
assert anything about their caller values. Shadowed or reassigned callees,
spreads, mutated helper parameters, writes before transfer, mismatched paths,
and unreachable calls must remain refused. Public protocol changes require
ownership coordination before implementation.

No proof rule or shared interface changed in this investigation. There are
zero new accepted cases or row transitions. TanStack Solid DB remains refused;
one possible complete row is an unmeasured ceiling, and later graph blockers
have not been established. The callback-retry full corpus completed against
unchanged production source and binaries: Until alone adds one complete row,
and this Fractional Indexing refusal remains open.
