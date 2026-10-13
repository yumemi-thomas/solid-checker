# host-dead-return-wildcard

**Claim (ADR 0288, pinning ADR 0282's premise).** `host-dead-return`'s
proven `Loop`, in a project that also holds `Object.assign(target, source)`
with an open source. ADR 0282 premises the stability of
`Array.prototype[Symbol.iterator]` and takes a visible write whose key no
literal names (ADR 0273's `all`) as its veto, project-wide. So no tuple slot
resolves and `Loop` is silent.

This is the shape that kept `createRAF`'s `start()` silent in the rc.13
corpus under `--dependency-source @solid-primitives/raf`: app-game's
`apps/abr-viewer/.../BrushDetailEditable.tsx` holds
`setValues((draft) => Object.assign(draft, next))`. Lifting the veto is a
premise decision recorded in ADR 0288, not a fix; if it is ever taken, this
snapshot moves.

| Case | Finding | Why |
| --- | --- | --- |
| `Loop` | none | the project's `assignAll` is a wildcard member write |

Stubs and `tsc --noEmit -p` as in `host-dead-return`, whose `node_modules`
this copies unchanged.
