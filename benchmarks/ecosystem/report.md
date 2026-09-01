# Ecosystem Benchmark Report

- Started: 2026-09-01T00:49:14.796Z
- Finished: 2026-09-01T00:53:19.796Z
- Duration: 245000 ms
- Checker native binary: /Users/thomas/Documents/Github/solid-checker/rust/target/release/solid-checker-rust
- Type Facts binary: /Users/thomas/Documents/Github/solid-checker/bin/solid-typefacts
- Manifest generated at: 2026-08-26T14:21:49.573Z (rows: 307, probes: 418)
- Scope: full corpus (418 probes run)

## Solid 1.x

### Official Solid

- Compatible packages: 6
- Probes run: 6
- Declared entrypoints: 44
- Generated entrypoints: 32
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 61
- Inapplicable artifact cases (recorded, not refused): 3
- Success (complete contracts): 2/6 (33.33%)
- Partial contracts: 3
- Failures: 1

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @solidjs/image | 0.1.0 | only | partial-success | partial-success |
| @solidjs/meta | 0.29.4 | only | success | success |
| @solidjs/router | 1.0.0 | only | success | success |
| @solidjs/start | 2.0.3 | only | partial-success | partial-success |
| @solidjs/testing-library | 0.8.10 | only | failure | dependency-contract-obligation |
| solid-js | 1.9.14 | only | partial-success | partial-success |

Failure groups:
- 1x dependency-contract-obligation: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@testing-library/dom solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @solidjs/testing-library)

Failure details:
- **@solidjs/testing-library@0.8.10** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@testing-library/dom solid-checker-rust: emit package contract: cannot statically expand external export-all "@testing-library/dom" from /private<package-root>/dist/index.js; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-c... _(stderr truncated for readability)_

### Kobalte

- Compatible packages: 4
- Probes run: 4
- Declared entrypoints: 14
- Generated entrypoints: 560
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 117
- Inapplicable artifact cases (recorded, not refused): 94
- Success (complete contracts): 0/4 (0%)
- Partial contracts: 3
- Failures: 1

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @kobalte/core | 0.13.13 | only | partial-success | partial-success |
| @kobalte/solidbase | 0.6.13 | only | partial-success | partial-success |
| @kobalte/themes | 0.0.1-next.0 | only | failure | unavailable-published-target |
| @kobalte/utils | 0.9.2 | only | partial-success | partial-success |

Failure groups:
- 1x unavailable-published-target: no certifiable artifact case; 2 case(s) refused; first refusal: .: resolved <callee> <package-root>/dist/index.jsx is not a file (packages: @kobalte/themes)

Failure details:
- **@kobalte/themes@0.0.1-next.0** (only, unavailable-published-target): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: resolved target <package-root>/dist/index.jsx is not a file

### Solid Primitives

- Compatible packages: 97
- Probes run: 97
- Declared entrypoints: 94
- Generated entrypoints: 95
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 3
- Inapplicable artifact cases (recorded, not refused): 85
- Success (complete contracts): 92/97 (94.85%)
- Partial contracts: 2
- Failures: 3

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @solid-primitives/active-element | 2.1.6 | only | success | success |
| @solid-primitives/analytics | 0.2.1 | only | success | success |
| @solid-primitives/audio | 1.4.5 | only | success | success |
| @solid-primitives/autofocus | 0.1.5 | only | success | success |
| @solid-primitives/bounds | 0.1.7 | only | success | success |
| @solid-primitives/broadcast-channel | 0.1.1 | only | success | success |
| @solid-primitives/clipboard | 1.6.6 | only | success | success |
| @solid-primitives/composites | 1.1.1 | only | failure | unavailable-published-target |
| @solid-primitives/connectivity | 0.4.6 | only | success | success |
| @solid-primitives/context | 0.3.2 | only | failure | missing-closure-module |
| @solid-primitives/controlled-props | 0.1.4 | only | success | success |
| @solid-primitives/cookies | 0.0.3 | only | success | success |
| @solid-primitives/cookies-store | 1.1.11 | only | success | success |
| @solid-primitives/countdown | 1.0.9 | only | success | success |
| @solid-primitives/cursor | 0.1.4 | only | success | success |
| @solid-primitives/date | 2.1.8 | only | success | success |
| @solid-primitives/date-difference | 1.0.2 | only | success | success |
| @solid-primitives/db-store | 1.1.4 | only | success | success |
| @solid-primitives/debounce | 1.3.0 | only | success | success |
| @solid-primitives/deep | 0.3.7 | only | success | success |
| @solid-primitives/destructure | 0.2.4 | only | success | success |
| @solid-primitives/devices | 1.3.1 | only | success | success |
| @solid-primitives/event-bus | 1.1.4 | only | success | success |
| @solid-primitives/event-dispatcher | 0.1.1 | only | success | success |
| @solid-primitives/event-listener | 2.4.6 | only | success | success |
| @solid-primitives/event-props | 0.3.1 | only | success | success |
| @solid-primitives/fetch | 2.5.2 | only | success | success |
| @solid-primitives/filesystem | 1.3.4 | only | success | success |
| @solid-primitives/flux-store | 0.1.1 | only | success | success |
| @solid-primitives/fullscreen | 1.3.5 | only | success | success |
| @solid-primitives/geolocation | 1.5.5 | only | success | success |
| @solid-primitives/gestures | 1.2.1 | only | success | success |
| @solid-primitives/graphql | 3.0.0-next.0 | only | success | success |
| @solid-primitives/history | 0.2.5 | only | success | success |
| @solid-primitives/i18n | 2.2.1 | only | success | success |
| @solid-primitives/idle | 0.2.3 | only | success | success |
| @solid-primitives/immutable | 2.0.0-next.0 | only | success | success |
| @solid-primitives/input-mask | 0.3.1 | only | success | success |
| @solid-primitives/intersection-observer | 2.2.5 | only | success | success |
| @solid-primitives/jsx-parser | 0.2.0 | only | success | success |
| @solid-primitives/jsx-tokenizer | 1.1.4 | only | success | success |
| @solid-primitives/keyboard | 1.3.7 | only | success | success |
| @solid-primitives/keyed | 1.5.3 | only | success | success |
| @solid-primitives/lifecycle | 0.1.2 | only | success | success |
| @solid-primitives/list | 0.1.2 | only | success | success |
| @solid-primitives/local-store | 1.1.4 | only | success | success |
| @solid-primitives/map | 0.7.4 | only | success | success |
| @solid-primitives/marker | 0.2.2 | only | success | success |
| @solid-primitives/masonry | 0.1.4 | only | success | success |
| @solid-primitives/match | 0.0.100 | only | success | success |
| @solid-primitives/media | 2.3.6 | only | success | success |
| @solid-primitives/memo | 1.5.1 | only | success | success |
| @solid-primitives/mouse | 2.1.7 | only | success | success |
| @solid-primitives/mutable | 1.1.1 | only | success | success |
| @solid-primitives/mutation-observer | 1.2.4 | only | success | success |
| @solid-primitives/page-visibility | 2.1.6 | only | success | success |
| @solid-primitives/pagination | 0.5.2 | only | success | success |
| @solid-primitives/permission | 1.3.2 | only | success | success |
| @solid-primitives/platform | 0.2.1 | only | success | success |
| @solid-primitives/pointer | 0.3.6 | only | success | success |
| @solid-primitives/presence | 0.1.4 | only | success | success |
| @solid-primitives/promise | 1.1.4 | only | success | success |
| @solid-primitives/props | 3.2.4 | only | success | success |
| @solid-primitives/raf | 2.3.5 | only | success | success |
| @solid-primitives/range | 0.2.5 | only | success | success |
| @solid-primitives/reducer | 0.0.101 | only | success | success |
| @solid-primitives/refs | 1.1.4 | only | success | success |
| @solid-primitives/resize-observer | 2.2.0 | only | success | success |
| @solid-primitives/resource | 0.4.3 | only | success | success |
| @solid-primitives/rootless | 1.5.4 | only | success | success |
| @solid-primitives/scheduled | 1.5.3 | only | success | success |
| @solid-primitives/script-loader | 2.3.2 | only | success | success |
| @solid-primitives/scroll | 2.1.6 | only | success | success |
| @solid-primitives/selection | 0.1.3 | only | success | success |
| @solid-primitives/set | 0.7.4 | only | success | success |
| @solid-primitives/share | 2.2.5 | only | success | success |
| @solid-primitives/signal-builders | 0.2.4 | only | success | success |
| @solid-primitives/spring | 0.1.2 | only | success | success |
| @solid-primitives/sse | 0.0.103 | only | partial-success | partial-success |
| @solid-primitives/start | 0.0.4 | only | success | success |
| @solid-primitives/state-machine | 0.1.1 | only | success | success |
| @solid-primitives/static-store | 0.1.4 | only | success | success |
| @solid-primitives/storage | 4.4.0 | only | success | success |
| @solid-primitives/stream | 0.7.4 | only | success | success |
| @solid-primitives/styles | 0.1.4 | only | success | success |
| @solid-primitives/throttle | 1.2.0 | only | success | success |
| @solid-primitives/timer | 1.4.4 | only | success | success |
| @solid-primitives/transition-group | 1.1.2 | only | success | success |
| @solid-primitives/trigger | 1.2.4 | only | success | success |
| @solid-primitives/tween | 1.4.1 | only | success | success |
| @solid-primitives/until | 0.1.1 | only | success | success |
| @solid-primitives/upload | 0.1.5 | only | success | success |
| @solid-primitives/utils | 6.4.1 | only | partial-success | partial-success |
| @solid-primitives/virtual | 0.2.5 | only | success | success |
| @solid-primitives/visibility-observer | 2.0.1 | only | success | success |
| @solid-primitives/websocket | 1.4.0 | only | success | success |
| @solid-primitives/workers | 0.4.3 | only | failure | missing-closure-module |

