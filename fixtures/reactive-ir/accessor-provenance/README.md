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
deferred or unproven-timing read. Round 5 also withholds SynchronousMemberPreference
and DirectMemoPreference: every transported origin is excluded from preference
dependencies, even through an ordinary summary hop. AsyncHelperPreference pins
an after-await read of a transported member through an ordinary async helper.
SynchronousHelperPreference and DirectAccessorPreference retain the pre-existing
non-transported synchronous preference positives. The nested/loop factories explicitly return
unbranded callable types, so legacy SourceAccessor type discovery cannot disguise
a refused transport. This matches the review examples' published-type-legal
shape; it does not loosen the primitive declarations.

The reduced stub preserves the published rc.13 claim-bearing signal/memo,
effect and context signatures (including effect cleanup-or-void returns).
Renderer and unused options are scaffolding. The independent published-typing
check replaces the stub with the retained real rc.13 packages, with strict
noEmit and skipLibCheck for prerelease declaration internals.

ContextStability joins exact context-derived receiver bindings by context identity.
Mutation through a second wrapper result, cross-file direct useContext mutation,
opaque escape through a second receiver and an escaped provider binding all
withhold transport for the entire context. Direct mutation through an unbound
useContext result also closes the whole context. The old single-receiver escape/alias
controls use independent contexts so they do not invalidate AccountBody's positive.
Definition-site obligations remain. Cache.tsx/cache-factory.ts/cache-helper.ts
pin an admitted named member arrow whose ordinary helper gains/loses a signal read
in another file, without changing the helper declaration or caller/factory spans.
Backend retained/cold tests exercise both directions of that summary edit and
both directions of the cross-file context receiver census edit.

ADR 0273 adds the explicit transported member stability premise. The receiver
census alone does not close shared-object stability, especially through wrapper
callbacks. One project-wide written-name index vetoes transported members on
any receiver's static assignment/update/destructuring/loop/delete target or
exact Object/Reflect mutation call. Unknown Object.assign sources veto all
members. Missing/unresolved/dependency/unsupported reflective/eval/HMR channels
are outside the premise; other receivers' nonliteral computed writes are too.
Vetoes withhold transport and keep ordinary obligations. Mutation controls use
distinct dead/stale names so this project retains its independent rows/state
positive expectations. The transported-member-stability projects pin the new
premise, three review examples, exact builtin identity and wildcard refusal.

The fixture is an application (`package.json` `"private": true`): an open
program closes no context's provider census (see `context-member-identity-open`).

ADR 0276: in `DbBody`, the provider's `cached` reads `initial()` as the right
operand of `??`, which runs only when `live()` is nullish. That read is not on
every completion of `cached`, so it is uncertifiable through `db`, while
`live` stays a violation.
