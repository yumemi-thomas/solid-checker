# Exact accessor provenance

Cross-file generic factories return a signal slot, memo, or a data-property
closure that reads it. A default-less context transports one exact provider
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
never proves callback invocation.

The reduced stub preserves the published rc.13 claim-bearing signal/memo,
effect and context signatures (including effect cleanup-or-void returns).
Renderer and unused options are scaffolding. The independent published-typing
check replaces the stub with the retained real rc.13 packages, with strict
noEmit and skipLibCheck for prerelease declaration internals.
