# Creating an accessor to hand to the callback is not a read of it

`mapValue` creates a signal and calls `mapFn(1, getItem)` -- the accessor is
passed *out* to the caller's callback, never read by this package. The summary
records exactly one thing: `mapFn` (argument 0) is invoked `same-stack`.

The reads domain stays empty rather than carrying a source read, and the
`creates` domain is not closed, so the package makes no claim that it creates
nothing. A regression that treated the accessor argument as an uncaptured
source read would attribute a reactivity defect to a package whose only act is
to construct a value and pass it on.

The `node_modules/solid-js` stub is 1.x, so `createSignal` is resolved through
the v1 catalog.
