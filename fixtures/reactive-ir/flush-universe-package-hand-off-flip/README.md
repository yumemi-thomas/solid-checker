# flush-universe-package-hand-off-flip

**Claim (batch 11, the flip of ADR 0967's default; the review of #216).** The PR before this (`flush-universe-package-hand-off`) followed the routes a function reaches an installed package the build does not read as source, and treated every context it did not recognize as "not handed". A review of it (Fable, BLOCKED) found fourteen programs that stayed silent where the package runs the flush as an action's step, all of them false cleans on `main` too. The owner's ruling: **a value counts as handed unless a fact of the program proves where it goes**, so the flush keeps its uncertifiable row of each code (SC2006 `flush-in-action-unaccounted`, SC2008 `flush-in-effect-unaccounted`, SC3001 `leaf-operation-unaccounted`): never silence, never a violation. `package_hand_off.rs` classifies each position of a function and of every holder it is copied into (call, argument, tagged template, JSX, initializer, assignment, `return`, member read, inert operand) and a position it does not know is handed.

Each handed function below is three rows (fifteen sites in `App.tsx` and `nsmod.ts`: 45 rows), against none on #216's build for the fourteen the review found (and the `Control`, the earlier fixture's `runAll([…])`, which was already handed):

| site | position | the package runs it because |
|---|---|---|
| `Collection` | a `Set` the project keeps and reads elsewhere | `Set.add` retains its argument (a standard call the table lists as keeping a value) |
| `ForOf`, `ReceiverArray` | an iterable's element, an array literal that is a method's receiver | a context nothing here follows |
| `PromiseValue` | `Promise.resolve(cb)` | the standard call returns its argument; the table does not list it |
| `ClassMethod` | a method of `class Job`, `runWith(new Job())` | a class's functions are handed whole (an instance, a subclass or `this` may hand them on) |
| `PackageAssign` | `config.onStep = …`, `config` imported from the package | an assignment into an object nothing here declares (an import) |
| `ComponentProps` | `<Wrap render={…}/>`, `Wrap` passes `props.render` to the package's `<Pkg>` | a project component's props are its first parameter, followed in its reads |
| `Accessor` | `runGet(() => accessed)` | a function the package runs may call what it returns |
| `ReflectedProject` | `viaStep.call(null, cb)`, `viaStep` goes on to `runStep` | a reflective call's forwarded arguments are the target's parameters, shifted by the thisArg |
| `InlineStandard`, `InlineParameter`, `InlinePrimitive` | `[1].forEach(inner)`, `callIt(inner)`, `untrack(inner)` in a step handed to the package | a value that is called or run inline runs on the stack of the function the call is written in |
| `DynamicPackage` | `<Dynamic component={Pkg} render={…}/>` | `Dynamic` hands its props to the component its `component` prop names (the real `@solidjs/web` declaration, below) |
| `Namespaced` (`nsmod.ts`) | `runNs(NS)`, `NS` a namespace import | a namespace that escapes with no readable name hands out every export of the module (the namespace census, ADR 0347) |

Silent twins, each by a fact of the program: `DomProperty` (`el.onclick = …` on a local element nothing reads), `ComponentPropsCalled` (`Own` calls `props.render()` itself: a member the project declares runs where it is called, and `Own` is not run by a package), `InlineLocal` (the same three inline runs and `.call(null)` on a stack no package runs), `Listener` (`addEventListener` registers on the host and `removeEventListener` only removes), `Inert` (`quiet === undefined` takes nothing), `DynamicIntrinsic` (`component="button"` is the DOM's), `NamespaceCalled` (`Local.localStep()` only calls into the module; the namespace is handed nowhere) and `Unreferenced` (an export nothing in a closed program enters: the site never runs).

`variants/open` is `Unreferenced` with the boundary open (`.solid-checker/runtime.json`): an export may be entered by code this build cannot see, so the site is reached and has no census reason: three rows. `flush-universe-package-hand-off` keeps the earlier routes (72 rows).

Published bytes: `node_modules/steplib` is synthetic (the package that runs a handed function as an action's step; its manifest names no Solid package, which is why the build does not read it), as in `flush-universe-package-hand-off`. `node_modules/@solidjs/web/types/index.d.ts` is the declaration excerpt of `dynamic-string-tag-events` (the `Component` import and the declarations `IntrinsicElement`, `ValidComponent`, `ComponentProps`, `DynamicProps` and `Dynamic`, byte-faithful to `@solidjs/web` `types/index.d.ts`): compared against `@solidjs/web@2.0.0-rc.21` (lines 40-47 and 177), the lines are identical; where `Dynamic` is declared (a package of the dialect, under that name) is the dialect's premise (`intrinsic_tag_renderers`), so it is not loosened. `.solid-checker/runtime.json` closes the program boundary.
