# refresh-target-untouched (ADR 1089)

`refresh(target)` in rc.21 (`@solidjs/signals` 2.0.0-rc.21, `dist/dev.js:3636-3710`) reads `target[$REFRESH]`, the node
`accessor()` stores on every Solid accessor (`:3237-3240`), marks that node and resolves its promise with the node's value. It
never calls `target` and never hands it on. So an accessor whose only use is `refresh`'s argument has no read the value-use walk
leaves open (`Dialect::argument_is_untouched_target`).

- `Direct`, `Namespace`: no SC9012 at the accessor (the namespace member resolves to the same primitive).
- `AlsoHanded`: the accessor also goes to `keep`, which nothing here reads: still open.
- `InBody`: `refresh` in a component body stays a violation (the misuse twin).
