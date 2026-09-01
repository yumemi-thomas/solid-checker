# A callback that escapes into a returned callable is deferred, not lost

`direct` invokes its callback on the caller's own stack, and its summary is the
only `same-stack` one here. Every other export hands the callback to a closure
it *returns*, so the invocation happens on some later call of that closure --
`queued`, count `0..many`, because the consumer may never call it or may call
it repeatedly.

The point of the fixture is that the deferral survives each way the closure can
leave the function:

- `debounce` returns the closure directly, and the timer adds a second hop;
- `decorated` returns `Object.assign(wrapper, { clear() {} })` -- the returned
  value is the *same* function identity with properties attached, so descending
  it is the only way the callback edge is kept;
- `throughIdentity` and `nestedThroughIdentity` return it through a local
  identity helper;
- `nestedThroughCallable` returns it through a helper that re-wraps it in a
  `.call` forwarder and then mutates a property on the result.

All five share one summary. A regression that stopped descending returned
callables would not refuse them -- it would silently publish "invokes no
caller-supplied callback", which is a false negative claim, so silence here is
not a safe failure and has to be pinned.