Failure groups:
- 1x unavailable-published-target: no certifiable artifact case; 1 case(s) refused; first refusal: .: resolved <callee> <package-root>/dist/index.cjs is not a file (packages: @solid-primitives/composites)
- 1x missing-closure-module: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: local closure module ../node_modules/solid-js/types/reactive/signal.js from <package-root>/dist/index.d.ts was not found (packages: @solid-primitives/context)
- 1x missing-closure-module: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: local closure module ./types.js from <package-root>/dist/index.d.ts was not found (packages: @solid-primitives/workers)

Failure details:
- **@solid-primitives/composites@1.1.1** (only, unavailable-published-target): solid-checker: no certifiable artifact case; 1 case(s) refused; first refusal: .: resolved target <package-root>/dist/index.cjs is not a file
- **@solid-primitives/context@0.3.2** (only, missing-closure-module): solid-checker: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: local closure module ../node_modules/solid-js/types/reactive/signal.js from <package-root>/dist/index.d.ts was not found
- **@solid-primitives/workers@0.4.3** (only, missing-closure-module): solid-checker: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: local closure module ./types.js from <package-root>/dist/index.d.ts was not found

### Corvu

- Compatible packages: 11
- Probes run: 11
- Declared entrypoints: 14
- Generated entrypoints: 23
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 0
- Inapplicable artifact cases (recorded, not refused): 0
- Success (complete contracts): 7/11 (63.64%)
- Partial contracts: 0
- Failures: 4

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @corvu/accordion | 0.2.5 | only | failure | dependency-contract-obligation |
| @corvu/calendar | 0.1.2 | only | success | success |
| @corvu/dialog | 0.2.4 | only | success | success |
| @corvu/disclosure | 0.2.2 | only | success | success |
| @corvu/drawer | 0.2.4 | only | failure | dependency-contract-obligation |
| @corvu/otp-field | 0.1.4 | only | success | success |
| @corvu/popover | 0.2.0 | only | failure | dependency-contract-obligation |
| @corvu/resizable | 0.2.5 | only | success | success |
| @corvu/tooltip | 0.2.2 | only | success | success |
| @corvu/utils | 0.4.2 | only | success | success |
| corvu | 0.7.2 | only | failure | dependency-contract-obligation |

Failure groups:
- 2x dependency-contract-obligation: no certifiable artifact case; 2 case(s) refused; first refusal: .: accepted dependency @corvu/dialog has no exact runtime binding for export Portal (packages: @corvu/drawer, @corvu/popover)
- 1x dependency-contract-obligation: no certifiable artifact case; 18 case(s) refused; first refusal: ./accordion: accepted dependency @corvu/accordion has no exact runtime binding for export default (packages: corvu)
- 1x dependency-contract-obligation: no certifiable artifact case; 2 case(s) refused; first refusal: .: accepted dependency @corvu/disclosure has no exact runtime binding for export useContext (packages: @corvu/accordion)

Failure details:
- **@corvu/accordion@0.2.5** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: accepted dependency @corvu/disclosure has no exact runtime binding for export useContext
- **@corvu/drawer@0.2.4** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: accepted dependency @corvu/dialog has no exact runtime binding for export Portal
- **@corvu/popover@0.2.0** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: accepted dependency @corvu/dialog has no exact runtime binding for export Portal
- **corvu@0.7.2** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 18 case(s) refused; first refusal: ./accordion: accepted dependency @corvu/accordion has no exact runtime binding for export default

### TanStack

- Compatible packages: 36
- Probes run: 36
- Declared entrypoints: 230
- Generated entrypoints: 44
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 34
- Inapplicable artifact cases (recorded, not refused): 20
- Success (complete contracts): 22/36 (61.11%)
- Partial contracts: 4
- Failures: 10

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @tanstack/ai-devtools-core | 0.5.8 | only | success | success |
| @tanstack/ai-solid | 0.19.1 | only | failure | dependency-contract-obligation |
| @tanstack/ai-solid-ui | 0.7.20 | only | failure | dependency-contract-obligation |
| @tanstack/charts | 0.15.0 | only | success | success |
| @tanstack/devtools | 0.14.2 | only | success | success |
| @tanstack/devtools-a11y | 0.2.2 | only | success | success |
| @tanstack/devtools-ui | 0.7.1 | only | success | success |
| @tanstack/devtools-utils | 0.7.0 | only | success | success |
| @tanstack/form-devtools | 1.0.0-alpha.2 | only | success | success |
| @tanstack/hotkeys-devtools | 0.9.0 | only | success | success |
| @tanstack/pacer-devtools | 1.4.0 | only | success | success |
| @tanstack/solid-ai-devtools | 0.2.71 | only | success | success |
| @tanstack/solid-charts | 0.15.0 | only | success | success |
| @tanstack/solid-db | 0.2.40 | only | failure | dependency-contract-obligation |
| @tanstack/solid-devtools | 0.8.12 | only | success | success |
| @tanstack/solid-form | 2.0.0-alpha.2 | only | failure | dependency-contract-obligation |
| @tanstack/solid-form-devtools | 1.0.0-alpha.2 | only | success | success |
| @tanstack/solid-hotkeys | 0.10.0 | only | failure | dependency-contract-obligation |
| @tanstack/solid-hotkeys-devtools | 0.7.0 | only | success | success |
| @tanstack/solid-pacer | 0.22.0 | only | partial-success | partial-success |
| @tanstack/solid-pacer-devtools | 0.14.0 | only | success | success |
| @tanstack/solid-query | 5.102.5 | only | failure | dependency-contract-obligation |
| @tanstack/solid-query-devtools | 5.102.5 | only | success | success |
| @tanstack/solid-query-persist-client | 5.102.5 | only | failure | dependency-contract-obligation |
| @tanstack/solid-router | 1.170.30 | only | partial-success | partial-success |
| @tanstack/solid-router-devtools | 1.167.1 | only | success | success |
| @tanstack/solid-router-ssr-query | 1.167.2-pre.0 | only | success | success |
| @tanstack/solid-start | 1.168.47 | only | partial-success | partial-success |
| @tanstack/solid-start-client | 1.168.29 | only | success | success |
| @tanstack/solid-start-config | 1.120.20 | only | success | success |
| @tanstack/solid-start-server | 1.167.36 | only | failure | dependency-contract-obligation |
| @tanstack/solid-store | 0.11.1 | only | failure | dependency-contract-obligation |
| @tanstack/solid-table | 9.1.2 | only | partial-success | partial-success |
| @tanstack/solid-table-devtools | 9.2.0 | only | success | success |
| @tanstack/solid-virtual | 3.13.37 | only | failure | dependency-contract-obligation |
| @tanstack/table-devtools | 9.2.0 | only | success | success |

Failure groups:
- 1x dependency-contract-obligation: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: accepted dependency @tanstack/db has no exact runtime binding for export createTransaction (packages: @tanstack/solid-db)
- 1x dependency-contract-obligation: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/form-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-form)
- 1x dependency-contract-obligation: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/hotkeys solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-hotkeys)
- 1x dependency-contract-obligation: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/start-server-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-start-server)
- 1x dependency-contract-obligation: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/store solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-store)
- 1x dependency-contract-obligation: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/virtual-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-virtual)
- 1x dependency-contract-obligation: no certifiable artifact case; 1 case(s) refused; first refusal: .: accepted dependency @tanstack/ai-client has no exact runtime binding for export StorageUnavailableError (packages: @tanstack/ai-solid)
- 1x dependency-contract-obligation: no certifiable artifact case; 2 case(s) refused; first refusal: .: accepted dependency @tanstack/ai-solid has no exact runtime binding for export useChat (packages: @tanstack/ai-solid-ui)
- 1x dependency-contract-obligation: no certifiable artifact case; 3 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/query-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-query)
- 1x dependency-contract-obligation: no certifiable artifact case; 3 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/query-persist-client-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-query-persist-client)

