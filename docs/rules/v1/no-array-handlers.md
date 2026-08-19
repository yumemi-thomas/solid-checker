# v1/no-array-handlers

`SC8007` · **error** · violation or uncertifiable

A conventional `onEvent` prop on a DOM element receives a `[handler, data]`
bound-handler pair.

## What it does

Checks conventional `onEvent` props on lowercase JSX elements. It reports a
violation only when the runtime value is structurally proven to be an array,
and reports `uncertifiable` when a type-correct value may be either a plain
function, a bound pair, or absent.

That set is the rule's because it is the set `tsc` permits. `onEvent` is typed
`EventHandlerUnion = EHandler | BoundEventHandler`, and `BoundEventHandler` is
an interface whose `0` is `(data: any, ...e: Parameters<EHandler>) => void` and
whose `1` is `any`. A plain array has no numbered members, a tuple with a
non-callable first slot fails at element 0, a one-slot tuple has no `1`, and a
first slot requiring three arguments is not assignable to a two-argument
signature — every one of those is already `TS2322`, so the rule stays out of
them.

The compiler decides tupleness (`tupleShape`) and the complete runtime domain,
so aliases and unions are recognized however they are spelled. Type-only tuple
evidence without a runtime-presence proof is retained as an uncertifiable
obligation; inline arrays and immutable local array initializers prove the
violation. When no type constrains the attribute at all, the same structural
fallback applies.

Assertions do not vouch for runtime safety. The checker peels `as` and non-null
wrappers: an asserted array still reports, an asserted function stays clean,
and a hidden value whose arrayness cannot be established is uncertifiable.

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

For an uncertifiable value, narrow it to a plain function before the JSX
boundary or make the bound pair's runtime construction explicit.

## Configuration

For a project that deliberately accepts the tuple tradeoff, disable only this
rule with `{ "v1/no-array-handlers": { "enabled": false } }` in the project
rule-options document.
