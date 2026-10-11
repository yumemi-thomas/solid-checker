# Exact accessor provenance

Cross-file generic factories return a signal slot, memo, or a data-property
closure that reads it, at one direct member level only. A default-less context transports one exact provider
object/closure through useContext and wrappers. Reads must be claimed at their
actual body/eager-initializer/effect-apply invocation, retaining source evidence.
Tracked memo/JSX calls and plain members stay clean. Mutable/replaced/escaped
receivers, computed/spread members, multiple factory returns, defaulted contexts
and two providers must not acquire a guessed violation.
An async helper's after-await read must not borrow the effect apply role of its
initial entry; that detached continuation remains uncertifiable.

PropObjectNotes and PropObjectHeader retain JSX-returning callbacks in a prop
object. The cross-file consumers invoke them in a JSX attribute and through a
helper in a JSX expression; their definitions must acquire no violation from
their enclosing component or from returning JSX. Prop-object invocation
projection is not claimed: the current exported-consumer dispatch obligation
may remain. ObjectConsumerBody supplies the positive: an exact factory returns
the notes closure and the consumer invokes decor.notes() in its own body,
where SC1001 must retain the original signal read. Object construction itself
never proves callback invocation. Unprojected definitions retain the ordinary
callee-callback-timing SC1001 obligation. RetainedOptionCallback mirrors an
object-literal option callback passed to a local factory that retains it: the
read through animate must stay uncertifiable, even though the returned factory
closure is invoked from an event. Definition-site obligations are never suppressed
by a proven invocation: another factory instance or opaque consumer can still
invoke the same callable.
ObjectConsumerTracked separately pins an exact member invocation in tracked JSX:
the consumer stays non-violating while its definition retains an obligation.

Round-4 controls refuse nested member projection (including unmutated nesting,
alias/helper mutation and opaque helpers), member aliases and for-of/for-in
writes. Receiver bindings may only be used in direct member invocations, apart
from the exact provider-value handoff. Factories must return the literal itself,
not an alias. ReturnedObjectAlias, ContextEscaped, ContextMemberAlias and
ExportedReceiver pin those return/receiver refusals. Async members and implementations absent from the producer's
function universe are refused; no synthetic nodes are admitted. Warm.tsx and
warm-factory.ts pin the caller-only `other` to `value` edit with a named helper
inside an anonymous returned arrow; the process test compares retained and cold
IR with retained factory syntax/compiler facts. Graph reuse follows the ordinary
source-discovery invalidation policy. Callback-parameter cases preserve the
ordinary source/argument obligations without synthetic parameter inference.
An exact callable missing from that universe keeps an SC9012 dispatch obligation
when its body contains calls/member reads; an inert literal needs no such proof.
MixedInstances and MixedSameInstance share makeDecor's definition obligation.

AsyncMemberPreference and RetainedMemberPreference must gain no SC8015 from a
deferred or unproven-timing read. SynchronousMemberPreference and DirectMemoPreference
pin direct synchronous dependencies. The nested/loop factories explicitly return
unbranded callable types, so legacy SourceAccessor type discovery cannot disguise
a refused transport. This matches the review examples' published-type-legal
shape; it does not loosen the primitive declarations.

The reduced stub preserves the published rc.13 claim-bearing signal/memo,
effect and context signatures (including effect cleanup-or-void returns).
Renderer and unused options are scaffolding. The independent published-typing
check replaces the stub with the retained real rc.13 packages, with strict
noEmit and skipLibCheck for prerelease declaration internals.