Failure details:
- **@tanstack/ai-solid@0.19.1** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 1 case(s) refused; first refusal: .: accepted dependency @tanstack/ai-client has no exact runtime binding for export StorageUnavailableError
- **@tanstack/ai-solid-ui@0.7.20** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: accepted dependency @tanstack/ai-solid has no exact runtime binding for export useChat
- **@tanstack/solid-db@0.2.40** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: accepted dependency @tanstack/db has no exact runtime binding for export createTransaction
- **@tanstack/solid-form@2.0.0-alpha.2** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/form-core solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/form-core" from /private<package-root>/dist/index.js; acquire a verified dependency contract and pass its receipt-issued... _(stderr truncated for readability)_
- **@tanstack/solid-hotkeys@0.10.0** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/hotkeys solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/hotkeys" from /private<package-root>/dist/index.js; acquire a verified dependency contract and pass its receipt-issued exa... _(stderr truncated for readability)_
- **@tanstack/solid-query@5.102.5** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 3 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/query-core solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/query-core" from /private<package-root>/build/index.js; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-... _(stderr truncated for readability)_
- **@tanstack/solid-query-persist-client@5.102.5** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 3 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/query-persist-client-core solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/query-persist-client-core" from /private<package-root>/build/index.js; acquire a verified dependency contract and pass its receipt-issued ex... _(stderr truncated for readability)_
- **@tanstack/solid-start-server@1.167.36** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/start-server-core solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/start-server-core" from /private<package-root>/dist/esm/index.js; acquire a verified dependency contract and pas... _(stderr truncated for readability)_
- **@tanstack/solid-store@0.11.1** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/store solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/store" from /private<package-root>/dist/index.js; acquire a verified dependency contract and pass its receipt-issued exact i... _(stderr truncated for readability)_
- **@tanstack/solid-virtual@3.13.37** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/virtual-core solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/virtual-core" from /private<package-root>/dist/esm/index.js; acquire a verified dependency contract and pass its rece... _(stderr truncated for readability)_

### Solid Devtools

- Compatible packages: 12
- Probes run: 12
- Declared entrypoints: 21
- Generated entrypoints: 19
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 43
- Inapplicable artifact cases (recorded, not refused): 44
- Success (complete contracts): 6/12 (50%)
- Partial contracts: 4
- Failures: 2

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @solid-devtools/babel-plugin | 0.3.1 | only | failure | no-exported-surface |
| @solid-devtools/debugger | 0.28.1 | only | partial-success | partial-success |
| @solid-devtools/ext-adapter | 0.17.0 | only | failure | no-exported-surface |
| @solid-devtools/extension-adapter | 0.12.1 | only | success | success |
| @solid-devtools/frontend | 0.15.4 | only | success | success |
| @solid-devtools/locator | 0.16.7 | only | partial-success | partial-success |
| @solid-devtools/logger | 0.9.11 | only | success | success |
| @solid-devtools/overlay | 0.33.5 | only | success | success |
| @solid-devtools/shared | 0.20.0 | only | partial-success | partial-success |
| @solid-devtools/transform | 0.10.4 | only | success | success |
| @solid-devtools/ui | 0.10.3 | only | success | success |
| solid-devtools | 0.34.5 | only | partial-success | partial-success |

Failure groups:
- 2x no-exported-surface: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file has no runtime ESM exports (packages: @solid-devtools/babel-plugin, @solid-devtools/ext-adapter)

Failure details:
- **@solid-devtools/babel-plugin@0.3.1** (only, no-exported-surface): solid-checker: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file /private<package-root>/dist/index.js has no runtime ESM exports
- **@solid-devtools/ext-adapter@0.17.0** (only, no-exported-surface): solid-checker: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file /private<package-root>/dist/index.js has no runtime ESM exports

### Solid Recharts

- Compatible packages: 1
- Probes run: 1
- Declared entrypoints: 1
- Generated entrypoints: 1
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 0
- Inapplicable artifact cases (recorded, not refused): 0
- Success (complete contracts): 1/1 (100%)
- Partial contracts: 0
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| solid-recharts | 1.0.1 | only | success | success |

### Motion for Solid

- Compatible packages: 1
- Probes run: 1
- Declared entrypoints: 2
- Generated entrypoints: 0
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 0
- Inapplicable artifact cases (recorded, not refused): 0
- Success (complete contracts): 0/1 (0%)
- Partial contracts: 0
- Failures: 1

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| motion-solidjs | 0.6.0 | only | failure | dependency-contract-obligation |

Failure groups:
- 1x dependency-contract-obligation: no certifiable artifact case; 2 case(s) refused; first refusal: .: accepted dependency motion-utils has no exact runtime binding for export MotionGlobalConfig (packages: motion-solidjs)

Failure details:
- **motion-solidjs@0.6.0** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: accepted dependency motion-utils has no exact runtime binding for export MotionGlobalConfig

**Solid 1.x totals:** 130/168 (77.38%) complete, 16 partial, 22 failed

## Solid 2.x

### Official Solid

- Compatible packages: 12
- Probes run: 15
- Declared entrypoints: 46
- Generated entrypoints: 30
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 68
- Inapplicable artifact cases (recorded, not refused): 3
- Success (complete contracts): 7/15 (46.67%)
- Partial contracts: 7
- Failures: 1

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @solidjs/diagnostics | 2.0.0-rc.3 | only | partial-success | partial-success |
| @solidjs/element | 2.0.0-rc.3 | only | failure | dependency-contract-obligation |
| @solidjs/h | 2.0.0-rc.3 | only | partial-success | partial-success |
| @solidjs/html | 2.0.0-rc.3 | only | success | success |
| @solidjs/meta | 1.0.0-next.2 | floor | success | success |
| @solidjs/meta | 1.0.0-next.2 | head | success | success |
| @solidjs/router | 2.0.0-next.18 | only | success | success |
| @solidjs/signals | 2.0.0-rc.3 | only | success | success |
| @solidjs/start-devtools | 1.0.0-next.4 | floor | success | success |
| @solidjs/start-devtools | 1.0.0-next.4 | head | success | success |
| @solidjs/universal | 2.0.0-rc.3 | only | partial-success | partial-success |
| @solidjs/vite-plugin | 3.0.0-next.34 | floor | partial-success | partial-success |
| @solidjs/vite-plugin | 3.0.0-next.34 | head | partial-success | partial-success |
| @solidjs/web | 2.0.0-rc.3 | only | partial-success | partial-success |
| solid-js | 2.0.0-rc.3 | only | partial-success | partial-success |

Failure groups:
- 1x dependency-contract-obligation: no certifiable artifact case; 1 case(s) refused; first refusal: .: accepted dependency component-register has no exact runtime binding for export getCurrentElement (packages: @solidjs/element)

Failure details:
- **@solidjs/element@2.0.0-rc.3** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 1 case(s) refused; first refusal: .: accepted dependency component-register has no exact runtime binding for export getCurrentElement

### Kobalte

- Compatible packages: 2
- Probes run: 2
- Declared entrypoints: 3
- Generated entrypoints: 65
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 7
- Inapplicable artifact cases (recorded, not refused): 0
- Success (complete contracts): 0/2 (0%)
- Partial contracts: 2
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @kobalte/core | 2.0.0-alpha.0 | only | partial-success | partial-success |
| @kobalte/utils | 2.0.0-alpha.0 | only | partial-success | partial-success |

### Solid Primitives

