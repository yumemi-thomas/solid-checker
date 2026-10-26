# test-runner-vitest4

Claim (ADR 0438): vitest 4 declares `describe`, `it`, `test`, `suite` and the hooks in `@vitest/runner` and re-exports them from `vitest`, so the registration's declaration lies in `node_modules/@vitest/runner`, a reviewed runner location. A dispatch written in an async `it` callback, after an await, is excluded from the veto as in `test-runner-vitest` (this project: the handler's effect creation is the one violation).

Published bytes (the rc.17 corpus's solid-groove installs them; copied unchanged, only the declaration files `vitest`'s types need):

- `node_modules/vitest` is `vitest@4.1.11`, integrity `sha512-fhACrNXUidIbGSBr5FlbuBkO7VWC1ZyLl0DO4CU2DrQoAPxX84Ysxs+HeGQpii5lZWV1Q4gBZTTu49mF+A6Edw==`.
- `node_modules/@vitest/runner` is `@vitest/runner@4.1.11`, integrity `sha512-LztvUgdwMNJMIkj3hQnnxiC2Xy1zNxq928W/xhjCLaNCzqTZOudjwbQf6v9IntZGPw132i2Lq2rgTRZHD3JHNw==`.

```
a28126d97bcaf567da5bed69443b7f3bcd9a7a8c38c8b66e554686b6bb2c10e0  node_modules/vitest/package.json
90ba95a763101bb61b8a799731a2ed60b5016b8135c1a2d5186862d4b534d4a1  node_modules/vitest/dist/index.d.ts
357e56773a82d3aa95bf920a54ea7f91575d02e6b156d9d7719ea485708e6837  node_modules/@vitest/runner/package.json
a39461ee1f27cf3e6cfd63d21045713d26d521da55ea4d8efccb705f689e6dbb  node_modules/@vitest/runner/dist/index.d.ts
53cf4076f42b29b8d411259d168d51b3a0274c42c8814e5b44dfa8803a35d4fc  node_modules/@vitest/runner/dist/tasks.d-DEYaIMIu.d.ts
```

The runner calls a test as `fn()` in `runTest` (`@vitest/runner/dist/chunk-artifact.js:2956`), a hook in `callSuiteHook` (`:2608`, `await hook(...args)`) and a suite body in `runWithSuite` (`:2255`, `:1889`); no Solid API is on those stacks. The other imports of the copied files (`@vitest/utils`, `vite`, ...) are not installed and resolve to nothing. The `solid-js` stubs carry only the version (2.0.0-rc.17) that selects the dialect release; `.solid-checker/runtime.json` closes the program boundary.
