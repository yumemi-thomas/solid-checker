# lazy-default-consumer

**Claim (ADR 0292).** A component declared in a `lazy` loader and returned as
its module's `default` is rendered by `lazy`, not retained by the module
object. Under a declared browser host with `dep-loop` analyzed as source
(ADR 0284), `start()` in its own body is reported as `start()` in a component
rendered by its tag is (`host-dead-return-wildcard`'s `Loop`). The
object-retention refusal (`interproc::runs_in_object_callable`) is withdrawn
for exactly that one witness, and only when every premise holds: the dialect's
`lazy` (`Dialect::renders_loaded_default_argument`, `solid-js@2.0.0-rc.13`
only), its one-argument form, an `async` loader literal at that slot whose one
unconditional return is a closed object literal, a `default` that is a plain
reference to a proven synchronous component declared directly in the loader
and referenced nowhere else, and a wrapper bound to an unexported `const`
whose every reference is a JSX tag name.

This is app-game's `packages/math-examples/rapier-2d-physics-engine` shape
(`TestCube` inside `lazy(async () => { ... return { default: TestCube }; })`),
which contract mode reported and `--dependency-source` left uncertifiable.

| Case | Finding | Why |
| --- | --- | --- |
| `Default.tsx` `LoopCube` `start()` | SC1001 violation | the witness holds; was uncertifiable |
| `Default.tsx` `LoopCube` `local()` | SC1001 violation | the direct read beside it, unchanged: both read paths agree |
| `Default.tsx` `NestedCube` `kick()` | SC1001 violation | the helper's call is written in the body; was uncertifiable |
| `Awaited.tsx` `AwaitedCube` `start()` | SC1001 violation | an `await` in the loader, a `const` arrow, `as Component` on the default, a closing tag |
| `Awaited.tsx` `NamespacedCube` `start()` | SC1001 violation | `Solid.lazy` through a namespace import |
| `Clean.tsx` `EventCube` | none | `start()` only in an event handler |
| `Refused.tsx` `NamedCube` | uncertifiable | `{ export: "Other" }` selects another member |
| `Refused.tsx` `EitherCube` | uncertifiable | two returns select between module objects |
| `Refused.tsx` `MutableCube` | uncertifiable | a `let`, written before the return |
| `Refused.tsx` `AliasCube` | uncertifiable | the module object is aliased and its `default` replaced |
| `Refused.tsx` `SpreadCube` | uncertifiable | a spread in the module object |
| `Refused.tsx` `RegistryCube` | uncertifiable | another holder (`registry.cube = RegistryCube`) |
| `Refused.tsx` `ReferencedCube` | none | the loader is a reference, not the call's literal; already silent, stays silent |
| `Refused.tsx` `SwappedCube` | uncertifiable | `.preload().then(mod => { mod.default = ... })` replaces the rendered member |
| `Refused.tsx` `PreloadedCube` | uncertifiable | only preloaded, never rendered |
| `Refused.tsx` `UnusedCube` | uncertifiable | the wrapper is never rendered |
| `Refused.tsx` `ExportedCube` | uncertifiable | an exported wrapper: another module may preload it |
| `Refused.tsx` `makeCube()` | SC1001 violation | unchanged: the loader's own synchronous prefix calls a helper (ADR 0292 claims nothing here; the default is a call, not a component) |
| `Conditional.tsx` `ConditionalCube` | SC1001 violation | unchanged and **not** this proof: `flag ? ConditionalCube : Plain` is no exact property value, so the retention guard never refused it (precision backlog) |
| `Shadow.tsx` module `ShadowCube` | uncertifiable | held by an exported registry; the loader's own `ShadowCube` shadows it |
| `LocalLazy.tsx` `LocalCube` | uncertifiable | a project function spelled `lazy` |
| `FakeLazy.tsx` `PackageCube` | uncertifiable | another package's `lazy` |
| `Thenable.tsx` `Loaded`, `QuotedCube`, `QuietCube` | uncertifiable | review round 1: an own `then` (data, quoted, method) makes the module a thenable, assimilated when the async return resolves, so `default` may never render |

Twin: `lazy-default-consumer-rc9` is `Default.tsx` over `solid-js@2.0.0-rc.9`,
whose `lazy` renders the member without `createComponent`, so no witness
exists there and both `start()` and `kick()` stay uncertifiable.

`.solid-checker/runtime.json` declares a browser development host and
`dependencySources: ["dep-loop"]`. `node_modules/` is copied unchanged from
`host-dead-return-wildcard` (see `host-dead-return`'s README for the stubs),
plus `node_modules/fake-lazy`, a package of this fixture's own whose one
export is spelled `lazy`. `solid-js.d.ts` adds `Component` and both `lazy`
overloads, copied from the published `solid-js@2.0.0-rc.13`
`types/client/component.d.ts` with JSDoc and `declare` dropped; every other
declaration is `host-dead-return-wildcard`'s. `tsc --noEmit -p` passes.