- Compatible packages: 97
- Probes run: 194
- Declared entrypoints: 212
- Generated entrypoints: 202
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 8
- Inapplicable artifact cases (recorded, not refused): 206
- Success (complete contracts): 182/194 (93.81%)
- Partial contracts: 6
- Failures: 6

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @solid-primitives/a11y | 1.0.0-next.3 | floor | success | success |
| @solid-primitives/a11y | 1.0.0-next.3 | head | success | success |
| @solid-primitives/active-element | 3.0.0-next.2 | floor | success | success |
| @solid-primitives/active-element | 3.0.0-next.2 | head | success | success |
| @solid-primitives/analytics | 2.0.0-next.2 | floor | success | success |
| @solid-primitives/analytics | 2.0.0-next.2 | head | success | success |
| @solid-primitives/animation | 1.0.0-next.1 | floor | failure | unavailable-published-target |
| @solid-primitives/animation | 1.0.0-next.1 | head | failure | unavailable-published-target |
| @solid-primitives/async | 0.0.101-next.3 | floor | success | success |
| @solid-primitives/async | 0.0.101-next.3 | head | success | success |
| @solid-primitives/audio | 3.0.0-next.2 | floor | success | success |
| @solid-primitives/audio | 3.0.0-next.2 | head | success | success |
| @solid-primitives/bounds | 1.0.0-next.2 | floor | success | success |
| @solid-primitives/bounds | 1.0.0-next.2 | head | success | success |
| @solid-primitives/broadcast-channel | 1.0.0-next.2 | floor | success | success |
| @solid-primitives/broadcast-channel | 1.0.0-next.2 | head | success | success |
| @solid-primitives/clipboard | 2.0.0-next.17 | floor | success | success |
| @solid-primitives/clipboard | 2.0.0-next.17 | head | success | success |
| @solid-primitives/connectivity | 1.0.0-next.2 | floor | success | success |
| @solid-primitives/connectivity | 1.0.0-next.2 | head | success | success |
| @solid-primitives/context | 2.0.0-next.2 | floor | success | success |
| @solid-primitives/context | 2.0.0-next.2 | head | success | success |
| @solid-primitives/controlled-props | 1.0.0-next.3 | floor | partial-success | partial-success |
| @solid-primitives/controlled-props | 1.0.0-next.3 | head | partial-success | partial-success |
| @solid-primitives/controlled-signal | 1.0.0-next.3 | floor | success | success |
| @solid-primitives/controlled-signal | 1.0.0-next.3 | head | success | success |
| @solid-primitives/cookies | 1.0.0-next.2 | floor | success | success |
| @solid-primitives/cookies | 1.0.0-next.2 | head | success | success |
| @solid-primitives/cursor | 1.0.0-next.2 | floor | success | success |
| @solid-primitives/cursor | 1.0.0-next.2 | head | success | success |
| @solid-primitives/date | 3.0.0-next.3 | floor | success | success |
| @solid-primitives/date | 3.0.0-next.3 | head | success | success |
| @solid-primitives/deep | 1.0.0-next.3 | floor | success | success |
| @solid-primitives/deep | 1.0.0-next.3 | head | success | success |
| @solid-primitives/destructure | 1.0.0-next.2 | floor | success | success |
| @solid-primitives/destructure | 1.0.0-next.2 | head | success | success |
| @solid-primitives/devices | 3.0.0-next.2 | floor | success | success |
| @solid-primitives/devices | 3.0.0-next.2 | head | success | success |
| @solid-primitives/drag-drop | 0.1.0-next.0 | floor | success | success |
| @solid-primitives/drag-drop | 0.1.0-next.0 | head | success | success |
| @solid-primitives/event-bus | 3.0.0-next.3 | floor | success | success |
| @solid-primitives/event-bus | 3.0.0-next.3 | head | success | success |
| @solid-primitives/event-dispatcher | 1.0.0-next.2 | floor | success | success |
| @solid-primitives/event-dispatcher | 1.0.0-next.2 | head | success | success |
| @solid-primitives/event-listener | 3.0.0-next.3 | floor | success | success |
| @solid-primitives/event-listener | 3.0.0-next.3 | head | success | success |
| @solid-primitives/event-props | 1.0.0-next.2 | floor | success | success |
| @solid-primitives/event-props | 1.0.0-next.2 | head | success | success |
| @solid-primitives/favicon | 1.0.0-next.1 | floor | success | success |
| @solid-primitives/favicon | 1.0.0-next.1 | head | success | success |
| @solid-primitives/filesystem | 3.0.0-next.3 | floor | success | success |
| @solid-primitives/filesystem | 3.0.0-next.3 | head | success | success |
| @solid-primitives/flux-store | 1.0.0-next.2 | floor | success | success |
| @solid-primitives/flux-store | 1.0.0-next.2 | head | success | success |
| @solid-primitives/focus | 1.0.0-next.4 | floor | success | success |
| @solid-primitives/focus | 1.0.0-next.4 | head | success | success |
| @solid-primitives/form | 1.0.0-next.2 | floor | failure | dependency-contract-obligation |
| @solid-primitives/form | 1.0.0-next.2 | head | failure | dependency-contract-obligation |
| @solid-primitives/fullscreen | 2.0.0-next.3 | floor | success | success |
| @solid-primitives/fullscreen | 2.0.0-next.3 | head | success | success |
| @solid-primitives/geolocation | 3.0.0-next.2 | floor | success | success |
| @solid-primitives/geolocation | 3.0.0-next.2 | head | success | success |
| @solid-primitives/gestures | 3.0.0-next.3 | floor | success | success |
| @solid-primitives/gestures | 3.0.0-next.3 | head | success | success |
| @solid-primitives/history | 1.0.0-next.3 | floor | success | success |
| @solid-primitives/history | 1.0.0-next.3 | head | success | success |
| @solid-primitives/i18n | 3.0.0-next.4 | floor | success | success |
| @solid-primitives/i18n | 3.0.0-next.4 | head | success | success |
| @solid-primitives/idle | 1.0.0-next.3 | floor | success | success |
| @solid-primitives/idle | 1.0.0-next.3 | head | success | success |
| @solid-primitives/input-mask | 1.0.0-next.2 | floor | success | success |
| @solid-primitives/input-mask | 1.0.0-next.2 | head | success | success |
| @solid-primitives/interaction | 1.0.0-next.4 | floor | success | success |
| @solid-primitives/interaction | 1.0.0-next.4 | head | success | success |
| @solid-primitives/intersection-observer | 3.0.0-next.3 | floor | failure | dependency-contract-obligation |
| @solid-primitives/intersection-observer | 3.0.0-next.3 | head | failure | dependency-contract-obligation |
| @solid-primitives/jsx-tokenizer | 3.0.0-next.2 | floor | success | success |
| @solid-primitives/jsx-tokenizer | 3.0.0-next.2 | head | success | success |
| @solid-primitives/keyboard | 2.0.0-next.5 | floor | success | success |
| @solid-primitives/keyboard | 2.0.0-next.5 | head | success | success |
| @solid-primitives/keyed | 3.0.0-next.2 | floor | success | success |
| @solid-primitives/keyed | 3.0.0-next.2 | head | success | success |
| @solid-primitives/lifecycle | 1.0.0-next.2 | floor | success | success |
| @solid-primitives/lifecycle | 1.0.0-next.2 | head | success | success |
| @solid-primitives/list | 1.0.0-next.2 | floor | success | success |
| @solid-primitives/list | 1.0.0-next.2 | head | success | success |
| @solid-primitives/list-state | 1.0.0-next.2 | floor | success | success |
| @solid-primitives/list-state | 1.0.0-next.2 | head | success | success |
| @solid-primitives/map | 1.0.0-next.2 | floor | success | success |
| @solid-primitives/map | 1.0.0-next.2 | head | success | success |
| @solid-primitives/marker | 2.0.0-next.2 | floor | success | success |
| @solid-primitives/marker | 2.0.0-next.2 | head | success | success |
| @solid-primitives/masonry | 2.0.0-next.2 | floor | success | success |
| @solid-primitives/masonry | 2.0.0-next.2 | head | success | success |
| @solid-primitives/match | 1.0.0-next.2 | floor | success | success |
| @solid-primitives/match | 1.0.0-next.2 | head | success | success |
| @solid-primitives/media | 4.0.0-next.2 | floor | success | success |
| @solid-primitives/media | 4.0.0-next.2 | head | success | success |
| @solid-primitives/mediastream | 1.0.0-next.2 | floor | success | success |
| @solid-primitives/mediastream | 1.0.0-next.2 | head | success | success |
| @solid-primitives/memo | 2.0.0-next.2 | floor | success | success |
| @solid-primitives/memo | 2.0.0-next.2 | head | success | success |
| @solid-primitives/mouse | 4.0.0-next.3 | floor | success | success |
| @solid-primitives/mouse | 4.0.0-next.3 | head | success | success |
| @solid-primitives/mutable | 3.0.0-next.2 | floor | success | success |
| @solid-primitives/mutable | 3.0.0-next.2 | head | success | success |
| @solid-primitives/mutation-observer | 3.0.0-next.2 | floor | success | success |
| @solid-primitives/mutation-observer | 3.0.0-next.2 | head | success | success |
| @solid-primitives/notification | 1.0.0-next.3 | floor | success | success |
| @solid-primitives/notification | 1.0.0-next.3 | head | success | success |
| @solid-primitives/orientation | 1.0.0-next.2 | floor | success | success |
| @solid-primitives/orientation | 1.0.0-next.2 | head | success | success |
| @solid-primitives/page-utilities | 3.0.0-next.2 | floor | success | success |
| @solid-primitives/page-utilities | 3.0.0-next.2 | head | success | success |
| @solid-primitives/pagination | 1.0.0-next.6 | floor | success | success |
| @solid-primitives/pagination | 1.0.0-next.6 | head | success | success |
| @solid-primitives/permission | 2.0.0-next.2 | floor | success | success |
| @solid-primitives/permission | 2.0.0-next.2 | head | success | success |
| @solid-primitives/platform | 1.0.0-next.2 | floor | success | success |
| @solid-primitives/platform | 1.0.0-next.2 | head | success | success |
| @solid-primitives/pointer | 1.0.0-next.2 | floor | success | success |
| @solid-primitives/pointer | 1.0.0-next.2 | head | success | success |
| @solid-primitives/presence | 1.0.0-next.2 | floor | success | success |
| @solid-primitives/presence | 1.0.0-next.2 | head | success | success |
| @solid-primitives/promise | 2.0.0-next.2 | floor | success | success |
| @solid-primitives/promise | 2.0.0-next.2 | head | success | success |
| @solid-primitives/props | 4.0.0-next.3 | floor | success | success |
| @solid-primitives/props | 4.0.0-next.3 | head | success | success |
| @solid-primitives/queue | 1.0.0-next.3 | floor | success | success |
| @solid-primitives/queue | 1.0.0-next.3 | head | success | success |
| @solid-primitives/raf | 4.0.0-next.2 | floor | success | success |
| @solid-primitives/raf | 4.0.0-next.2 | head | success | success |
| @solid-primitives/range | 1.0.0-next.3 | floor | success | success |
| @solid-primitives/range | 1.0.0-next.3 | head | success | success |
| @solid-primitives/refs | 3.0.0-next.2 | floor | success | success |
| @solid-primitives/refs | 3.0.0-next.2 | head | success | success |
| @solid-primitives/resize-observer | 4.0.0-next.3 | floor | success | success |
| @solid-primitives/resize-observer | 4.0.0-next.3 | head | success | success |
| @solid-primitives/rootless | 2.0.0-next.2 | floor | success | success |
| @solid-primitives/rootless | 2.0.0-next.2 | head | success | success |
| @solid-primitives/scheduled | 2.0.0-next.2 | floor | success | success |
| @solid-primitives/scheduled | 2.0.0-next.2 | head | success | success |
| @solid-primitives/script-loader | 3.0.0-next.2 | floor | success | success |
| @solid-primitives/script-loader | 3.0.0-next.2 | head | success | success |
| @solid-primitives/scroll | 3.0.0-next.4 | floor | success | success |
| @solid-primitives/scroll | 3.0.0-next.4 | head | success | success |
| @solid-primitives/selection | 1.0.0-next.2 | floor | success | success |
| @solid-primitives/selection | 1.0.0-next.2 | head | success | success |
| @solid-primitives/sensors | 1.0.0-next.3 | floor | success | success |
| @solid-primitives/sensors | 1.0.0-next.3 | head | success | success |
| @solid-primitives/set | 1.0.0-next.2 | floor | success | success |
| @solid-primitives/set | 1.0.0-next.2 | head | success | success |
| @solid-primitives/share | 4.0.0-next.4 | floor | success | success |
| @solid-primitives/share | 4.0.0-next.4 | head | success | success |
| @solid-primitives/signal-builders | 1.0.0-next.4 | floor | success | success |
| @solid-primitives/signal-builders | 1.0.0-next.4 | head | success | success |
| @solid-primitives/sortable | 1.0.0-next.0 | floor | success | success |
| @solid-primitives/sortable | 1.0.0-next.0 | head | success | success |
| @solid-primitives/spring | 1.0.0-next.3 | floor | success | success |
| @solid-primitives/spring | 1.0.0-next.3 | head | success | success |
| @solid-primitives/sse | 1.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/sse | 1.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/state-machine | 1.0.0-next.2 | floor | success | success |
| @solid-primitives/state-machine | 1.0.0-next.2 | head | success | success |
| @solid-primitives/static-store | 1.0.0-next.2 | floor | success | success |
| @solid-primitives/static-store | 1.0.0-next.2 | head | success | success |
| @solid-primitives/storage | 5.0.0-next.4 | floor | success | success |
| @solid-primitives/storage | 5.0.0-next.4 | head | success | success |
| @solid-primitives/styles | 1.0.0-next.2 | floor | success | success |
| @solid-primitives/styles | 1.0.0-next.2 | head | success | success |
| @solid-primitives/timer | 1.4.5-next.1 | floor | success | success |
| @solid-primitives/timer | 1.4.5-next.1 | head | success | success |
| @solid-primitives/transition-group | 2.0.0-next.2 | floor | success | success |
| @solid-primitives/transition-group | 2.0.0-next.2 | head | success | success |
| @solid-primitives/trigger | 3.0.0-next.2 | floor | success | success |
| @solid-primitives/trigger | 3.0.0-next.2 | head | success | success |
| @solid-primitives/tween | 2.0.0-next.2 | floor | success | success |
| @solid-primitives/tween | 2.0.0-next.2 | head | success | success |
| @solid-primitives/upload | 1.0.0-next.4 | floor | success | success |
| @solid-primitives/upload | 1.0.0-next.4 | head | success | success |
| @solid-primitives/url | 0.2.0-next.2 | floor | success | success |
| @solid-primitives/url | 0.2.0-next.2 | head | success | success |
| @solid-primitives/utils | 7.0.0-next.4 | floor | success | success |
| @solid-primitives/utils | 7.0.0-next.4 | head | success | success |
| @solid-primitives/vibrate | 1.0.0-next.2 | floor | success | success |
| @solid-primitives/vibrate | 1.0.0-next.2 | head | success | success |
| @solid-primitives/video | 1.0.0-next.3 | floor | success | success |
| @solid-primitives/video | 1.0.0-next.3 | head | success | success |
| @solid-primitives/virtual | 1.0.0-next.4 | floor | partial-success | partial-success |
| @solid-primitives/virtual | 1.0.0-next.4 | head | partial-success | partial-success |
| @solid-primitives/websocket | 2.0.0-next.3 | floor | success | success |
| @solid-primitives/websocket | 2.0.0-next.3 | head | success | success |
| @solid-primitives/workers | 2.0.1-next.1 | floor | success | success |
| @solid-primitives/workers | 2.0.1-next.1 | head | success | success |

