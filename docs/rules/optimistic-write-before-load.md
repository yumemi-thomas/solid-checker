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

A setter handed to a component as a prop value (`<K s={setO} />`, a branch of `?:`, `&&`,
`||`, `??`, a spread, a context provider's `value`) is uncertifiable at the hand-off, blocker
`optimistic-reach-unresolved` (ADR 0712): the component's `props.s(…)` is a call no row
places, since prop value flow is not followed. Chrome throws when the component runs it before
the answer. A setter a hook or context carries to a call is an obligation there too.
A call whose reach the walk cannot finish (512 nodes) is uncertifiable too (ADR 0752).

## How to fix

Keep pending items in a separate list until the source loads, disable the action until
it has, or declare a `loadingValue`.

## Endings by code (ADR 0968)

A setter call is accounted for, for this rule's Chrome code, only by an ending that answers that code: a row of its own table, an SC9012, the compiler's discard, a finding of a rule the dialect says claims the code, or a named silence that names it. A row of another table or a finding of another rule at the same place answers another code and ends nothing here, so this rule's own uncertifiable row (`optimistic-write-unaccounted`) stands beside it: never a violation, and it yields to any row of the rule that stands. A site whose rule claims no code here, or whose code has no policy in the dialect, is not ended by what stands at it.
