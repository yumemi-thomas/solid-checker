# lazy-default-consumer-rc9

**Claim (ADR 0292).** `lazy-default-consumer`'s `Default.tsx` over
`solid-js@2.0.0-rc.9`. That release's `lazy` renders the loaded member as
`untrack(() => Comp(props))` (`dist/solid.dev.js:1175-1179`), with no
`createComponent` and no strict-read label, so `Solid2::lazy_renders_component`
is `false`, `Dialect::renders_loaded_default_argument` answers `None`, and the
object-retention refusal stays.

| Case | Finding | Why |
| --- | --- | --- |
| `LoopCube` `start()` | SC1001 uncertifiable | no witness on rc.9 |
| `NestedCube` `kick()` | SC1001 uncertifiable | no witness on rc.9 |
| `LoopCube` `local()` | SC1001 violation | pre-existing, not this proof (precision backlog) |
| `package.json` | SC9014 uncertifiable | the mixed triple (`solid-js` rc.9, `@solidjs/signals` and `@solidjs/web` rc.13) no review read |

Everything but `node_modules/solid-js/package.json` (its `version`) and the
removed files is copied from `lazy-default-consumer`. `lazy`'s two overloads
are byte-identical in rc.9's and rc.13's `types/client/component.d.ts`.
`tsc --noEmit -p` passes.