Failure groups:
- 2x unavailable-published-target: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: resolved <callee> <package-root>/dist/index.js is not a file (packages: @solid-primitives/animation)
- 2x dependency-contract-obligation: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: accepted dependency @solid-primitives/a11y has no exact runtime binding for export FormControlContext (packages: @solid-primitives/form)
- 2x dependency-contract-obligation: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: accepted dependency solid-js has no exact runtime binding for export NotReadyError (packages: @solid-primitives/intersection-observer)

Failure details:
- **@solid-primitives/animation@1.0.0-next.1** (floor, unavailable-published-target): solid-checker: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: resolved target <package-root>/dist/index.js is not a file
- **@solid-primitives/animation@1.0.0-next.1** (head, unavailable-published-target): solid-checker: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: resolved target <package-root>/dist/index.js is not a file
- **@solid-primitives/form@1.0.0-next.2** (floor, dependency-contract-obligation): solid-checker: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: accepted dependency @solid-primitives/a11y has no exact runtime binding for export FormControlContext
- **@solid-primitives/form@1.0.0-next.2** (head, dependency-contract-obligation): solid-checker: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: accepted dependency @solid-primitives/a11y has no exact runtime binding for export FormControlContext
- **@solid-primitives/intersection-observer@3.0.0-next.3** (floor, dependency-contract-obligation): solid-checker: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: accepted dependency solid-js has no exact runtime binding for export NotReadyError
- **@solid-primitives/intersection-observer@3.0.0-next.3** (head, dependency-contract-obligation): solid-checker: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: accepted dependency solid-js has no exact runtime binding for export NotReadyError

### Corvu

- Compatible packages: 17
- Probes run: 17
- Declared entrypoints: 20
- Generated entrypoints: 31
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 0
- Inapplicable artifact cases (recorded, not refused): 0
- Success (complete contracts): 15/17 (88.24%)
- Partial contracts: 0
- Failures: 2

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @corvu-next/accordion | 0.1.5 | only | failure | dependency-contract-obligation |
| @corvu-next/calendar | 0.1.5 | only | success | success |
| @corvu-next/dialog | 0.1.5 | only | success | success |
| @corvu-next/disclosure | 0.1.5 | only | success | success |
| @corvu-next/dismissible | 0.1.5 | only | success | success |
| @corvu-next/drawer | 0.1.5 | only | success | success |
| @corvu-next/focus-trap | 0.1.5 | only | success | success |
| @corvu-next/list | 0.1.5 | only | success | success |
| @corvu-next/otp-field | 0.1.5 | only | success | success |
| @corvu-next/persistent | 0.1.5 | only | success | success |
| @corvu-next/popover | 0.1.5 | only | failure | dependency-contract-obligation |
| @corvu-next/presence | 0.1.5 | only | success | success |
| @corvu-next/prevent-scroll | 0.1.5 | only | success | success |
| @corvu-next/resizable | 0.1.5 | only | success | success |
| @corvu-next/tooltip | 0.1.5 | only | success | success |
| @corvu-next/transition-size | 0.1.5 | only | success | success |
| @corvu-next/utils | 0.1.5 | only | success | success |

