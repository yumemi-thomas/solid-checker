# no-implicit-draggable

`SC8019` · **error** · violation

The `draggable` HTML attribute is given JSX boolean shorthand or a literal
boolean `true`.

## What it does

`draggable` is an enumerated attribute whose valid values are the strings
`"true"` and `"false"`; it is not an HTML boolean attribute. Solid 2.0
serializes boolean attribute values presence-only ("Boolean literals
add/remove the attribute" — no `="true"` string), so both the shorthand and a
literal `true` render a bare `draggable`, which selects the invalid-value
default `auto` — the element's own default — instead of the `true` state.

Probed on `@solidjs/web@2.0.0-rc.0`, both render paths agree, so this is not
a hydration mismatch — it is the wrong state everywhere:

| JSX | SSR (`renderToString`) | Client (`setAttribute`) | HTML state |
| --- | --- | --- | --- |
| `draggable` | `<div draggable>` | `draggable=""` | `auto` |
| `draggable={true}` | `<div draggable>` | `draggable=""` | `auto` |
| `draggable={false}` | attribute absent | attribute removed | `auto` |
| `draggable="true"` | `draggable="true"` | `draggable="true"` | `true` |
| `draggable="false"` | `draggable="false"` | `draggable="false"` | `false` |

```tsx
<img draggable />                              // Incorrect: draggable -> auto
<img draggable={true} />                       // Incorrect: same presence-only serialization
<img draggable="true" />                       // Correct static value
<img draggable={canDrag() ? "true" : "false"} /> // Correct dynamic value
```

A dynamic *boolean* expression (`draggable={canDrag()}`) has the same defect
when it is truthy; the rule only proves the literal forms, so map booleans to
the strings as above.

## Known boundary

`draggable={false}` is deliberately not flagged: it removes the attribute,
which selects `auto`. On most elements `auto` means not draggable, matching
the intent — but on draggable-by-default elements (`<img>`, `<a href>`)
removal silently re-enables dragging where `draggable="false"` would disable
it.

The rule only checks intrinsic elements. Explicit string and non-literal
expression values, component props, and namespaced attributes are unaffected.
