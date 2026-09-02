# A shadowed constructor is not the built-in one

`new ResizeObserver(callback)` looks exactly like the built-in whose entry in
the runtime-semantics table says "invokes argument 0, deferred". Here the name
resolves to a class declared in the same module, so the entry must not apply
and `shadowedResizeObserver` carries no callback operation at all.

This is the constructor half of the control; `runtime-semantics` carries the
function half (`shadowedString`, `shadowedQueueMicrotask`) alongside the
positive table. Matching a built-in by spelling instead of by resolved
declaration would publish a deferred-invocation claim about a body that
provably never invokes anything.
