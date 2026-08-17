# v1/no-array-handlers

`SC8007` · **error** · violation

A conventional `onEvent` prop on a DOM element receives an array- or
tuple-shaped value.

## What it does

Checks conventional `onEvent` props on lowercase JSX elements. It asks the
compiler to classify the value's type (`arrayShape`), so an imported or aliased
tuple is recognized however it is spelled; without that classification it
conservatively recognizes local array literals and their bindings.

The `on:*` namespaced form is **not** checked. Solid types `onEvent` as
`EventHandlerUnion = EHandler | BoundEventHandler`, where `BoundEventHandler` is
an interface with members `0` and `1` — so a `[handler, data]` tuple is legal
per Solid's own types and only this rule can object to it. `on:event` is typed
`EventHandlerWithOptionsUnion = EHandler | EventHandlerWithOptions`, which has no
bound-handler arm at all, so every array and tuple there is already `TS2322` and
reporting it again would duplicate the type checker.

## Why is this bad?

Solid supports the delegated `[handler, data]` shorthand, and `BoundEventHandler`
types its first member as `(data: any, ...e) => void` — `any`, so the data the
handler receives is never checked against the data the tuple carries. The pair
type-checks and then fails when the event is dispatched. The checker therefore
treats this as a type-safety boundary, not merely a style preference, and it is
the only thing that can: the tuple is legal per Solid's own types.

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

## Known imprecision

A **plain array** on `onEvent` — `X[]`, `Array<X>`, `ReadonlyArray<X>`, `any[]`,
`unknown[]` — has no `0`/`1` members either, so it is also already `TS2322`
against the real `solid-js` typings, and this rule's report there duplicates the
type checker. Narrowing it needs a finer fact than `arrayShape` provides today:
the condition that is genuinely this rule's is "a tuple whose first element is
callable", which is what `BoundEventHandler` accepts and `tsc` therefore permits.
Tracked in [docs/precision-backlog.md](../../precision-backlog.md).

## Configuration

For a project that deliberately accepts the tuple tradeoff, disable only this
rule with `{ "v1/no-array-handlers": { "enabled": false } }` in the project
rule-options document.
