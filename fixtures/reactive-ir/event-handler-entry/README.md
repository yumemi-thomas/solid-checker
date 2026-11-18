# event-handler-entry (ADR 1089 part E)

An anonymous function written as a plain `on*` attribute of an intrinsic element is handed to rc.21 `@solidjs/web`, which
delegates it to `node["_$$" + name]` (and its data to `node["_$$" + name + "Data"]`) or adds it with `addEventListener`
(`dist/web.dev.js:480, 790-814`). It is entered by event delivery alone while no code of the program can read the stored handler
(`event_delivery::scan_delegated_handler_reach`). A creation in it then runs off every effect apply's stack (SC6002).

- `Resolve`, `Until`: no SC6002 row at the creation.
- `Named`: the handler is a name an effect's apply also calls: the row stays.
- `InCompute`: `resolve` in a memo's compute keeps its violation (the misuse twin).
- `variants/ref-read`: code reads `_$$click` from the element: the program-wide premise does not hold, the row stays.
- `variants/computed-read`: a member read under a key no constant names: the row stays.

`node_modules` holds the published rc.21 bytes of `solid-js`, `@solidjs/signals`, `@solidjs/web` and `csstype`, and the
`vite-plugin-solid` manifest that makes the checker read package source (ADR 0297), copied unchanged from
fixtures/uncertifiable-repro (versions, registry integrity and each file's sha256 in `packages.json`). Without the real
`@solidjs/web` the handler is handed to an unread package and declines on `unread-package-may-run` first. The variants resolve
the same install from their parent directory.
