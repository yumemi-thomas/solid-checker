# A destructuring pattern is not an alias of the value it destructures

`const { onData } = props` binds `props.onData`. The callable-identity walk
(`direct_value_symbols` in `rust/crates/solid-reactive-ir/src/indexes.rs`) used
to resolve such a local through the binding's `initializer_identifier` rung —
the rung that makes `const alias = original` carry `original`'s identity — and
so answered `props` for a call written `onData(1)`.

That answer is not merely imprecise, it is a different value. The callback
derivation then found `props` in the enclosing function's parameter list and
published a `callbacks` row saying **parameter 0 is invoked**: the props object
itself, called as a function. `@solidjs/router`'s `Navigate` published exactly
that row for

```js
const { href, state } = props;
const path = typeof href === "function" ? href({ navigate, location }) : href;
```

and certification then refused it, correctly — no compiler fact can confirm
that a props object is invoked, because the call is on a member. The claim, not
the verification, was wrong.

The cases:

- `Parameter` is the negative that keeps the derivation useful. `onData` really
  is the declared parameter, it really is invoked before the export returns,
  and the row survives with `arg: 0, path: []`.
- `ObjectPattern` is `Navigate`'s shape. It now publishes no callback row: the
  local resolves to its own symbol, so nothing claims the parameter is invoked.
- `MemberAlias` is the same runtime meaning written without a pattern
  (`const onData = props.onData`). It never published a row, and pins that the
  pattern form now agrees with it rather than the other way round.
- `ArrayPattern` is the tuple form of the same mistake (`const [onData] =
  handlers` is `handlers[0]`, not `handlers`), which the identifier-initializer
  rung reached the same way.

## What this does not yet prove

The honest positive claim for `ObjectPattern`/`ArrayPattern` is a callback
bound to the *member* — `arg: 0, path: ["onData"]`. `ContractCallback` carries
only a parameter index, so the semantic model cannot express it, and the
generator publishes silence (the `callbacks` domain stays open and a consumer
fails closed) rather than a false row. Recorded in docs/precision-backlog.md.
