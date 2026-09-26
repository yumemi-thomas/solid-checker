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
(`Dialect::callback_runs_on_result_access`, answered only by the rc.9
vocabulary): the engine does not follow reads of a returned view back to the
call that made it, so code inside the predicate is given no execution role
(`ExecutionRole::Unknown`) and is never a violation. A predicate that is not
proven inert is uncertifiable instead: SC9012 `reactive-dispatch-unresolved`
at the predicate argument, with the missing proof named in `analysisContext`
(`static_rules::result_access_callbacks`).

| case | line | result | reason |
| --- | --- | --- | --- |
| `InlinePredicateRead`, reads `mode()` | 20 | SC9012 | `reactive-operation` |
| `InlinePredicateWrite`, writes in the predicate | 27 | SC9012 | `reactive-operation` |
| `NamedPredicateRead`, a `function` passed by name | 39 | SC9012 | `reactive-operation` |
| `BodyReadBesidePredicate`, an inert predicate beside a body read | 48 | SC1001 on the body read only (control) | |
| `KeyListForm`, `omit(props, "a")` | | silent | a key list is a value |
| `StandardLibraryPredicate`, only `typeof` and `String#startsWith` | | silent | inert |
| `ConstPredicateRead`, a `const` arrow passed by name | 67 | SC9012 | `reactive-operation` |
| `PropsPredicateRead`, reads `props.a` | 73 | SC9012 | `reactive-operation` |
| `HelperPredicate`, calls an inert project helper | 84 | SC9012 | `opaque-call` (calls out of the predicate are not followed) |
| `ImportedPredicate`, `hiddenKey` from `./keys` | 91 | SC9012 | `body-unresolved` |
| `hideBy`, forwards its parameter into `omit` | 100 | SC9005 only | the exported wrapper's open callback; the body is its callers' |
| `ForwardedPredicateCaller`, `hideBy(props, (key) => key === mode())` | 107 | SC9012 | `reactive-operation`, one wrapper away |

Before 2026-09-26 the first two cases were violations placed in the component
body (SC1001 "read directly in rendering function", SC2001), and after the
first rc.9 modelling they were silent, which let `--certify` answer
`certified`. `ForwardedPredicateCaller` was still an SC1001 violation until the
wrapper was followed. The predicate runs where `rest.b` is read, which here is
tracked JSX, and the call site cannot know that.

The package-contract half of the same fact, a forwarded predicate no longer
published as a value the export never invokes, is pinned by
`fixtures/package-contracts/rc9-callback-forms`.

`tsc --noEmit` is clean against this fixture's stubs. The `omit` overloads in
`solid-js.d.ts` are byte-faithful to the published rc.9 typings, which were
checked for the original five cases. Against rc.3's typings every predicate
call is TS2345, so no rc.3-valid typed call is affected, and the rc.3
vocabulary does not answer the predicate slot at all.
`node_modules/solid-js/package.json` selects the 2.0 dialect at `2.0.0-rc.9`.
