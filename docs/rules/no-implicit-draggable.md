# no-implicit-draggable

`SC8019` · **error** · violation

The `draggable` HTML attribute uses JSX boolean shorthand.

## What it does

`draggable` is an enumerated attribute whose valid values are the strings
`"true"` and `"false"`; it is not an HTML boolean attribute. The shorthand
form emits an empty value, which selects the invalid/default `auto` state.

```tsx
<img draggable />              // Incorrect: draggable=""
<img draggable="true" />       // Correct static value
<img draggable={canDrag()} />  // Correct dynamic value
```

The rule only flags shorthand on intrinsic elements. Explicit string and
expression values, component props, and namespaced attributes are unaffected.
