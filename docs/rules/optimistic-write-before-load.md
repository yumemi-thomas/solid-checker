# optimistic-write-before-load

`SC2009` · **error** · violation

An optimistic setter of an async source runs before the source's first answer.

## What it does

rc.21 refuses it: a `createOptimistic(async ...)` or `createOptimisticStore(async ...,
seed)` setter throws `OPTIMISTIC_WRITE_BEFORE_LOAD` while the source has no first answer,
ahead of the owned-scope guard, in any role. The seed does not count; a declared
`loadingValue` is an answer. Proven where the call runs on one stack with a `const` creation
with a literal `async` compute, entered after it and past no await: in the same function, or
in closures, helpers' callbacks, `untrack` or `createRoot` on that stack, in a handler that
creates the source too, at module scope (ADR 0636).

Anywhere else it is uncertifiable: a handler or action that did not create the source may run
before the answer lands or after it, and no fact tells which. So is a compute that is not a
literal `async` one, a source a hook creates, and a setter a `let` or `var` holds.

## How to fix

Keep pending items in a separate list until the source loads, disable the action until
it has, or declare a `loadingValue`.
