# flush-universe-inline-suite

**Claim (batch 11, slice 5, the second re-review of #213: N4).** `node:test` runs a suite body inside the `describe` call, at module evaluation (ADR 0438), so the census gives a flush written directly in it the class of the registration's position, `ModuleInitialization` at module level: nothing has run yet, no step, apply or leaf callback is below it. The class says nothing of code written in the body that runs elsewhere or later, and the accounting asks of what is written inside the registration the same questions as of a scheduler's callback (`flush_accounting::suite_body_runs_it`): a class field's initializer (an instance's construction runs it), a default value (the function's call runs it), a prop written in JSX (a getter that runs where the component reads it) and a suite body that is a named function expression its own code uses (`kept = again`, called later from any stack) end no code: each flush is an uncertifiable row of each of the three codes, never a violation and never silence. The runtime: Node 24 with the real `node:test` and solid-js rc.21 (dev build) ran `Late = class { x = flush(); }` in a suite body and `new Late()` in an effect's apply, and the diagnostic FLUSH_IN_EFFECT_CALLBACK was raised at the flush (Chrome cannot load `node:test`: the shape is held by this fixture and by the Node run in the PR).

`App.test.tsx`: silent, on `FlushSafe::ModuleInitialization`: the flush directly in the suite body, in a nested suite body, and in a named suite body whose name nothing uses (`once`). A generator handed to an inline carrier is a function of its own with no class: uncertifiable before and after. `Card.tsx` steps an action, runs an effect's apply and opens a leaf scope, so no program-wide reason holds and each flush stands on its class alone.

Published bytes: `node_modules/@types/node` is `@types/node@26.6.5`, integrity `sha512-uMzsv9stDdofIZBx4UGh4Z9ZgXnGC9seRDscilBcTHAeOFk67MZg/k9bWVfO+bk4qZmG+l52hRnLwUiH2LnJTg==`; only `package.json` and `test.d.ts` are copied, unchanged from `fixtures/reactive-ir/test-runner-node` (`node-test.d.ts` is a reference that brings the module declaration into the program, as the package's own `index.d.ts` does):

```
e9bf302124358d90ec233c509c4f9fb2223129b988da8147b5c1bf835f652e86  package.json
3e221ff0522e6ff95aa74a5211126aaf4d7e85ed8c8ff19290d591ad4bef988f  test.d.ts
```

The `solid-js` stub carries only the version (2.0.0-rc.21) that selects the dialect release; `.solid-checker/runtime.json` closes the program boundary. See `flush-universe` and `flush-universe-test-runner`.
