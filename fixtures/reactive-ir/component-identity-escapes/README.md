# component-identity-escapes

Solid runs a function as a component only when `createComponent` calls it (a JSX tag, a dialect renderer prop, `lazy`); called directly, its body runs in the caller's scope (rc.13 `observedComponent`: `createRoot(() => untrack(() => Comp(props), label))`). A direct JSX return is therefore proof only when every reference to the function is accounted for: its declaration, its module surface, a dialect callback value, a dialect renderer prop (ADR 0138) or a dialect loader's `default` (ADR 0292). Any other reference, a function with no resolvable binding, or an object or class method leaves identity undecided. The function is still analyzed as a component, so nothing is lost, and every violation in its own body becomes uncertifiable (`UndecidedRegion`, applied with the instance-field demotion in `diagnostics.rs`); callbacks a dialect primitive places itself are carved out (`indexes/components.rs`).

- `Tagged` (positive): rendered by its tag; the body read is a violation.
- `Row` passed to `items().map(Row)` and the object method `renderers.badge()` (uncertifiable): both run as plain calls inside tracked JSX; both were false violations, runtime-probed on rc.13 with no `STRICT_READ_UNTRACKED`.
- `Cell` handed as `<List component={Cell}/>` to a project component that calls it in tracked JSX (uncertifiable): the prop's spelling proves nothing.
- `Route.tsx`: a route component in an opaque router config. Its body read is uncertifiable; the `For` children callback read inside it stays a violation, since a dialect callback is placed by its primitive.
- `Stored.tsx` (uncertifiable): stored in an object (`parts.badge()`) and through an alias (`[1].map(chip)`). A nested `Inner` rendered by its tag inside the stored `Shell` is carved out of the region and stays a violation.
- `Writes.tsx` (uncertifiable): a setter call in `Step`, handed to `map` in an event handler, directly and through an inline `untrack`; the owned-scope role of an undecided component body is not proven (SC2001).
- `Default.tsx` (uncertifiable): an anonymous `export default` arrow; its importers' uses are not enumerated.
