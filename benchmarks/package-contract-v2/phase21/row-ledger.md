# Phase 21 ecosystem refusal-reduction ledger

- Baseline fully refused rows: 30
- Current report SHA-256: 64d42ab02f9110d83c12817e9fc85d85ca7976fe961b22505937776b72ab4bf0
- Upstream missing-byte controls: 5
- CJS/no-ESM controls: 7
- Checker-addressable rows: 18
- Newly verified rows: 8
- Confirmed upstream declaration defects: 1

## Current terminal classes

| Class | Rows |
| --- | ---: |
| dependency-contract-obligation | 16 |
| success | 6 |
| published-target-missing | 4 |
| no-exported-surface | 2 |
| authenticated-dependency-layout-required | 1 |
| published-declaration-closure-missing | 1 |

## Explicit dispositions

| State | Rows |
| --- | ---: |
| retained-unsupported-runtime-model | 7 |
| exact-refusal-authenticated-layout | 5 |
| retained-upstream-missing-bytes | 5 |
| exact-refusal-semantic-model | 4 |
| exact-refusal-package-import-resolution | 3 |
| pending-phase21-checker-work | 3 |
| confirmed-upstream-declaration-defect | 1 |
| exact-refusal-type-facts-capability | 1 |
| verified-through-ordinary-receipt-load | 1 |

## Remaining owners

| Owner | Rows |
| --- | ---: |
| runtime-model | 7 |
| checker-semantic-model | 6 |
| upstream-package | 6 |
| authenticated-dependency-layout | 5 |
| checker-resolver | 3 |
| checker-type-facts | 2 |
| none | 1 |

## Row disposition

| Probe | State | Remaining owner | Terminal class |
| --- | --- | --- | --- |
| @kobalte/themes@0.0.1-next.0\|solid1\|only | retained-upstream-missing-bytes | upstream-package | published-target-missing |
| @solid-devtools/babel-plugin@0.3.1\|solid1\|only | retained-unsupported-runtime-model | runtime-model | no-exported-surface |
| @solid-devtools/ext-adapter@0.17.0\|solid1\|only | retained-unsupported-runtime-model | runtime-model | no-exported-surface |
| @solid-devtools/extension-adapter@0.12.1\|solid1\|only | retained-unsupported-runtime-model | runtime-model | success |
| @solid-primitives/animation@1.0.0-next.1\|solid2\|floor | retained-upstream-missing-bytes | upstream-package | published-target-missing |
| @solid-primitives/animation@1.0.0-next.1\|solid2\|head | retained-upstream-missing-bytes | upstream-package | published-target-missing |
| @solid-primitives/composites@1.1.1\|solid1\|only | retained-upstream-missing-bytes | upstream-package | published-target-missing |
| @solid-primitives/context@0.3.2\|solid1\|only | confirmed-upstream-declaration-defect | upstream-package | authenticated-dependency-layout-required |
| @solid-primitives/countdown@1.0.9\|solid1\|only | retained-unsupported-runtime-model | runtime-model | success |
| @solid-primitives/date-difference@1.0.2\|solid1\|only | retained-unsupported-runtime-model | runtime-model | success |
| @solid-primitives/geolocation@1.5.5\|solid1\|only | pending-phase21-checker-work | checker-semantic-model | success |
| @solid-primitives/reducer@0.0.101\|solid1\|only | retained-unsupported-runtime-model | runtime-model | success |
| @solid-primitives/until@0.1.1\|solid1\|only | retained-unsupported-runtime-model | runtime-model | success |
| @solid-primitives/workers@0.4.3\|solid1\|only | retained-upstream-missing-bytes | upstream-package | published-declaration-closure-missing |
| @solidjs/testing-library@0.8.10\|solid1\|only | exact-refusal-authenticated-layout | authenticated-dependency-layout | dependency-contract-obligation |
| @tanstack/solid-db@0.2.40\|solid1\|only | exact-refusal-semantic-model | checker-semantic-model | dependency-contract-obligation |
| @tanstack/solid-form@2.0.0-alpha.2\|solid1\|only | exact-refusal-semantic-model | checker-semantic-model | dependency-contract-obligation |
| @tanstack/solid-hotkeys@0.10.0\|solid1\|only | pending-phase21-checker-work | checker-semantic-model | dependency-contract-obligation |
| @tanstack/solid-query@5.102.5\|solid1\|only | exact-refusal-semantic-model | checker-semantic-model | dependency-contract-obligation |
| @tanstack/solid-query@6.0.0-rc.0\|solid2\|floor | exact-refusal-authenticated-layout | authenticated-dependency-layout | dependency-contract-obligation |
| @tanstack/solid-query@6.0.0-rc.0\|solid2\|head | exact-refusal-authenticated-layout | authenticated-dependency-layout | dependency-contract-obligation |
| @tanstack/solid-query-persist-client@5.102.5\|solid1\|only | exact-refusal-semantic-model | checker-semantic-model | dependency-contract-obligation |
| @tanstack/solid-query-persist-client@6.0.0-rc.0\|solid2\|floor | exact-refusal-authenticated-layout | authenticated-dependency-layout | dependency-contract-obligation |
| @tanstack/solid-query-persist-client@6.0.0-rc.0\|solid2\|head | exact-refusal-authenticated-layout | authenticated-dependency-layout | dependency-contract-obligation |
| @tanstack/solid-start-server@1.167.36\|solid1\|only | exact-refusal-package-import-resolution | checker-resolver | dependency-contract-obligation |
| @tanstack/solid-start-server@2.0.0-rc.2\|solid2\|floor | exact-refusal-package-import-resolution | checker-resolver | dependency-contract-obligation |
| @tanstack/solid-start-server@2.0.0-rc.2\|solid2\|head | exact-refusal-package-import-resolution | checker-resolver | dependency-contract-obligation |
| @tanstack/solid-store@0.11.1\|solid1\|only | exact-refusal-type-facts-capability | checker-type-facts | dependency-contract-obligation |
| @tanstack/solid-virtual@3.13.37\|solid1\|only | pending-phase21-checker-work | checker-type-facts | dependency-contract-obligation |
| corvu@0.7.2\|solid1\|only | verified-through-ordinary-receipt-load | none | dependency-contract-obligation |
