# v1/no-array-handlers

`SC8007` · **error** · violation

A conventional `onEvent` prop on a DOM element receives a `[handler, data]`
bound-handler pair.

## What it does

Checks conventional `onEvent` props on lowercase JSX elements, and reports
exactly the values Solid's own types accept there: a tuple with both numbered
slots whose first slot is callable.

That set is the rule's because it is the set `tsc` permits. `onEvent` is typed
`EventHandlerUnion = EHandler | BoundEventHandler`, and `BoundEventHandler` is
an interface with members `0` and `1` whose `0` must be callable. A plain array
has no numbered members, a tuple with a non-callable first slot fails at element
0, and a one-slot tuple has no `1` — every one of those is already `TS2322`, so
the rule stays out of them.

The compiler decides tupleness (`tupleShape`), so an imported or aliased tuple is
recognized however it is spelled. When no type constrains the attribute at all —
a project whose JSX typings are permissive, where `tsc` says nothing — the rule
falls back to recognizing local array literals and their bindings, because there
it is the only thing that can speak. A cast vouches for the value, as upstream
also honours.

The `on:*` namespaced form is **not** checked. Solid types `onEvent` as
`EventHandlerUnion = EHandler | BoundEventHandler`, where `BoundEventHandler` is
an interface with members `0` and `1` — so a `[handler, data]` tuple is legal
per Solid's own types and only this rule can object to it. `on:event` is typed
`EventHandlerWithOptionsUnion = EHandler | EventHandlerWithOptions`, which has no
bound-handler arm at all, so every array and tuple there is already `TS2322` and
reporting it again would duplicate the type checker.

## Why is this bad?

`BoundEventHandler` types its first member as `(data: any, ...e) => void` —
`any`, so the data the handler receives is never checked against the data the
tuple carries. The pair type-checks and then fails when the event is dispatched.
That unchecked seam is the whole finding, and this rule is the only thing that
can report it: everything TypeScript *can* check about the pair, it already
does.

## Examples

Incorrect:

```tsx
type SaveHandler = [(data: Record, event: MouseEvent) => void, Record];
const click: SaveHandler = [save, record];
<button onClick={click}>Save</button>
```

The alias is the point: `click` renders as `SaveHandler`, so nothing about its
spelling reveals the tuple. `tsc` accepts all of this.

Correct:

```tsx
<button onClick={(event) => save(record, event)}>Save</button>
```

## How to fix

Pass a plain function whose parameters and captured data TypeScript can check.
If a tuple abstraction is essential, wrap it behind a function at the JSX
boundary. The rule does not rewrite handlers automatically because it cannot
infer the intended handler signature.

## Configuration

For a project that deliberately accepts the tuple tradeoff, disable only this
rule with `{ "v1/no-array-handlers": { "enabled": false } }` in the project
rule-options document.
