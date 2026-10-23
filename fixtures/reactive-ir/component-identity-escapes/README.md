# component-identity-escapes

ADR 0331: a direct JSX return proves a component only when the census accounts for every reference; otherwise the function is analyzed as one and every violation in its own body is uncertifiable (`UndecidedRegion`), except in callbacks a dialect primitive places.

- `Tagged` (positive): rendered by its tag; the body read is a violation.
- `Row` in `items().map(Row)` and the method `renderers.badge()` (uncertifiable): plain calls in tracked JSX (rc.13: no `STRICT_READ_UNTRACKED`), but other code may render either.
- `Cell` as `<List component={Cell}/>`, called by `List` in tracked JSX (uncertifiable): the prop's name proves nothing.
- `Route.tsx`: a route component in an opaque router config: body read uncertifiable; its `For` callback read stays a violation.
- `Stored.tsx` (uncertifiable): stored (`parts.badge()`), by shorthand (`{ Chip2 }`), through an alias (`[1].map(chip)`). `Inner`, tagged inside the stored `Shell`, stays a violation.
- `MethodAlias.tsx` (uncertifiable): a method called in tracked JSX and rendered through an alias, or whose holder escapes (ADR 0372).
- `Writes.tsx` (uncertifiable): a setter call in `Step`, handed to `map` in a handler, directly and through `untrack`: an undecided body's owned role is unproven (SC2001).
- `Default.tsx` (uncertifiable): an anonymous `export default` arrow; importers' uses are not enumerated.
