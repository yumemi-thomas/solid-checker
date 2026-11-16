# rc.21 read after an await in a test runner's callback (ADR 1004)

The real-runner half of ADR 1004's evidence: the shapes of `fixtures/reactive-ir/after-await-runner-callback` under vitest 5.0.5's own runner on V8 (Node 26.13.0), against the dev build of `@solidjs/signals` 2.0.0-rc.21 (`dist/dev.js`, named by file in `vitest.config.ts`), with the dev diagnostics collected from the console (`reportDiagnostic` writes there).

```sh
npm ci --ignore-scripts          # integrities are in package-lock.json
PROBE_OUT=diagnostics.json npx vitest run
```

Recorded 2026-11-16 (`diagnostics.recorded.json`): two diagnostics, both UNTRACKED_READ_AFTER_AWAIT, and both in a computation's flight:

| shape (`runner.probe.ts`) | rc.21 |
|---|---|
| a read, a call of a function that reads and `expect(count())` after an await, written directly in `beforeEach`, in vitest's `describe` body, in `it(…)` and in `it.each(…)(…)` | silent |
| the control: the same body awaited by an async memo's compute, written in a test | UNTRACKED_READ_AFTER_AWAIT |
| a named test callback (`async function again`) on the runner's own call | silent |
| the same function handed by its own name to `createMemo` (its second run, on that computation's flight) | UNTRACKED_READ_AFTER_AWAIT |

The control shows the diagnostics are live in the run. The last row is why `runner-scheduled` refuses a named function expression whose name is used. Nothing here is read by a gate; it is the record a later release is compared against. The Chrome half (the stack semantics, with a loop written like a runner's) is the oracle case `fixtures/runtime-oracle/after-await-runner-callback`.
