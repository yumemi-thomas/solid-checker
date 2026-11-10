# arrow-bound-helper-reachability

**Claim.** An arrow or function expression bound to a single-identifier `const` is reached by calling that binding, as a declaration is by its name (not `let`/`var`): its body runs where its callers run. Reachability grants no owner or role.

Violations (controls: declarations `declaredAction`, `bumpDeclared`): `SC2002` for an action called in an arrow, expression, named-expression and chained helper run in a component body; `SC2001` for a setter, `SC1001` for a read, in an arrow helper. Silent: a helper whose only caller is an event handler. SC2002 (and SC2001: ADR 0721) obligations (ADR 0715): a reassigned `let` helper, and a helper nothing calls or shadowed by a same-named `const` or local arrow (not proven unreached here).

Temporal dead zone (ADR 0339). A reference proves execution only where it follows the declarator: in the same body after it, in a function expression or arrow created after it, in a module-level declaration when nothing running ahead of the declarator can call that declaration and the module is on no import cycle, or from a module that imports the declaring one and the declaring one cannot import back. `Tdz.tsx` pins the rest as silent or uncertifiable: a call from a hoisted declaration run by `createRoot` at module init, a nested arrow called before its declarator (write and action), a `var`, a conditional initializer, a destructured binding and a call-result initializer; `cycle-a.ts`/`cycle-b.tsx` pin the import cycle. Positive controls: a hoisted nested declaration, an arrow created after the declarator, an arrow or nested declaration written before it whose every mention follows it, a module-level declaration calling an earlier arrow, and `provider.ts`/`consumer.tsx` across a one-way import.

Stubs: published rc.13 typings (`SC9014`: no `@solidjs/signals` install).

`SwitchScope.tsx`, `ModuleSwitch.tsx` (a declarator in a `switch` case is withheld, from `in_switch_case`), `Wrapped.tsx` (mentions from the compiler's reference index: `go!()`, `(go as F)()`, `(go satisfies F)()`), `barrel-b/c/d` (an importer on an import cycle is not ordered after the declaring module).

a direct `eval` withholds every mention of a file's bindings and leaves every function-valued binding unproven (variant `eval`, below).

ADR 0339 as an allowlist: only a hoisted declaration whose binding is never written, a `const` or `let` (by source position), and an import ordered by module evaluation are credited; everything else is withheld. `EvalReassignedComponent.tsx` withholds a component or exported root whose binding may be written (a direct `eval` in the file included; `ExportedComponentControl.tsx` is the positive). `ArrowDefault.tsx`: a default parameter initializer that is more than a literal runs before the body and may throw, so mentions of the function are withheld (facts schema 70, `parameters_inert`; `Control` with a literal default is the positive). `VarSkipped.tsx`: a `var` is initialized only if its initializer ran (loop, branch, caught throw): never credited.

ADR 0506: `Tdz.tsx`'s `WrappedInitializer` (`wrap(() => setCount(8))` at module scope) is reached as an argument; its write is uncertifiable, since `wrapped()` holds the call's result.

Variant `eval` holds `EvalCaught.tsx`, `EvalRoot.tsx`, `EvalReassigned.tsx` and `EvalReassignedComponent.tsx`: a direct `eval` may patch array iteration (ADR 0803 item 3), withdrawing every tuple slot in its project; the variant pins that, the cases here keep their proofs.