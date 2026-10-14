# host-dead-return-wildcard

**Claim (ADR 0288).** `host-dead-return`'s proven `Loop`, in a project that
also holds `Object.assign(target, source)` with an open source (ADR 0273's
wildcard veto, `all`). ADR 0282 premises the stability of
`Array.prototype[Symbol.iterator]`. Its amendment refuses the unresolved
target: even though no other holder can reach the fresh returned literal,
`target` may name Array.prototype or an iterator prototype. `Loop` is now
uncertifiable, like the explicit target in `host-dead-return-prototype`.

This is the shape that kept `createRAF`'s `start()` silent in the rc.13
corpus under `--dependency-source @solid-primitives/raf`: app-game's
`apps/abr-viewer/.../BrushDetailEditable.tsx` holds
`setValues((draft) => Object.assign(draft, next))`.

| Case | Finding | Why |
| --- | --- | --- |
| `Loop` | SC9012 uncertifiable at `start()` | the target may be an intrinsic prototype |

Stubs and `tsc --noEmit -p` as in `host-dead-return`, whose `node_modules`
this copies unchanged.
