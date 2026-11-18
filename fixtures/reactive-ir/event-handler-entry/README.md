# event-handler-entry (ADR 1089 part E)

An anonymous function written as a plain `on*` attribute of an intrinsic element is handed to rc.21 `@solidjs/web`, which
delegates it to `node["_$$" + name]` (and its data to `node["_$$" + name + "Data"]`) or adds it with `addEventListener`
(`dist/web.dev.js:480, 790-814`). It is entered by event delivery alone while no code that may run in its realm can reach the stored handler
(`plain_premise::stored_handler_hazard`, asked by realm, ADR 1120). A creation in it then runs off every effect apply's stack (SC6002).

- `Resolve`, `Until`: no SC6002 row at the creation.
- `Named`: the handler is a name an effect's apply also calls: the row stays.
- `InCompute`: `resolve` in a memo's compute keeps its violation (the misuse twin).
- `variants/ref-read`: code reads `_$$click` from the element: the realm holds a Solid private key, the row stays.
- `variants/computed-read`: a member read under a key no constant names: the row stays.

`node_modules` holds the published rc.21 bytes of `solid-js`, `@solidjs/signals`, `@solidjs/web` and `csstype`, and the
`vite-plugin-solid` manifest that makes the checker read package source (ADR 0297), copied unchanged from
fixtures/uncertifiable-repro (versions, registry integrity and each file's sha256 in `packages.json`). Without the real
`@solidjs/web` the handler is handed to an unread package and declines on `unread-package-may-run` first. The variants resolve
the same install from their parent directory.
- Round 2 (ADR 1120's per-question API), the Fable review's four false cleans, each keeps its SC6002 row:
  - `variants/shadow`: the iframe moved into a shadow root by an effect's apply (F1; a shadow-root spelling in the realm).
  - `variants/listener-patch`: `EventTarget.prototype.addEventListener` replaced (F2; a prototype route, a listener-API write).
  - `variants/escaped-key`: `el["_\x24\x24click"]` (F3; the key's cooked value holds `$$`).
  - `variants/define-setter`: `Object.defineProperty(Element.prototype, "_$$" + "click", …)` (F4; a prototype route).
