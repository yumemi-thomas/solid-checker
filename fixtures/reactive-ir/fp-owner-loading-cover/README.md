# fp-owner-loading-cover

**Claim.** `async-outside-loading-boundary` (`SC5003`) is a claim about a
*render* with no `<Loading>` above it, proven only when the whole render chain
is visible:

- a read inside a memo, a user effect's compute or a derived signal is not a
  render (measured on rc.9 dev: only a render effect emits
  `ASYNC_OUTSIDE_LOADING_BOUNDARY`); `createRenderEffect` still is one;
- a memo whose *value* is an async function is never pending, so it is not an
  async source (`computation_is_async` matches the computation's own function);
- a boundary any number of call-site hops above the rendering component covers
  it;
- a chain that ends at a function nothing in the project renders (an exported
  component, a route handed to a router) is **uncertifiable**; only a chain
  followed to a `render()` root with no boundary is a proven violation.

| Case | Finding | Why |
| --- | --- | --- |
| `createMemo`/`createEffect` compute reads the async memo, consumer under `<Loading>` | none | not a render |
| `createMemo(() => async () => ...)` read in JSX | none | the memo's value is a function |
| `Leaf` rendered by `Middle` rendered under `<Loading>` | none | two hops |
| `UnmountedRoute`, exported, rendered by nothing | `SC5003` uncertifiable | mount context unresolved |
| `Page` rendered from the `render()` root | `SC5003` violation | chain fully resolved, no boundary |
| `createRenderEffect` compute, mounted by `render()` | `SC5003` violation | render effect |

Stub as in `fp-owner-show-children-callback` (plus `@solidjs/web`'s `render`,
verbatim); the file also type-checks against the real rc.9 typings.

`CallbackPropPage` (P-M): an async read inside a component's callback prop
(`onPick={() => pick(articles()[0]?.id)}`) runs when the row calls it, so it
is not reported, even though the page is mounted from `render` with no
boundary (`owners::async_read_role`). Before, it was a proven violation.
