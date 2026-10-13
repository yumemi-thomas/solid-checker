# Returned-callable captures consumer (hand-stated)

createTicker retains argument 0 as callback; a call of its returned value
invokes that exact value once, inline, in the invoker's owner and tracking
context. createPanel retains the same value but may invoke it on an external
timer with no owner. Factory construction does neither read.

CapturedReadInBody, InvocationArgumentIsSeparate, ResourceCaptureRead and
OperationResultCaptureRead expect SC1001 proven violations at the returned
invocation. CapturedReadInJSX, CapturedReadInHandler, DeferredRead, Shadowed,
Discard, NestedLiteralIsNotTheCapture and both resource/result Tracked twins
expect no finding.
TwoCapturedInstances expects a proven SC1001 only at `reads()`, with `plain()`
clean: factory instances keep distinct captured values. WrappedReturnedInitializer
expects SC9012 at the wrapped result and invocation, with no SC1001 derived
through the cast. These round-2 cases have not been run through the checker.
Every other annotated branch expects reactive-dispatch-unresolved (SC9012),
uncertifiable, with no alias-, wrapper- or escape-derived proven violation.
UnknownCapture must stay uncertifiable even though its const identity is
exact. Both captured and returned onClick values are escapes in this
vocabulary; the old handler exemption survives only without captures.

The package manifest bytes, authorization import/closure metadata and dialect
stubs are unchanged from package-returned-callable-consumer. Declarations are
for this synthetic package, not reduced published primitives. The Solid stub
is byte-identical to the base fixture; its accessor brand is the positive read
premise. No accepted-contracts catalog and no generated snapshot is supplied.

UnknownOperationResultCapture
expects SC9012, with no guessed accessor read. Resource/result captures require
the graph's explicit same-resource read; resource kind alone proves nothing.
The authorization shape is copied from the base, with three new export records
using the same byte-identical package manifest and closure identities.
Model unit tests pin validation. The lead must authorize a copied fixture with a fresh
pinned debug build, run non-updating coverage, review exact findings, and then
add only this fixture's snapshot and gitignore lines.
