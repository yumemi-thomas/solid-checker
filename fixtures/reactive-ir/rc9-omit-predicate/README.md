# rc9-omit-predicate

`@solidjs/signals@2.0.0-rc.9` adds a predicate form of `omit`:

```ts
omit<T>(props: T, hidden: (key: keyof T & (string | symbol)) => boolean): Partial<T>
```

(`dist/types/store/utils.d.ts:251`). The runtime selects it by
`keys.length === 1 && typeof keys[0] === "function"` (`dist/dev.js:4380`), so
only a two-argument call can carry one. When the predicate runs:

- **With `Proxy`** (every supported runtime; `SUPPORTS_PROXY`,
  `dist/dev-shared.js:273`) `omit` only stores it in the view it returns
  (`:4406`). The view's `get`/`has`/`ownKeys` traps and the merge/spread walks
  call it on every read of that view (`isHidden`, `:3495-3498`; `omitTraps`,
  `:4178-4241`), with nothing around the call touching the listener. Its reads
  subscribe whatever computation reads the view, and its writes run under that
  reader's owner.
- **Without `Proxy`** it runs once per own property name during the `omit`
  call (`:4408-4427`).

The dialect states exactly that and no more
(`Dialect::callback_runs_on_result_access`): the engine does not follow reads of
a returned view back to the call that made it, so code inside the predicate is
given no execution role (`ExecutionRole::Unknown`) and projection claims
nothing about it.

| case | before | after |
| --- | --- | --- |
| `InlinePredicateRead` — reads `mode()`, view read in tracked JSX | SC1001, "read directly in rendering function" | silent |
| `InlinePredicateWrite` — writes in the predicate | SC2001 in the component's owned scope | silent |
| `NamedPredicateRead` — the predicate passed by name | silent | silent |
| `BodyReadBesidePredicate` — a direct body read beside a predicate | SC1001 | SC1001 (control) |
| `KeyListForm` — `omit(props, "a")` | silent | silent |

Both "before" findings claimed the predicate runs in the component body. It
runs where `rest.b` is read, which here is tracked JSX.

The package-contract half of the same fact — a forwarded predicate is no longer
published as a value the export never invokes — is pinned by
`fixtures/package-contracts/rc9-callback-forms`.

`tsc --noEmit` is clean against this fixture's stubs and against the published
rc.9 typings. Against rc.3's typings every predicate call is TS2345, so no
rc.3-valid typed call is affected. `node_modules/solid-js/package.json` selects
the 2.0 dialect at `2.0.0-rc.9`.
