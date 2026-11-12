# flush-universe-package-hand-off-accessors

**Claim (the review of #222's check 3, R1 and R2; ADR 0967 amended).** An accessor runs where it is read, not where its value goes. A flush that an installed package the build does not read as source (`steplib`, as in `flush-universe-package-hand-off-causes`) may run as an action's step keeps its three uncertifiable rows (SC2006 `flush-in-action-unaccounted`, SC2008 `flush-in-effect-unaccounted`, SC3001 `leaf-operation-unaccounted`): never silence, never a violation.

**R1.** The compiler makes a component's child or prop expression a getter of its props object. A project component that reads the prop in a step the package runs (`runStep(() => { void props.children; })`, from its own timer) runs the expression there, whatever it does with the value. `stepChildCall`, `StepChildElement`, `destructuredChild` (`const { children } = props` in the step), the flush written in `DirectReader`'s child expression, and `renderProp` (a `render` callback handed to `steplib`'s `Pkg`) keep three rows each. Before this, a project component's first parameter was followed by value only, and a read whose value goes nowhere is inert there.

**R2.** An object literal's getter runs where a member of the object is read: `getterObject.g` read through `readsInStep`'s parameter in a step keeps three rows. Before this, the getter was followed as a function value, which a member read does not move.

**Twins, silent.** `twinBodyChild`: `BodyReader` reads its children in its own body, which runs where its element is created (a timer's callback). `twinTimerChild`: `TimerReader` reads them in a timer's callback. `twinGetterObject`: the getter is read through a parameter in a timer's callback.

Published bytes: `node_modules/steplib` is synthetic, as in the causes fixture; `@solidjs/web/types/index.d.ts` is the same excerpt. `.solid-checker/runtime.json` closes the program boundary.
