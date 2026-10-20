# arrow-bound-helper-reachability

**Claim.** A function expression or arrow bound to a `const` identifier is reached by calling that binding, exactly as a declaration is reached by its name. The reachability node of `const useMenu = () => ...` takes the binding's symbol (only an immutable single-identifier `const`; a `let` or `var` is not an exact binding), so the helper's body is proven to run wherever its callers run. Reachability grants no owner and no role: the same classification that judges a declaration's body judges it.

Violations (the declaration `declaredAction` and `bumpDeclared` are controls with the same findings): `SC2002` for an action called in an arrow, expression, named-expression and chained (component to arrow to arrow) helper run in a component body; `SC2001` for a setter in an arrow helper; `SC1001` for a signal read through an arrow helper. Silent: a helper whose only caller is an event handler, a helper nothing calls, a same-named `const` or local arrow shadowing the module helper, and a reassigned `let` helper (a call through it may reach either function).

Stubs are copied from the published solid-js 2.0.0-rc.13 typings (`createSignal`, `action`); `SC9014` is the fixture's missing `@solidjs/signals` install, not its subject.
