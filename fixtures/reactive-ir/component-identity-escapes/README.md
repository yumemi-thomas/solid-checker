# component-identity-escapes

Solid runs a function as a component only when `createComponent` calls it (a tag, a dialect renderer prop, `lazy`); called directly, its body runs in the caller's scope (rc.13 `observedComponent`: `createRoot(() => untrack(() => Comp(props), label))`). A tag, a direct JSX return or an exact `Component` type is therefore proof only when every reference is accounted for: the declaration, the module surface, a tag, a reference inside a type query (`typeof Row` in a type), a dialect callback value, a dialect renderer prop (ADR 0138) or a dialect loader's `default` (ADR 0292). Any other reference, a function with no resolvable binding, or an object or class method leaves identity undecided. The function is still analyzed as a component, and every violation in its body becomes uncertifiable (`UndecidedRegion`); callbacks a dialect primitive places itself are carved out.

- `Tagged` (positive): rendered by its tag; the body read is a violation.
- `Row` in `items().map(Row)` and the object method `renderers.badge()` (uncertifiable): plain calls inside tracked JSX, runtime-probed on rc.13 with no `STRICT_READ_UNTRACKED`.
- `Cell` handed as `<List component={Cell}/>` to a project component that calls it (uncertifiable): the prop's spelling proves nothing.
- `Route.tsx`: a route component in an opaque router config; its body read is uncertifiable, the `For` callback read inside it stays a violation (placed by its primitive).
- `Stored.tsx` (uncertifiable): stored in an object, by shorthand (`{ Chip2 }`, a reference since Type Facts records shorthand values) and through an alias. A nested `Inner` rendered by its tag inside the stored `Shell` is carved out and stays a violation.
- `Writes.tsx` (uncertifiable): a setter call in `Step`, handed to `map` in an event handler, directly and through `untrack` (SC2001).
- `Default.tsx` (uncertifiable): an anonymous `export default` arrow; its importers are not enumerated.
- `Mixed.tsx` (uncertifiable): `Mixed` has a tag in `<Show when={false}>` and `items().map(Mixed)`; reachability is not folded, so a tag-only function stays proven. `MixedReachable` (reachable tag) and `Called` (`Called()`) too: the recall cost. Probed in `runtime-oracle/precision-unreachable-tag-renderer`.
- Controls (violations): `Exported` (tag and `export {}`), `Remote` (tag in another file, named import), `Chip` (`<Parts.Chip/>`).
- `Typed.tsx`: an exact `Component` type never overrides an escaping reference (`tsc` accepts `items().map(Typed)`). Uncertifiable: `Typed` (tag and `map`), `TypedHelperMapped` and `TypedBareMapped` (no JSX return, a helper's element; `map`). Violations: `TypedOnly` (tag), `TypedHelper` (no JSX return, tag), `TypedBare` (no JSX return, no tag; the type and the export prove it).
- `Types.tsx`: `TypeRef` with `ComponentProps`, `Parameters` and `ReturnType` of `typeof TypeRef` stays a violation (a type query is erased). Uncertifiable: `TypeMapped` (type query plus `map`) and `RuntimeTypeof` (`typeof RuntimeTypeof === "function"` is a value reference).
- `solid-js.d.ts`: `Show`, `Component`, `ComponentProps` and their helper types are transcribed from rc.17.
