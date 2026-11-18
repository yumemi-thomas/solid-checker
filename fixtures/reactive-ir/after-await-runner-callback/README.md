# after-await-runner-callback

**Claim (ADR 1004, on ADR 0438's premise).** A read, or a call that may run a project function, written after an await **directly in a function literal a reviewed test runner schedules** is on no async computation's flight, so SC1002's claim cannot hold there: the site ends with the typed reason `after_await_accounting.account.runner-scheduled` and gets no `read-after-await-unaccounted` row.

Why it is a proof and not a role. Solid 2.0.0-rc.21 raises UNTRACKED_READ_AFTER_AWAIT only when a tail-flight frame of an async computation is on the V8 async stack of the read (`@solidjs/signals` `dist/dev-shared.js`, `attributedFlight`: `Error.captureStackTrace`, 50 frames, the first frame whose function name is a tail frame's). ADR 0438 read the runners' published bytes: vitest 5.0.0 calls a registered test as `fn()` in `runTest`, a hook in `callSuiteHook` and a suite body in `runWithSuite` from `collect`, each from its own loop with no Solid API on the stack, and a registered `async` callback is wholly in that premise, its prefix and every resumption. The async stack of a continuation of such a callback is the callback and the runner's awaiting frames.

The facts asked, the flush accounting's for the same class (`flush_accounting::runs_when_the_call_runs`, `FlushSafe::TestRunner`): the registration resolves, through its alias chain, to declarations that all lie in a reviewed runner's installed declaration files (`SemanticLookup::test_runner_callback`, never a spelling) and the runner **schedules** the callback (`TestRunnerCallback::Scheduled`: a test, a hook, a suite body of a runner whose bytes show it defers the body; never a body run inside the registration call, never `bun:test`'s unproven one); the function around the site is the literal written directly as the registration's argument; the site is in its body, in no class field, JSX expression or tracked region written in it; the literal is no generator; and its own name, if it has one, is used nowhere.

| site (`App.test.tsx`) | outcome |
|---|---|
| `reads()`, `expect(…)` and its `.toBe`, `new Map(…)`, `setCount(1)` in `it("a read", async …)` | ended, no row |
| `count()`, `state.n` and the `count()` inside `expect(…)` in the same callback | ended, no row: ADR 1073, the callback literal is handed to no computation, so the walk starts no member for it (before, each kept the walk's `function-run-unplaced`) |
| `count()` in `it("a stored name", async function self …)`, which stores `self` | SC1002 `function-run-unplaced`: a name the function hands on may reach a computation (ADR 1073 refuses it) |
| `reads()` in `beforeEach`, in vitest's `describe` body, under `test.only`, `it.each(…)(…)`, the alias `check` and the re-export `spec` (`runner.ts`) | ended, no row |
| `reads()` in a wrapper's argument (`wrap(async …)`) and in a binding handed by name (`shared`) | SC1002 uncertifiable, `read-after-await-unaccounted`: no literal in the registration |
| `reads()` under a block-scoped `it` bound to a project function | the same: a project declaration is not the runner |
| `reads()` in `again`, a named function expression that hands its own name to `createMemo` | the same; the read it runs (`count()` in `signals.ts`) is the walk's **violation**, `again` being that computation's compute (the probe's `named-again` row) |
| `count()` in the async memo written in the test | SC1002 **violation**: the computation's own read after its own await |
| the promise handler's read, the nested async arrow's read | uncertifiable: each is its own function |
| `make()` in `pause().then(make())` in `it("a handler argument", …)` | SC1002 uncertifiable, `read-after-await-unaccounted`: ADR 1003's site for a promise handler handed as an argument that names no function. A later job runs the handler, not the callback's own code, so the reason refuses the site (`via: handler`); without that refusal the row is gone (checked by building without it). The `.then` call itself ends |
| `count()` in `helper` (called from a test and handed to `createMemo`) | uncertifiable: a test-only helper is not covered, and this one is no test-only helper |
| `helper()` in `it("a helper", …)` | ended: the call is the callback's own site |

Runtime: the oracle case `after-await-runner-callback` (Chrome rc.21: the stack semantics the reason rests on) and the vitest run recorded in ADR 1004 (the fixture's shape under vitest's own runner on V8, the dev diagnostics collected).

Published bytes: `node_modules/vitest` is `vitest@5.0.5` (the declaration files only), copied unchanged from `fixtures/reactive-ir/test-runner-vitest`, whose README records the manifest and declaration hashes and the registry tarball's integrity `sha512-cPODOMF8YnkiiYtDdHo6n4wWmBrOBBbOIYH2d33M4l3CdNn72h+qMmO6uXffUN6A5AZFXNND/d+krS9mgkXRfg==`. `.solid-checker/runtime.json` closes the program boundary. `solid-js.d.ts` is `after-await-universe`'s.
