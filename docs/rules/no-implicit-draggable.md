# no-implicit-draggable

`SC8019` · **error** · violation

The `draggable` HTML attribute is given literal `false` on an element that is
draggable by default.

## What it does

`draggable` is an enumerated attribute whose valid values are the strings
`"true"` and `"false"`; it is not an HTML boolean attribute. Solid 2.0 removes
the attribute for literal `false`, selecting the `auto` state. On `img` and
linked `a` elements, `auto` means draggable, reversing the written intent.

The published Solid 2 JSX types already reject shorthand `draggable` and
`draggable={true}` with TS2322. The checker deliberately leaves those spellings
to TypeScript even though the runtime would serialize them presence-only.

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
<img draggable={false} />                      // Incorrect: removal -> draggable auto
<img draggable="true" />                       // Correct static value
<img draggable={canDrag() ? "true" : "false"} /> // Correct dynamic value
```

A dynamic *boolean* expression has the same runtime shape, but the rule only
proves the literal-false/default-draggable case; map booleans to strings.

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

An `<a>` is a proven violation only when its final source-order `href` write is
proven present — a JSX string (`href="/x"`) or the bare spelling (`href`), the
two forms that always emit the attribute. A later static `href` overrides an
earlier spread and remains proven. Ambiguous final writes are reported as
**uncertifiable**, not silently treated as safe and not promoted to violations:

- `href` carried or overwritten by a spread may or may not be there at all;
- `href={expr}` is removed by Solid when `expr` is nullish, and an `<a>` with
  no `href` attribute is *not* draggable by default.

An anchor with no direct `href` and no spread is proven href-free and stays
silent because its `auto` state is not draggable.

## Known boundary

The rule only checks intrinsic elements. Explicit string and non-literal
expression values, component props, and namespaced attributes are unaffected.
