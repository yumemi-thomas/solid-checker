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

## `draggable={false}`

The runtime serializes a literal `false` by removing the attribute (the
removal half of the same rc.0 probe), and removal selects `auto`:

- On draggable-by-default elements — `<img>`, and `<a>` with an `href` —
  `auto` **is draggable**, so `draggable={false}` silently re-enables the
  dragging it was written to disable. The rule flags these; write
  `draggable="false"`, the only spelling that selects the disabled state
  there.
- On every other element `auto` is not draggable, so removal matches the
  author's intent and stays clean.

An `<a>` counts as draggable-by-default only when its `href` is **proven
present** — a JSX string (`href="/x"`) or the bare spelling (`href`), the two
forms that always emit the attribute. Anything else stays clean rather than
guessed:

- `href` carried by a spread may or may not be there at all;
- `href={expr}` is removed by Solid when `expr` is nullish, and an `<a>` with
  no `href` attribute is *not* draggable by default — so `draggable={false}`
  on it is correct code, and reporting it would be a false positive.

## Known boundary

The rule only checks intrinsic elements. Explicit string and non-literal
expression values, component props, and namespaced attributes are unaffected.