Failure groups:
- 1x dependency-contract-obligation: no certifiable artifact case; 2 case(s) refused; first refusal: .: accepted dependency @corvu-next/dialog has no exact runtime binding for export Portal (packages: @corvu-next/popover)
- 1x dependency-contract-obligation: no certifiable artifact case; 2 case(s) refused; first refusal: .: accepted dependency @corvu-next/disclosure has no exact runtime binding for export useContext (packages: @corvu-next/accordion)

Failure details:
- **@corvu-next/accordion@0.1.5** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: accepted dependency @corvu-next/disclosure has no exact runtime binding for export useContext
- **@corvu-next/popover@0.1.5** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: accepted dependency @corvu-next/dialog has no exact runtime binding for export Portal

### TanStack

- Compatible packages: 9
- Probes run: 18
- Declared entrypoints: 60
- Generated entrypoints: 20
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 26
- Inapplicable artifact cases (recorded, not refused): 10
- Success (complete contracts): 8/18 (44.44%)
- Partial contracts: 4
- Failures: 6

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @tanstack/solid-query | 6.0.0-rc.0 | floor | failure | dependency-contract-obligation |
| @tanstack/solid-query | 6.0.0-rc.0 | head | failure | dependency-contract-obligation |
| @tanstack/solid-query-devtools | 6.0.0-rc.0 | floor | success | success |
| @tanstack/solid-query-devtools | 6.0.0-rc.0 | head | success | success |
| @tanstack/solid-query-persist-client | 6.0.0-rc.0 | floor | failure | dependency-contract-obligation |
| @tanstack/solid-query-persist-client | 6.0.0-rc.0 | head | failure | dependency-contract-obligation |
| @tanstack/solid-router | 2.0.0-rc.2 | floor | partial-success | partial-success |
| @tanstack/solid-router | 2.0.0-rc.2 | head | partial-success | partial-success |
| @tanstack/solid-router-devtools | 2.0.0-rc.2 | floor | success | success |
| @tanstack/solid-router-devtools | 2.0.0-rc.2 | head | success | success |
| @tanstack/solid-router-ssr-query | 2.0.0-rc.2 | floor | success | success |
| @tanstack/solid-router-ssr-query | 2.0.0-rc.2 | head | success | success |
| @tanstack/solid-start | 2.0.0-rc.2 | floor | partial-success | partial-success |
| @tanstack/solid-start | 2.0.0-rc.2 | head | partial-success | partial-success |
| @tanstack/solid-start-client | 2.0.0-rc.2 | floor | success | success |
| @tanstack/solid-start-client | 2.0.0-rc.2 | head | success | success |
| @tanstack/solid-start-server | 2.0.0-rc.2 | floor | failure | dependency-contract-obligation |
| @tanstack/solid-start-server | 2.0.0-rc.2 | head | failure | dependency-contract-obligation |

Failure groups:
- 2x dependency-contract-obligation: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/start-server-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-start-server)
- 2x dependency-contract-obligation: no certifiable artifact case; 3 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/query-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-query)
- 2x dependency-contract-obligation: no certifiable artifact case; 3 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/query-persist-client-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-query-persist-client)

Failure details:
- **@tanstack/solid-query@6.0.0-rc.0** (floor, dependency-contract-obligation): solid-checker: no certifiable artifact case; 3 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/query-core solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/query-core" from /private<package-root>/build/index.js; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-... _(stderr truncated for readability)_
- **@tanstack/solid-query@6.0.0-rc.0** (head, dependency-contract-obligation): solid-checker: no certifiable artifact case; 3 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/query-core solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/query-core" from /private<package-root>/build/index.js; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-... _(stderr truncated for readability)_
- **@tanstack/solid-query-persist-client@6.0.0-rc.0** (floor, dependency-contract-obligation): solid-checker: no certifiable artifact case; 3 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/query-persist-client-core solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/query-persist-client-core" from /private<package-root>/build/index.js; acquire a verified dependency contract and pass its receipt-issued ex... _(stderr truncated for readability)_
- **@tanstack/solid-query-persist-client@6.0.0-rc.0** (head, dependency-contract-obligation): solid-checker: no certifiable artifact case; 3 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/query-persist-client-core solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/query-persist-client-core" from /private<package-root>/build/index.js; acquire a verified dependency contract and pass its receipt-issued ex... _(stderr truncated for readability)_
- **@tanstack/solid-start-server@2.0.0-rc.2** (floor, dependency-contract-obligation): solid-checker: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/start-server-core solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/start-server-core" from /private<package-root>/dist/esm/index.js; acquire a verified dependency contract and pas... _(stderr truncated for readability)_
- **@tanstack/solid-start-server@2.0.0-rc.2** (head, dependency-contract-obligation): solid-checker: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/start-server-core solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/start-server-core" from /private<package-root>/dist/esm/index.js; acquire a verified dependency contract and pas... _(stderr truncated for readability)_

### Solid Devtools

- Compatible packages: 0
- Probes run: 0
- Declared entrypoints: 0
- Generated entrypoints: 0
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 0
- Inapplicable artifact cases (recorded, not refused): 0
- Success (complete contracts): 0/0 (no probes run)
- Partial contracts: 0
- Failures: 0

### Solid Recharts

- Compatible packages: 1
- Probes run: 2
- Declared entrypoints: 2
- Generated entrypoints: 2
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 0
- Inapplicable artifact cases (recorded, not refused): 0
- Success (complete contracts): 2/2 (100%)
- Partial contracts: 0
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| solid-recharts | 2.0.0-beta.1 | floor | success | success |
| solid-recharts | 2.0.0-beta.1 | head | success | success |

### Motion for Solid

- Compatible packages: 1
- Probes run: 2
- Declared entrypoints: 6
- Generated entrypoints: 2
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 4
- Inapplicable artifact cases (recorded, not refused): 0
- Success (complete contracts): 0/2 (0%)
- Partial contracts: 2
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| motion-solidjs | 0.7.0-beta.4 | floor | partial-success | partial-success |
| motion-solidjs | 0.7.0-beta.4 | head | partial-success | partial-success |

**Solid 2.x totals:** 214/250 (85.6%) complete, 21 partial, 15 failed

### Beta-only packages

- @corvu-next/accordion@0.1.5 (corvu)
- @corvu-next/calendar@0.1.5 (corvu)
- @corvu-next/dialog@0.1.5 (corvu)
- @corvu-next/disclosure@0.1.5 (corvu)
- @corvu-next/dismissible@0.1.5 (corvu)
- @corvu-next/drawer@0.1.5 (corvu)
- @corvu-next/focus-trap@0.1.5 (corvu)
- @corvu-next/list@0.1.5 (corvu)
- @corvu-next/otp-field@0.1.5 (corvu)
- @corvu-next/persistent@0.1.5 (corvu)
- @corvu-next/popover@0.1.5 (corvu)
- @corvu-next/presence@0.1.5 (corvu)
- @corvu-next/prevent-scroll@0.1.5 (corvu)
- @corvu-next/resizable@0.1.5 (corvu)
- @corvu-next/tooltip@0.1.5 (corvu)
- @corvu-next/transition-size@0.1.5 (corvu)
- @corvu-next/utils@0.1.5 (corvu)

### RC-only packages

- @kobalte/core@2.0.0-alpha.0 (kobalte)
- @kobalte/utils@2.0.0-alpha.0 (kobalte)
- @solidjs/diagnostics@2.0.0-rc.3 (official-solid)
- @solidjs/element@2.0.0-rc.3 (official-solid)
- @solidjs/h@2.0.0-rc.3 (official-solid)
- @solidjs/html@2.0.0-rc.3 (official-solid)
- @solidjs/router@2.0.0-next.18 (official-solid)
- @solidjs/signals@2.0.0-rc.3 (official-solid)
- @solidjs/universal@2.0.0-rc.3 (official-solid)
- @solidjs/web@2.0.0-rc.3 (official-solid)
- solid-js@2.0.0-rc.3 (official-solid)

### Worse at head than at floor

None.

### Better at head than at floor

None.

## Contract content (what the emitted contracts claim)

