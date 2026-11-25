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
creates the source too, at module scope (ADR 0636). One run that throws is the claim, so one entry
on that stack is enough (ADR 1216): a direct, straight-line call of an unrebound named helper written
outside JSX, whatever its other uses (a handler, a map), and a primitive that runs its callback during
its call (`flush(fn)`, a memo's or an effect's compute with no `lazy` option). The write and every such call must run
whenever their function runs (ADR 1216 part 2): a completion site (no branch, loop, `try`, logical or earlier exit around
it), in no JSX element but an intrinsic one. A write under a branch, after an early return or in a component's children
is uncertifiable.

Proven too (ADR 1175): the setter call is in the first run of an effect's apply that comes
before the answer. The apply half is ADR 0504's (the call on every completion of the apply's
first run, after no other call of it; an effect with no options and a compute that is one
primitive literal, created on every completion of a component a module-scope `render` mounts),
and the source half is ADR 0505's own-body arm with the optimistic creation in place of the
memo: a `const` tuple of the same body, a literal `async` compute, no `loadingValue`, nothing
run between the creation and the effect. A `createEffect` also needs ADR 0505's mount flush (a
static tree, a body that runs nothing else, a compute that is one primitive literal); a
`createRenderEffect` runs its apply at creation. The answer lands a microtask later at the
earliest, while `render` flushes synchronously. A guarded write, a `loadingValue`, an apply
whose compute reads the source (it waits for the answer), a deferred effect, a write handed to
a timer and a namespace import of the primitives stay uncertifiable.

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
