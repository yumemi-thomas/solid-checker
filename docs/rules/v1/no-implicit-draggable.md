# v1/no-implicit-draggable

`SC8019` · **error** · violation

Rejects JSX boolean shorthand for the intrinsic `draggable` attribute.
`draggable` is an HTML enumerated attribute, not a boolean attribute: its valid
values are the strings `"true"` and `"false"`. The shorthand form serializes
an empty value and selects the invalid/default `auto` state.

```tsx
<img draggable />              // Incorrect: draggable=""
<img draggable="true" />       // Correct static value
<img draggable={canDrag()} />  // Correct dynamic value
```

Component props, explicit values, and namespaced attributes are not affected.
Use a static string when the element is always draggable, or an expression
when the state changes. More detail is available under the version-independent
[no-implicit-draggable](../no-implicit-draggable.md) rule.
