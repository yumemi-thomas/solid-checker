# A callback that escapes into a returned callable is deferred, not lost

`direct` invokes its callback on the caller's own stack, and its summary is the
only `same-stack` one here. Directly returned callback bodies retain `queued`,
count `0..many`: the consumer may never call them or may call them repeatedly.

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

`debounce`, `decorated`, and `throughIdentity` keep their existing summary.
The two `nestedThrough*` cases remain unknown after ADR 0015: their callback
calls belong to nested helpers, whose execution the return inference does not
prove. A returned ancestor's lexical containment was the old, invalid premise;
the never-invoked control in `returned-callback-descendant` disproves it.
No empty closed callback domain replaces either omitted positive operation.
