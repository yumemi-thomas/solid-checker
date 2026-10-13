# host-dead-return-wildcard

**Claim (ADR 0288).** `host-dead-return`'s proven `Loop`, in a project that
also holds `Object.assign(target, source)` with an open source (ADR 0273's
wildcard veto, `all`). ADR 0282 premises the stability of
`Array.prototype[Symbol.iterator]`. The wildcard write cannot reach the
tuple: the array is a literal created at the factory's return and
destructured by the call's own binding, so nothing else holds it. `Loop` is
reported. A write through a `prototype` still vetoes the tuple
(`host-dead-return-prototype`).

This is the shape that kept `createRAF`'s `start()` silent in the rc.13
corpus under `--dependency-source @solid-primitives/raf`: app-game's
`apps/abr-viewer/.../BrushDetailEditable.tsx` holds
`setValues((draft) => Object.assign(draft, next))`.

| Case | Finding | Why |
| --- | --- | --- |
| `Loop` | SC1001 at `start()` | a wildcard write on another target does not reach a fresh tuple |

Stubs and `tsc --noEmit -p` as in `host-dead-return`, whose `node_modules`
this copies unchanged.
