# optimistic-write-before-load

`SC2009` · **error** · violation

An optimistic setter of an async source runs before the source's first answer.

## What it does

rc.21 refuses it: a `createOptimistic(async ...)` or `createOptimisticStore(async ...,
seed)` setter throws `OPTIMISTIC_WRITE_BEFORE_LOAD` while the source has no first answer,
ahead of the owned-scope guard, and the seed does not count. A declared `loadingValue` is
an answer. Proven in the pass that creates the source: the setter runs in the function
holding a creation with a literal `async` compute, after it, in a component body or
module scope. Anywhere else it is uncertifiable (ADR 0636): a handler or action may run
before the answer lands or after it, and no fact tells which.

## How to fix

Keep pending items in a separate list until the source loads, disable the action until
it has, or declare a `loadingValue`.
