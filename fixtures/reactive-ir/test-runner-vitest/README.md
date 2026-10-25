# test-runner-vitest

Claim (ADR 0396): a callback handed directly, as a function literal, to a vitest registration (`it`, `test`, `describe`, a hook, their `only`/`skip`/`each` members) is invoked by the runner from its own scheduler: it has no Solid owner and runs outside every strict-read window. A synchronous dispatch written in it therefore does not withhold the event proofs (this project: the handler's effect creation is a violation, its write and read are clean), and code in it is placed as a runner callback.

Published bytes: `node_modules/vitest` is `vitest@5.0.5` (the declaration files only; the runtime premise cited in ADR 0396 was read on the 5.0.0 bytes the corpus installed, not on these), integrity `sha512-cPODOMF8YnkiiYtDdHo6n4wWmBrOBBbOIYH2d33M4l3CdNn72h+qMmO6uXffUN6A5AZFXNND/d+krS9mgkXRfg==` (registry tarball). Only the package manifest and the declaration files `dist/index.d.ts` needs are copied, unchanged:

```
d00333f3a13ed44fedc77184c8675bae45246534f06684d94cff5e4f8a9f7c10  package.json
e7f7f7f328f7cc0a9ce1e16d72180967d0f09bfe79c10401b0e9ddfb9b84f3d4  dist/index.d.ts
022c64ffd21008457427d9ba910c81a653775ad7c36d890f7a4b75151034a959  dist/chunks/browser.d.FGQ9_rK6.d.ts
cdcced99c921c651feb245ac08300520a49e5c8aba0d980d8b7ad61312db735b  dist/chunks/config.d.QBNYCgFU.d.ts
b413fbc6658fe2774f8bf9a15cf4c53e586fc38a2d5256b3b9647da242c14389  dist/chunks/evaluatedModules.d.BxJ5omdx.d.ts
dc1ba9578b49d66ae26927b6ce62e3796e295f87d78d092645afe416a1382818  dist/chunks/rpc.d.Cft1biqy.d.ts
0bb393b3ff2115443530b3b97f9711074050888968cb2dfc3840664f2791401b  dist/chunks/task-utils.d.PFh2BV6Q.d.ts
b2c3155317bd1541fd548729c1b1226155fc9cbf493cab2ce69dfaef77bc6ce6  dist/chunks/worker.d.BH-r1bPu.d.ts
```

The other imports of those files (`tinybench`, `vite`, ...) are not installed and resolve to nothing; the registration functions are declared in the copied files. The `solid-js` stubs carry only the version (2.0.0-rc.17) that selects the dialect release; `.solid-checker/runtime.json` closes the program boundary.

Variants (each a project of its own over this `node_modules`):

- `variants/shadowed`: a project function named `it` is not the runner (the callee resolves to a project declaration), so the dispatch keeps the veto.
- `variants/wrapper`: `it("x", wrap(() => ...))` hands the runner whatever `wrap` returns, which may run the arrow on its own stack, so the dispatch keeps the veto.
- `variants/members`: `describe.skip`, `beforeEach`, `afterAll`, `it.only`, `test.skip` and `it.each(rows)(...)` are all registrations: every dispatch is excluded.
- `variants/role-owner`, `role-write`, `role-read`: the placement is not confined to the veto. A registration written in a component body hands the runner a callback with no owner and no window: its effect creation is SC4001, its write and read are clean, where the same callback handed to a project function that calls it inline is owned by the component (its write is SC2001, its read SC1001).
- `variants/async-dispatch`: an `async` registered callback is in the premise whole: a dispatch before its first await, after one, and inside a `for await` loop is excluded (the continuations run with no owner, ADR 0333). `variants/async-role`: its reads and writes, before the await, after it and in the loop, are clean where the same async body handed to a project function that calls it inline reports SC1001 and SC2001.
- `variants/helper-test-only`: a helper called only from test callbacks has the same premise, whether declared in the `describe` body or at module level, called through another such helper, or async and dispatching after its await: every dispatch is excluded (class `test-only helper`), and the helper's own signal read and write are clean. `variants/helper-passed-through`: handing the helper to a project function that only tests call keeps it test-only.
- `variants/helper-component-call`, `helper-passed`, `helper-open`: a helper also called from a component body (there its read and write are SC1001 and SC2001), stored as a value (`handlers.push(choose)`), or in a program whose boundary is not closed (no `.solid-checker/runtime.json`) may be called from elsewhere, so its dispatch keeps the veto.
- `variants/helper-exported` (ADR 0400): an exported helper in its own module of a closed program is called only where the program calls it: every other module's reference to it is a call in a test callback, so its dispatch leaves the veto (`Card`'s handler write and read are clean). `helper-exported-barrel` reaches it through an `export *` module, which is still only calls. The veto stays where a module's reference is not a call (`helper-exported-value`: kept in a table), a call is in a component body (`helper-exported-component`), a namespace import that reaches it escapes (`helper-exported-namespace`) or the boundary is open (`helper-exported-open`).
- `variants/helper-literal-test`, `helper-literal-component`, `helper-literal-stored` (ADR 0400): a function literal handed to a project helper that calls its parameter during the call (`fireAndFlush(fire) { fire(); flush(); }`) runs where the call that handed it over runs. Called only from test callbacks, a dispatch written in the literal leaves the veto and a write in it is clean; the same helper also called from a component body makes its write SC2001 and its read SC1001 there and keeps the dispatch's veto; a helper that keeps its parameter (`later`) leaves the obligation and the veto. `helper-literal-async-destructured`: the census also places a call written after an `await` in an async helper (ADR 0333: the resumption has no owner and no window, and the part before it runs on its caller's stack) and a helper a factory returns in an object that a test destructures (`const { open } = renderCard()`); `helper-literal-async-component` and `helper-literal-destructured-component` call the same helpers from a component body, an owned entry, and keep the veto.

`test-runner-vitest4` (a project of its own) is the vitest 4 layout, where the registration is declared in `@vitest/runner`.