- Contracts measured: 381 probe(s) across 190 package(s)
- Probes fully proven (no unknown claim, no refused entrypoint, no closure note): 0/381 (0%)
- Packages fully proven (every one of their probes): 0/190 (0%)
- Probes with at least one unknown claim: 381
- Probes with at least one refused entrypoint: 37
- Probes with at least one inapplicable artifact case: 311
- Probes with at least one closure note: 0
- Exports proven: 0/8682 (0%) (with unknown: 8682, without a summary: 0)
- Of those unknown exports: 31 unknown in every measured domain (the generator said nothing about them at all), 0 unknown only inside a conditional variant (the default resolution is fully claimed)
- Entrypoints: 1126 emitted, 0 refused; 371 artifact cases refused, 465 artifact cases inapplicable
- Closure notes (block byte-attested verification): 0
- Attested closure notes (record complete, runtime unbounded): 0

### Proposal wire size

| Artifact | Samples | p50 bytes | p95 bytes | max bytes |
| --- | ---: | ---: | ---: | ---: |
| Pretty main | 381 | 2186 | 12864 | 668201 |
| Canonical minified main | 381 | 1632 | 9602 | 495730 |
| Proposal plan (not evidence) | 381 | 49252 | 540062 | 19529867 |
| Canonical bytes per export | 381 | 356 | 1085 | 1696 |
| Canonical bytes per operation | 92 | 844.5 | 2628.5 | 15491.56 |

Proposal-plan bytes are construction obligations, not proof evidence and not acceptance authority. Proof-transcript and receipt bytes are measured separately by the Phase 16 accepted-corpus gate.

### Unknown claims by domain

| Domain | Exports carrying an unknown |
| --- | --- |
| callbacks | 8682 |
| reads | 8682 |
| writes | 8682 |
| creates | 8682 |
| invalidates | 8682 |
| throws | 8682 |
| returns | 8682 |
| cleanups | 8682 |
| disposals | 8682 |
| recursiveValue | 31 |
| **total** | **78169** |

Read the domain columns together, not separately: 31 of the 8682 unknown exports are unknown in every measured domain at once, so the same export can contribute to several columns.

### Positive behavioral rows (what a probe step would have to drive)

| Row kind | Count |
| --- | --- |
| invoke | 418 |
| return | 285 |
| read | 391 |
| write | 0 |
| invalidate | 0 |
| create | 85 |
| cleanup | 0 |
| dispose | 0 |

### Contract content by family

| Family | Contracts | Fully proven | With unknowns | With refusals | Exports proven | Unknown claims |
| --- | --- | --- | --- | --- | --- | --- |
| Official Solid | 19 | 0/19 (0%) | 19 | 10 | 0/1614 (0%) | 14545 |
| Kobalte | 5 | 0/5 (0%) | 5 | 5 | 0/3417 (0%) | 30753 |
| Solid Primitives | 282 | 0/282 (0%) | 282 | 8 | 0/1935 (0%) | 17423 |
| Corvu | 22 | 0/22 (0%) | 22 | 0 | 0/292 (0%) | 2632 |
| TanStack | 38 | 0/38 (0%) | 38 | 8 | 0/271 (0%) | 2439 |
| Solid Devtools | 10 | 0/10 (0%) | 10 | 4 | 0/278 (0%) | 2502 |
| Solid Recharts | 3 | 0/3 (0%) | 3 | 0 | 0/545 (0%) | 4905 |
| Motion for Solid | 2 | 0/2 (0%) | 2 | 2 | 0/330 (0%) | 2970 |

### Most unknown claims

| Package | Solid | Unknown claims | Exports with unknown / total | All five domains | Variant-only | Dominant cause |
| --- | --- | --- | --- | --- | --- | --- |
| @kobalte/core@0.13.13 | solid1 | 20295 | 2255/2255 | 0 | 0 | callbacks |
| @kobalte/core@2.0.0-alpha.0 | solid2 | 8352 | 928/928 | 0 | 0 | callbacks |
| solid-js@1.9.14 | solid1 | 5526 | 612/612 | 18 | 0 | callbacks |
| @solidjs/web@2.0.0-rc.3 | solid2 | 4347 | 483/483 | 0 | 0 | callbacks |
| solid-recharts@1.0.1 | solid1 | 2943 | 327/327 | 0 | 0 | callbacks |
| @solidjs/signals@2.0.0-rc.3 | solid2 | 1647 | 183/183 | 0 | 0 | callbacks |
| motion-solidjs@0.7.0-beta.4 | solid2 | 1485 | 165/165 | 0 | 0 | callbacks |
| motion-solidjs@0.7.0-beta.4 | solid2 | 1485 | 165/165 | 0 | 0 | callbacks |
| @kobalte/solidbase@0.6.13 | solid1 | 1296 | 144/144 | 0 | 0 | callbacks |
| @solid-devtools/shared@0.20.0 | solid1 | 1224 | 136/136 | 0 | 0 | callbacks |
| solid-recharts@2.0.0-beta.1 | solid2 | 981 | 109/109 | 0 | 0 | callbacks |
| solid-recharts@2.0.0-beta.1 | solid2 | 981 | 109/109 | 0 | 0 | callbacks |
| @solid-primitives/utils@7.0.0-next.4 | solid2 | 891 | 99/99 | 0 | 0 | callbacks |
| @solid-primitives/utils@7.0.0-next.4 | solid2 | 891 | 99/99 | 0 | 0 | callbacks |
| @solidjs/start@2.0.3 | solid1 | 747 | 83/83 | 0 | 0 | callbacks |

These figures describe the GENERATED DRAFT, not consumer findings. An unknown claim becomes a finding only when a consumer actually touches that surface, so a package with many unknowns on exports nobody imports costs a real project nothing. Nothing here has been reviewed or probed: every claim counted as proven is still inferred evidence awaiting review, and a closure note means the contract cannot be byte-attested at all.

## Combined

### Worker timings

- Worker time: 3347121 ms
- Phases: install 74322 ms, generation 457927 ms, harness 2814872 ms

### Top failure signatures

- 3x dependency-contract-obligation: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/start-server-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-start-server)
- 3x dependency-contract-obligation: no certifiable artifact case; 3 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/query-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-query)
- 3x dependency-contract-obligation: no certifiable artifact case; 3 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/query-persist-client-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-query-persist-client)
- 2x no-exported-surface: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file has no runtime ESM exports (packages: @solid-devtools/babel-plugin, @solid-devtools/ext-adapter)
- 2x unavailable-published-target: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: resolved <callee> <package-root>/dist/index.js is not a file (packages: @solid-primitives/animation)
- 2x dependency-contract-obligation: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: accepted dependency @solid-primitives/a11y has no exact runtime binding for export FormControlContext (packages: @solid-primitives/form)
- 2x dependency-contract-obligation: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: accepted dependency solid-js has no exact runtime binding for export NotReadyError (packages: @solid-primitives/intersection-observer)
- 2x dependency-contract-obligation: no certifiable artifact case; 2 case(s) refused; first refusal: .: accepted dependency @corvu/dialog has no exact runtime binding for export Portal (packages: @corvu/drawer, @corvu/popover)
- 1x unavailable-published-target: no certifiable artifact case; 1 case(s) refused; first refusal: .: resolved <callee> <package-root>/dist/index.cjs is not a file (packages: @solid-primitives/composites)
- 1x unavailable-published-target: no certifiable artifact case; 2 case(s) refused; first refusal: .: resolved <callee> <package-root>/dist/index.jsx is not a file (packages: @kobalte/themes)
- 1x missing-closure-module: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: local closure module ../node_modules/solid-js/types/reactive/signal.js from <package-root>/dist/index.d.ts was not found (packages: @solid-primitives/context)
- 1x missing-closure-module: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: local closure module ./types.js from <package-root>/dist/index.d.ts was not found (packages: @solid-primitives/workers)
- 1x dependency-contract-obligation: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: accepted dependency @tanstack/db has no exact runtime binding for export createTransaction (packages: @tanstack/solid-db)
- 1x dependency-contract-obligation: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/form-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-form)
- 1x dependency-contract-obligation: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/hotkeys solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-hotkeys)
- 1x dependency-contract-obligation: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/store solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-store)
- 1x dependency-contract-obligation: no certifiable artifact case; 1 case(s) refused and 1 case(s) recorded inapplicable; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/virtual-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-virtual)
- 1x dependency-contract-obligation: no certifiable artifact case; 1 case(s) refused; first refusal: .: accepted dependency @tanstack/ai-client has no exact runtime binding for export StorageUnavailableError (packages: @tanstack/ai-solid)
- 1x dependency-contract-obligation: no certifiable artifact case; 1 case(s) refused; first refusal: .: accepted dependency component-register has no exact runtime binding for export getCurrentElement (packages: @solidjs/element)
- 1x dependency-contract-obligation: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@testing-library/dom solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @solidjs/testing-library)
- 1x dependency-contract-obligation: no certifiable artifact case; 18 case(s) refused; first refusal: ./accordion: accepted dependency @corvu/accordion has no exact runtime binding for export default (packages: corvu)
- 1x dependency-contract-obligation: no certifiable artifact case; 2 case(s) refused; first refusal: .: accepted dependency @corvu-next/dialog has no exact runtime binding for export Portal (packages: @corvu-next/popover)
- 1x dependency-contract-obligation: no certifiable artifact case; 2 case(s) refused; first refusal: .: accepted dependency @corvu-next/disclosure has no exact runtime binding for export useContext (packages: @corvu-next/accordion)
- 1x dependency-contract-obligation: no certifiable artifact case; 2 case(s) refused; first refusal: .: accepted dependency @corvu/disclosure has no exact runtime binding for export useContext (packages: @corvu/accordion)
- 1x dependency-contract-obligation: no certifiable artifact case; 2 case(s) refused; first refusal: .: accepted dependency @tanstack/ai-solid has no exact runtime binding for export useChat (packages: @tanstack/ai-solid-ui)
- 1x dependency-contract-obligation: no certifiable artifact case; 2 case(s) refused; first refusal: .: accepted dependency motion-utils has no exact runtime binding for export MotionGlobalConfig (packages: motion-solidjs)

