# test-runner-suites

Claim (ADR 0396): a suite body runs where its runner's bytes say. jest-circus (`@types/jest`), mocha (`@types/mocha`), `node:test` and playwright call the body inside the registration call, so it takes the role of the call's position: under `createRoot` its effect is owned and its dispatch runs on an owned stack (the veto stays, the handler's effect is uncertifiable); at module evaluation the effect has no owner and the dispatch leaves the veto (both SC4001). A test (`test`, `it`) is recorded and run later from the runner's loop wherever it is registered (`variants/jest-test`: SC4001 under `createRoot`). vitest collects the body after the file loads (`test-runner-vitest/variants/suite-scheduled`: SC4001 under `createRoot`) and `bun:test` shows no readable bytes (`test-runner-bun/variants/suite-unproven`: nothing is placed). This project is the jest case under `createRoot`.

Variants: `mocha`, `playwright` (`test.describe`), `node` (`test-runner-node/variants/suite-inline-owned`) are the same project over each runner; `jest-module`, `mocha-module`, `playwright-module` write the registration at module level; `jest-test` registers a test under `createRoot`.

Published bytes, unchanged (tarball integrity):

- `@types/jest@30.0.0`, `sha512-XTYugzhuwqWjws0CVz8QpM36+T+Dz5mTEBKhNs/esGLnCIlGdRy+Dq78NRjd7ls7r8BC8ZRMOrKlkO1hU0JOwA==`: `package.json` 03428df6…336f9e, `index.d.ts` 3e2f739b…b370;
- `@types/mocha@10.0.10`, `sha512-xPyYSz1cMPnJQhl0CLMH68j3gprKZaTjG3s5Vi+fDgx+uhG9NOXwbVt52eFS8ECyXhyKcjDLCBEqBExKuiZb7Q==`: `package.json` 5f3f8e3f…e863, `index.d.ts` 29f72ec1…3636;
- `playwright@1.63.0`, `sha512-+7ziBLidS4NaNCdt57SUDT+wYmmd5fmiQejUic/kb+YsYSCPyOOE9sebzMjNmQrsnNpDJqd4WHvV/8lfKfUDUg==`: `package.json` 8bc2ddeb…0998, `test.d.ts` 3f00324f…d79c, `types/test.d.ts` 0c45393d…c056;
- `@playwright/test@1.63.0`, `sha512-oxMK4vllB9RK5NQ2l1pq1IfOf2AvnEuj/vYGDj0H2nMtmtZpKtCwt/l00GEO6xjGfpBNAvjovvYdCm50dRQkpQ==`: `package.json` 5587f932…9536, `index.d.ts` ec8997c2…1e48.

Other imports of those files (`expect`, `playwright-core`) are not installed and resolve to nothing. The `solid-js` stubs carry only the version (2.0.0-rc.17) that selects the dialect release; `.solid-checker/runtime.json` closes the program boundary.