### Partial contracts

- @kobalte/core@0.13.13 (kobalte): 507 entrypoint(s) generated, 0 entrypoint(s) and 53 artifact case(s) refused
- @kobalte/core@2.0.0-alpha.0 (kobalte): 58 entrypoint(s) generated, 0 entrypoint(s) and 6 artifact case(s) refused
- @kobalte/solidbase@0.6.13 (kobalte): 33 entrypoint(s) generated, 0 entrypoint(s) and 59 artifact case(s) refused
- @kobalte/utils@0.9.2 (kobalte): 20 entrypoint(s) generated, 0 entrypoint(s) and 5 artifact case(s) refused
- @kobalte/utils@2.0.0-alpha.0 (kobalte): 7 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-devtools/debugger@0.28.1 (solid-devtools): 4 entrypoint(s) generated, 0 entrypoint(s) and 30 artifact case(s) refused
- @solid-devtools/locator@0.16.7 (solid-devtools): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-devtools/shared@0.20.0 (solid-devtools): 4 entrypoint(s) generated, 0 entrypoint(s) and 5 artifact case(s) refused
- @solid-primitives/controlled-props@1.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/controlled-props@1.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/sse@0.0.103 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 2 artifact case(s) refused
- @solid-primitives/sse@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 2 artifact case(s) refused
- @solid-primitives/sse@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 2 artifact case(s) refused
- @solid-primitives/utils@6.4.1 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/virtual@1.0.0-next.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/virtual@1.0.0-next.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solidjs/diagnostics@2.0.0-rc.3 (official-solid): 4 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solidjs/h@2.0.0-rc.3 (official-solid): 3 entrypoint(s) generated, 0 entrypoint(s) and 2 artifact case(s) refused
- @solidjs/image@0.1.0 (official-solid): 2 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solidjs/start@2.0.3 (official-solid): 10 entrypoint(s) generated, 0 entrypoint(s) and 3 artifact case(s) refused
- @solidjs/universal@2.0.0-rc.3 (official-solid): 1 entrypoint(s) generated, 0 entrypoint(s) and 2 artifact case(s) refused
- @solidjs/vite-plugin@3.0.0-next.34 (official-solid): 1 entrypoint(s) generated, 0 entrypoint(s) and 2 artifact case(s) refused
- @solidjs/vite-plugin@3.0.0-next.34 (official-solid): 1 entrypoint(s) generated, 0 entrypoint(s) and 2 artifact case(s) refused
- @solidjs/web@2.0.0-rc.3 (official-solid): 10 entrypoint(s) generated, 0 entrypoint(s) and 38 artifact case(s) refused
- @tanstack/solid-pacer@0.22.0 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 13 artifact case(s) refused
- @tanstack/solid-router@1.170.30 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 6 artifact case(s) refused
- @tanstack/solid-router@2.0.0-rc.2 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 4 artifact case(s) refused
- @tanstack/solid-router@2.0.0-rc.2 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 4 artifact case(s) refused
- @tanstack/solid-start@1.168.47 (tanstack): 3 entrypoint(s) generated, 0 entrypoint(s) and 9 artifact case(s) refused
- @tanstack/solid-start@2.0.0-rc.2 (tanstack): 3 entrypoint(s) generated, 0 entrypoint(s) and 9 artifact case(s) refused
- @tanstack/solid-start@2.0.0-rc.2 (tanstack): 3 entrypoint(s) generated, 0 entrypoint(s) and 9 artifact case(s) refused
- @tanstack/solid-table@9.1.2 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 6 artifact case(s) refused
- motion-solidjs@0.7.0-beta.4 (motion-solidjs): 1 entrypoint(s) generated, 0 entrypoint(s) and 2 artifact case(s) refused
- motion-solidjs@0.7.0-beta.4 (motion-solidjs): 1 entrypoint(s) generated, 0 entrypoint(s) and 2 artifact case(s) refused
- solid-devtools@0.34.5 (solid-devtools): 1 entrypoint(s) generated, 0 entrypoint(s) and 7 artifact case(s) refused
- solid-js@1.9.14 (official-solid): 18 entrypoint(s) generated, 0 entrypoint(s) and 57 artifact case(s) refused
- solid-js@2.0.0-rc.3 (official-solid): 1 entrypoint(s) generated, 0 entrypoint(s) and 21 artifact case(s) refused

### Shared dependency blockers

- @corvu/dialog: estimated 2 package(s) unlocked (@corvu/drawer, @corvu/popover)
- @corvu-next/dialog: estimated 1 package(s) unlocked (@corvu-next/popover)
- @corvu-next/disclosure: estimated 1 package(s) unlocked (@corvu-next/accordion)
- @corvu/accordion: estimated 1 package(s) unlocked (corvu)
- @corvu/disclosure: estimated 1 package(s) unlocked (@corvu/accordion)
- @solid-primitives/a11y: estimated 1 package(s) unlocked (@solid-primitives/form)
- @tanstack/ai-client: estimated 1 package(s) unlocked (@tanstack/ai-solid)
- @tanstack/ai-solid: estimated 1 package(s) unlocked (@tanstack/ai-solid-ui)
- @tanstack/db: estimated 1 package(s) unlocked (@tanstack/solid-db)
- @tanstack/form-core: estimated 1 package(s) unlocked (@tanstack/solid-form)
- @tanstack/hotkeys: estimated 1 package(s) unlocked (@tanstack/solid-hotkeys)
- @tanstack/query-core: estimated 1 package(s) unlocked (@tanstack/solid-query)
- @tanstack/query-persist-client-core: estimated 1 package(s) unlocked (@tanstack/solid-query-persist-client)
- @tanstack/start-server-core: estimated 1 package(s) unlocked (@tanstack/solid-start-server)
- @tanstack/store: estimated 1 package(s) unlocked (@tanstack/solid-store)
- @tanstack/virtual-core: estimated 1 package(s) unlocked (@tanstack/solid-virtual)
- @testing-library/dom: estimated 1 package(s) unlocked (@solidjs/testing-library)
- component-register: estimated 1 package(s) unlocked (@solidjs/element)
- motion-utils: estimated 1 package(s) unlocked (motion-solidjs)
- solid-js: estimated 1 package(s) unlocked (@solid-primitives/intersection-observer)

### Multi-blocker packages

None.

### Family comparison (Solid 1.x vs Solid 2.x)

| Family | Solid 1.x complete/total | Solid 2.x complete/total |
| --- | --- | --- |
| Official Solid | 2/6 (33.33%) | 7/15 (46.67%) |
| Kobalte | 0/4 (0%) | 0/2 (0%) |
| Solid Primitives | 92/97 (94.85%) | 182/194 (93.81%) |
| Corvu | 7/11 (63.64%) | 15/17 (88.24%) |
| TanStack | 22/36 (61.11%) | 8/18 (44.44%) |
| Solid Devtools | 6/12 (50%) | 0/0 (no probes run) |
| Solid Recharts | 1/1 (100%) | 2/2 (100%) |
| Motion for Solid | 0/1 (0%) | 0/2 (0%) |

### Discovery limitations

- packument for "@tanstack/tests-adapters" is unavailable (registry returned nothing for it)

### Unavailable metadata

- 381 contract-producing probe(s) missing checklistItems

### Baseline comparison

No baseline supplied.
