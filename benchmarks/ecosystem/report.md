# Ecosystem Benchmark Report

- Started: 2026-08-28T07:57:24.899Z
- Finished: 2026-08-28T08:05:13.515Z
- Duration: 468616 ms
- Checker native binary: /Users/thomas/Documents/Github/solid-checker/rust/target/release/solid-checker-rust
- Type Facts binary: /Users/thomas/Documents/Github/solid-checker/bin/solid-typefacts
- Manifest generated at: 2026-08-26T14:21:49.573Z (rows: 307, probes: 418)
- Scope: full corpus (418 probes run)

## Solid 1.x

### Official Solid

- Compatible packages: 6
- Probes run: 6
- Declared entrypoints: 44
- Generated entrypoints: 5
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 405
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
- Generated entrypoints: 1
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 2
- Success (complete contracts): 0/4 (0%)
- Partial contracts: 1
- Failures: 3

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @kobalte/core | 0.13.13 | only | partial-success | partial-success |
| @kobalte/solidbase | 0.6.13 | only | failure | unclassified |
| @kobalte/themes | 0.0.1-next.0 | only | failure | unclassified |
| @kobalte/utils | 0.9.2 | only | failure | unclassified |

Failure groups:
- 1x unclassified: no certifiable artifact case; 15 case(s) refused; first refusal: ./default-theme/*: wildcard export requires an explicit finite --entrypoint census (packages: @kobalte/solidbase)
- 1x unclassified: no certifiable artifact case; 2 case(s) refused; first refusal: .: resolved <callee> <package-root>/dist/index.jsx is not a file (packages: @kobalte/themes)
- 1x unclassified: no certifiable artifact case; 3 case(s) refused; first refusal: ./src/*: wildcard export requires an explicit finite --entrypoint census (packages: @kobalte/utils)

Failure details:
- **@kobalte/solidbase@0.6.13** (only, unclassified): solid-checker: no certifiable artifact case; 15 case(s) refused; first refusal: ./default-theme/*: wildcard export requires an explicit finite --entrypoint census
- **@kobalte/themes@0.0.1-next.0** (only, unclassified): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: resolved target <package-root>/dist/index.jsx is not a file
- **@kobalte/utils@0.9.2** (only, unclassified): solid-checker: no certifiable artifact case; 3 case(s) refused; first refusal: ./src/*: wildcard export requires an explicit finite --entrypoint census

### Solid Primitives

- Compatible packages: 97
- Probes run: 97
- Declared entrypoints: 94
- Generated entrypoints: 84
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 88
- Success (complete contracts): 6/97 (6.19%)
- Partial contracts: 78
- Failures: 13

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @solid-primitives/active-element | 2.1.6 | only | partial-success | partial-success |
| @solid-primitives/analytics | 0.2.1 | only | failure | export-kind-unresolved |
| @solid-primitives/audio | 1.4.5 | only | failure | export-kind-unresolved |
| @solid-primitives/autofocus | 0.1.5 | only | partial-success | partial-success |
| @solid-primitives/bounds | 0.1.7 | only | partial-success | partial-success |
| @solid-primitives/broadcast-channel | 0.1.1 | only | partial-success | partial-success |
| @solid-primitives/clipboard | 1.6.6 | only | partial-success | partial-success |
| @solid-primitives/composites | 1.1.1 | only | failure | unclassified |
| @solid-primitives/connectivity | 0.4.6 | only | partial-success | partial-success |
| @solid-primitives/context | 0.3.2 | only | failure | unclassified |
| @solid-primitives/controlled-props | 0.1.4 | only | partial-success | partial-success |
| @solid-primitives/cookies | 0.0.3 | only | partial-success | partial-success |
| @solid-primitives/cookies-store | 1.1.11 | only | failure | export-kind-unresolved |
| @solid-primitives/countdown | 1.0.9 | only | failure | no-exported-surface |
| @solid-primitives/cursor | 0.1.4 | only | partial-success | partial-success |
| @solid-primitives/date | 2.1.8 | only | partial-success | partial-success |
| @solid-primitives/date-difference | 1.0.2 | only | failure | no-exported-surface |
| @solid-primitives/db-store | 1.1.4 | only | partial-success | partial-success |
| @solid-primitives/debounce | 1.3.0 | only | partial-success | partial-success |
| @solid-primitives/deep | 0.3.7 | only | partial-success | partial-success |
| @solid-primitives/destructure | 0.2.4 | only | partial-success | partial-success |
| @solid-primitives/devices | 1.3.1 | only | partial-success | partial-success |
| @solid-primitives/event-bus | 1.1.4 | only | partial-success | partial-success |
| @solid-primitives/event-dispatcher | 0.1.1 | only | partial-success | partial-success |
| @solid-primitives/event-listener | 2.4.6 | only | partial-success | partial-success |
| @solid-primitives/event-props | 0.3.1 | only | partial-success | partial-success |
| @solid-primitives/fetch | 2.5.2 | only | partial-success | partial-success |
| @solid-primitives/filesystem | 1.3.4 | only | partial-success | partial-success |
| @solid-primitives/flux-store | 0.1.1 | only | partial-success | partial-success |
| @solid-primitives/fullscreen | 1.3.5 | only | partial-success | partial-success |
| @solid-primitives/geolocation | 1.5.5 | only | failure | unresolved-parameter-behavior |
| @solid-primitives/gestures | 1.2.1 | only | success | success |
| @solid-primitives/graphql | 3.0.0-next.0 | only | partial-success | partial-success |
| @solid-primitives/history | 0.2.5 | only | partial-success | partial-success |
| @solid-primitives/i18n | 2.2.1 | only | partial-success | partial-success |
| @solid-primitives/idle | 0.2.3 | only | partial-success | partial-success |
| @solid-primitives/immutable | 2.0.0-next.0 | only | partial-success | partial-success |
| @solid-primitives/input-mask | 0.3.1 | only | partial-success | partial-success |
| @solid-primitives/intersection-observer | 2.2.5 | only | failure | export-kind-unresolved |
| @solid-primitives/jsx-parser | 0.2.0 | only | success | success |
| @solid-primitives/jsx-tokenizer | 1.1.4 | only | partial-success | partial-success |
| @solid-primitives/keyboard | 1.3.7 | only | partial-success | partial-success |
| @solid-primitives/keyed | 1.5.3 | only | partial-success | partial-success |
| @solid-primitives/lifecycle | 0.1.2 | only | partial-success | partial-success |
| @solid-primitives/list | 0.1.2 | only | partial-success | partial-success |
| @solid-primitives/local-store | 1.1.4 | only | success | success |
| @solid-primitives/map | 0.7.4 | only | partial-success | partial-success |
| @solid-primitives/marker | 0.2.2 | only | partial-success | partial-success |
| @solid-primitives/masonry | 0.1.4 | only | partial-success | partial-success |
| @solid-primitives/match | 0.0.100 | only | partial-success | partial-success |
| @solid-primitives/media | 2.3.6 | only | partial-success | partial-success |
| @solid-primitives/memo | 1.5.1 | only | partial-success | partial-success |
| @solid-primitives/mouse | 2.1.7 | only | partial-success | partial-success |
| @solid-primitives/mutable | 1.1.1 | only | partial-success | partial-success |
| @solid-primitives/mutation-observer | 1.2.4 | only | partial-success | partial-success |
| @solid-primitives/page-visibility | 2.1.6 | only | partial-success | partial-success |
| @solid-primitives/pagination | 0.5.2 | only | partial-success | partial-success |
| @solid-primitives/permission | 1.3.2 | only | partial-success | partial-success |
| @solid-primitives/platform | 0.2.1 | only | failure | export-kind-unresolved |
| @solid-primitives/pointer | 0.3.6 | only | partial-success | partial-success |
| @solid-primitives/presence | 0.1.4 | only | partial-success | partial-success |
| @solid-primitives/promise | 1.1.4 | only | partial-success | partial-success |
| @solid-primitives/props | 3.2.4 | only | partial-success | partial-success |
| @solid-primitives/raf | 2.3.5 | only | partial-success | partial-success |
| @solid-primitives/range | 0.2.5 | only | partial-success | partial-success |
| @solid-primitives/reducer | 0.0.101 | only | failure | no-exported-surface |
| @solid-primitives/refs | 1.1.4 | only | partial-success | partial-success |
| @solid-primitives/resize-observer | 2.2.0 | only | partial-success | partial-success |
| @solid-primitives/resource | 0.4.3 | only | partial-success | partial-success |
| @solid-primitives/rootless | 1.5.4 | only | partial-success | partial-success |
| @solid-primitives/scheduled | 1.5.3 | only | partial-success | partial-success |
| @solid-primitives/script-loader | 2.3.2 | only | partial-success | partial-success |
| @solid-primitives/scroll | 2.1.6 | only | partial-success | partial-success |
| @solid-primitives/selection | 0.1.3 | only | partial-success | partial-success |
| @solid-primitives/set | 0.7.4 | only | partial-success | partial-success |
| @solid-primitives/share | 2.2.5 | only | partial-success | partial-success |
| @solid-primitives/signal-builders | 0.2.4 | only | partial-success | partial-success |
| @solid-primitives/spring | 0.1.2 | only | partial-success | partial-success |
| @solid-primitives/sse | 0.0.103 | only | partial-success | partial-success |
| @solid-primitives/start | 0.0.4 | only | success | success |
| @solid-primitives/state-machine | 0.1.1 | only | partial-success | partial-success |
| @solid-primitives/static-store | 0.1.4 | only | partial-success | partial-success |
| @solid-primitives/storage | 4.4.0 | only | partial-success | partial-success |
| @solid-primitives/stream | 0.7.4 | only | partial-success | partial-success |
| @solid-primitives/styles | 0.1.4 | only | partial-success | partial-success |
| @solid-primitives/throttle | 1.2.0 | only | success | success |
| @solid-primitives/timer | 1.4.4 | only | partial-success | partial-success |
| @solid-primitives/transition-group | 1.1.2 | only | partial-success | partial-success |
| @solid-primitives/trigger | 1.2.4 | only | partial-success | partial-success |
| @solid-primitives/tween | 1.4.1 | only | partial-success | partial-success |
| @solid-primitives/until | 0.1.1 | only | failure | no-exported-surface |
| @solid-primitives/upload | 0.1.5 | only | partial-success | partial-success |
| @solid-primitives/utils | 6.4.1 | only | partial-success | partial-success |
| @solid-primitives/virtual | 0.2.5 | only | partial-success | partial-success |
| @solid-primitives/visibility-observer | 2.0.1 | only | success | success |
| @solid-primitives/websocket | 1.4.0 | only | partial-success | partial-success |
| @solid-primitives/workers | 0.4.3 | only | failure | unclassified |

Failure groups:
- 4x no-exported-surface: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file has no runtime ESM exports (packages: @solid-primitives/countdown, @solid-primitives/date-difference, @solid-primitives/reducer, @solid-primitives/until)
- 3x export-kind-unresolved: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/audio, @solid-primitives/intersection-observer, @solid-primitives/platform)
- 2x export-kind-unresolved: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/analytics, @solid-primitives/cookies-store)
- 1x unresolved-parameter-behavior: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker-rust: package contract value export .:createGeolocation cannot have function effects (packages: @solid-primitives/geolocation)
- 1x unclassified: no certifiable artifact case; 1 case(s) refused; first refusal: .: resolved <callee> <package-root>/dist/index.cjs is not a file (packages: @solid-primitives/composites)
- 1x unclassified: no certifiable artifact case; 2 case(s) refused; first refusal: .: local closure module ../node_modules/solid-js/types/reactive/signal.js from <package-root>/dist/index.d.ts was not found (packages: @solid-primitives/context)
- 1x unclassified: no certifiable artifact case; 2 case(s) refused; first refusal: .: local closure module ./types.js from <package-root>/dist/index.d.ts was not found (packages: @solid-primitives/workers)

Failure details:
- **@solid-primitives/analytics@0.2.1** (only, export-kind-unresolved): solid-checker: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file /private<package-root>/dist/index.js exports "EventType", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "value" would certify it invokes no caller-supplied callback
- **@solid-primitives/audio@1.4.5** (only, export-kind-unresolved): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file /private<package-root>/dist/index.js exports "AudioState", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "value" would certify it invokes no caller-supplied callback
- **@solid-primitives/composites@1.1.1** (only, unclassified): solid-checker: no certifiable artifact case; 1 case(s) refused; first refusal: .: resolved target <package-root>/dist/index.cjs is not a file
- **@solid-primitives/context@0.3.2** (only, unclassified): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: local closure module ../node_modules/solid-js/types/reactive/signal.js from <package-root>/dist/index.d.ts was not found
- **@solid-primitives/cookies-store@1.1.11** (only, export-kind-unresolved): solid-checker: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file /private<package-root>/dist/index.js exports "CookieSitePolicy", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "value" would certify it invokes no caller-supplied callback
- **@solid-primitives/countdown@1.0.9** (only, no-exported-surface): solid-checker: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file /private<package-root>/dist/index.cjs has no runtime ESM exports
- **@solid-primitives/date-difference@1.0.2** (only, no-exported-surface): solid-checker: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file /private<package-root>/dist/index.cjs has no runtime ESM exports
- **@solid-primitives/geolocation@1.5.5** (only, unresolved-parameter-behavior): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker:unknown-claim-attribution={"analysisContext":"contract-generation-obligation","domains":["callbacks"],"endByte":1287,"exports":["createGeolocation"],"mechanism":"obligation-identity","obligation":"UnknownCallbackExecution","path":"<package-root>/dist/index.js","startByte":1280} solid-checker:unknown-cla... _(stderr truncated for readability)_
- **@solid-primitives/intersection-observer@2.2.5** (only, export-kind-unresolved): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file /private<package-root>/dist/index.js exports "DirectionX", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "value" would certify it invokes no caller-supplied callback
- **@solid-primitives/platform@0.2.1** (only, export-kind-unresolved): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file /private<package-root>/dist/index.js exports "isBrave", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "value" would certify it invokes no caller-supplied callback
- **@solid-primitives/reducer@0.0.101** (only, no-exported-surface): solid-checker: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file /private<package-root>/dist/index.cjs has no runtime ESM exports
- **@solid-primitives/until@0.1.1** (only, no-exported-surface): solid-checker: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file /private<package-root>/dist/index.cjs has no runtime ESM exports
- **@solid-primitives/workers@0.4.3** (only, unclassified): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: local closure module ./types.js from <package-root>/dist/index.d.ts was not found

### Corvu

- Compatible packages: 11
- Probes run: 11
- Declared entrypoints: 14
- Generated entrypoints: 10
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 3
- Success (complete contracts): 9/11 (81.82%)
- Partial contracts: 1
- Failures: 1

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @corvu/accordion | 0.2.5 | only | success | success |
| @corvu/calendar | 0.1.2 | only | success | success |
| @corvu/dialog | 0.2.4 | only | success | success |
| @corvu/disclosure | 0.2.2 | only | success | success |
| @corvu/drawer | 0.2.4 | only | success | success |
| @corvu/otp-field | 0.1.4 | only | success | success |
| @corvu/popover | 0.2.0 | only | success | success |
| @corvu/resizable | 0.2.5 | only | success | success |
| @corvu/tooltip | 0.2.2 | only | success | success |
| @corvu/utils | 0.4.2 | only | partial-success | partial-success |
| corvu | 0.7.2 | only | failure | unclassified |

Failure groups:
- 1x unclassified: package exports ./*; pass each finite --entrypoint explicitly so generation does not guess the public surface (packages: corvu)

Failure details:
- **corvu@0.7.2** (only, unclassified): solid-checker: package exports ./*; pass each finite --entrypoint explicitly so generation does not guess the public surface

### TanStack

- Compatible packages: 36
- Probes run: 36
- Declared entrypoints: 230
- Generated entrypoints: 17
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 93
- Success (complete contracts): 2/36 (5.56%)
- Partial contracts: 15
- Failures: 19

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @tanstack/ai-devtools-core | 0.5.8 | only | partial-success | partial-success |
| @tanstack/ai-solid | 0.19.1 | only | success | success |
| @tanstack/ai-solid-ui | 0.7.20 | only | failure | unclassified |
| @tanstack/charts | 0.15.0 | only | failure | package-contract-export-missing |
| @tanstack/devtools | 0.14.2 | only | partial-success | partial-success |
| @tanstack/devtools-a11y | 0.2.2 | only | failure | package-contract-export-missing |
| @tanstack/devtools-ui | 0.7.1 | only | partial-success | partial-success |
| @tanstack/devtools-utils | 0.7.0 | only | failure | unclassified |
| @tanstack/form-devtools | 1.0.0-alpha.2 | only | partial-success | partial-success |
| @tanstack/hotkeys-devtools | 0.9.0 | only | failure | export-kind-unresolved |
| @tanstack/pacer-devtools | 1.4.0 | only | partial-success | partial-success |
| @tanstack/solid-ai-devtools | 0.2.71 | only | partial-success | partial-success |
| @tanstack/solid-charts | 0.15.0 | only | success | success |
| @tanstack/solid-db | 0.2.40 | only | failure | dependency-contract-obligation |
| @tanstack/solid-devtools | 0.8.12 | only | partial-success | partial-success |
| @tanstack/solid-form | 2.0.0-alpha.2 | only | failure | dependency-contract-obligation |
| @tanstack/solid-form-devtools | 1.0.0-alpha.2 | only | partial-success | partial-success |
| @tanstack/solid-hotkeys | 0.10.0 | only | failure | dependency-contract-obligation |
| @tanstack/solid-hotkeys-devtools | 0.7.0 | only | failure | export-kind-unresolved |
| @tanstack/solid-pacer | 0.22.0 | only | failure | dependency-contract-obligation |
| @tanstack/solid-pacer-devtools | 0.14.0 | only | partial-success | partial-success |
| @tanstack/solid-query | 5.102.5 | only | failure | dependency-contract-obligation |
| @tanstack/solid-query-devtools | 5.102.5 | only | partial-success | partial-success |
| @tanstack/solid-query-persist-client | 5.102.5 | only | failure | dependency-contract-obligation |
| @tanstack/solid-router | 1.170.30 | only | partial-success | partial-success |
| @tanstack/solid-router-devtools | 1.167.1 | only | partial-success | partial-success |
| @tanstack/solid-router-ssr-query | 1.167.2-pre.0 | only | partial-success | partial-success |
| @tanstack/solid-start | 1.168.47 | only | failure | dependency-contract-obligation |
| @tanstack/solid-start-client | 1.168.29 | only | partial-success | partial-success |
| @tanstack/solid-start-config | 1.120.20 | only | partial-success | partial-success |
| @tanstack/solid-start-server | 1.167.36 | only | failure | dependency-contract-obligation |
| @tanstack/solid-store | 0.11.1 | only | failure | dependency-contract-obligation |
| @tanstack/solid-table | 9.1.2 | only | failure | dependency-contract-obligation |
| @tanstack/solid-table-devtools | 9.2.0 | only | failure | export-kind-unresolved |
| @tanstack/solid-virtual | 3.13.37 | only | failure | dependency-contract-obligation |
| @tanstack/table-devtools | 9.2.0 | only | failure | export-kind-unresolved |

Failure groups:
- 3x export-kind-unresolved: no certifiable artifact case; 3 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @tanstack/hotkeys-devtools, @tanstack/solid-table-devtools, @tanstack/table-devtools)
- 1x dependency-contract-obligation: no certifiable artifact case; 10 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/table-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-table)
- 1x dependency-contract-obligation: no certifiable artifact case; 13 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/start-client-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-start)
- 1x dependency-contract-obligation: no certifiable artifact case; 15 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/pacer solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-pacer)
- 1x dependency-contract-obligation: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/db solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-db)
- 1x dependency-contract-obligation: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/form-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-form)
- 1x dependency-contract-obligation: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/hotkeys solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-hotkeys)
- 1x dependency-contract-obligation: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/start-server-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-start-server)
- 1x dependency-contract-obligation: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/store solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-store)
- 1x dependency-contract-obligation: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/virtual-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-virtual)
- 1x dependency-contract-obligation: no certifiable artifact case; 4 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/query-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-query)
- 1x dependency-contract-obligation: no certifiable artifact case; 4 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/query-persist-client-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-query-persist-client)
- 1x package-contract-export-missing: no certifiable artifact case; 32 case(s) refused; first refusal: ./solid: solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker-rust: normalized operation graph is invalid: inference has no entrypoint "<value>" (packages: @tanstack/charts)
- 1x package-contract-export-missing: no certifiable artifact case; 4 case(s) refused; first refusal: ./core: solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker-rust: normalized operation graph is invalid: inference has no entrypoint "<value>" (packages: @tanstack/devtools-a11y)
- 1x export-kind-unresolved: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @tanstack/solid-hotkeys-devtools)
- 1x unclassified: no certifiable artifact case; 2 case(s) refused; first refusal: .: local closure module ./chat from <package-root>/src/index.ts was not found (packages: @tanstack/ai-solid-ui)
- 1x unclassified: no certifiable artifact case; 32 case(s) refused; first refusal: ./solid: solid-checker-rust: normalized operation graph is invalid: inference has no entrypoint "<value>" (packages: @tanstack/devtools-utils)

Failure details:
- **@tanstack/ai-solid-ui@0.7.20** (only, unclassified): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: local closure module ./chat from <package-root>/src/index.ts was not found
- **@tanstack/charts@0.15.0** (only, package-contract-export-missing): solid-checker: no certifiable artifact case; 32 case(s) refused; first refusal: ./solid: solid-checker:unknown-claim-attribution={"analysisContext":"no receipt-accepted contract matches this exact import","domains":["reactiveReads","returns","callbacks","ownerRequirements","asyncBehavior"],"endByte":407,"exports":["Chart"],"mechanism":"fallback-all","obligation":"PackageContractExportMissing","pat... _(stderr truncated for readability)_
- **@tanstack/devtools-a11y@0.2.2** (only, package-contract-export-missing): solid-checker: no certifiable artifact case; 4 case(s) refused; first refusal: ./core: solid-checker:unknown-claim-attribution={"analysisContext":"no receipt-accepted contract matches this exact import","domains":["reactiveReads","returns","callbacks","ownerRequirements","asyncBehavior"],"endByte":63,"exports":["A11yDevtoolsCore"],"mechanism":"fallback-all","obligation":"PackageContractExportMissi... _(stderr truncated for readability)_
- **@tanstack/devtools-utils@0.7.0** (only, unclassified): solid-checker: no certifiable artifact case; 32 case(s) refused; first refusal: ./solid: solid-checker-rust: normalized operation graph is invalid: inference has no entrypoint "./solid"
- **@tanstack/hotkeys-devtools@0.9.0** (only, export-kind-unresolved): solid-checker: no certifiable artifact case; 3 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file /private<package-root>/dist/index.js exports "HotkeysDevtoolsCore", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "value" would certify it invokes no caller-supplied callback
- **@tanstack/solid-db@0.2.40** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/db solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/db" from /private<package-root>/dist/esm/index.js; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts
- **@tanstack/solid-form@2.0.0-alpha.2** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/form-core solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/form-core" from /private<package-root>/dist/index.js; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-con... _(stderr truncated for readability)_
- **@tanstack/solid-hotkeys@0.10.0** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/hotkeys solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/hotkeys" from /private<package-root>/dist/index.js; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contrac... _(stderr truncated for readability)_
- **@tanstack/solid-hotkeys-devtools@0.7.0** (only, export-kind-unresolved): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file /private<package-root>/dist/index.js exports "HotkeysDevtoolsPanel", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "value" would certify it invokes no caller-supplied callback
- **@tanstack/solid-pacer@0.22.0** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 15 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/pacer solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/pacer" from /private<package-root>/dist/index.js; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts
- **@tanstack/solid-query@5.102.5** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 4 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/query-core solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/query-core" from /private<package-root>/build/index.js; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-... _(stderr truncated for readability)_
- **@tanstack/solid-query-persist-client@5.102.5** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 4 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/query-persist-client-core solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/query-persist-client-core" from /private<package-root>/build/index.js; acquire a verified dependency contract and pass its receipt-issued ex... _(stderr truncated for readability)_
- **@tanstack/solid-start@1.168.47** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 13 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/start-client-core solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/start-client-core" from /private<package-root>/dist/esm/index.js; acquire a verified dependency contract and pass its receipt-issued exact import t... _(stderr truncated for readability)_
- **@tanstack/solid-start-server@1.167.36** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/start-server-core solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/start-server-core" from /private<package-root>/dist/esm/index.js; acquire a verified dependency contract and pass its receipt-issued exact import th... _(stderr truncated for readability)_
- **@tanstack/solid-store@0.11.1** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/store solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/store" from /private<package-root>/dist/index.js; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts
- **@tanstack/solid-table@9.1.2** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 10 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/table-core solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/table-core" from /private<package-root>/dist/index.js; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-... _(stderr truncated for readability)_
- **@tanstack/solid-table-devtools@9.2.0** (only, export-kind-unresolved): solid-checker: no certifiable artifact case; 3 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file /private<package-root>/dist/index.js exports "TableDevtoolsPanel", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "value" would certify it invokes no caller-supplied callback
- **@tanstack/solid-virtual@3.13.37** (only, dependency-contract-obligation): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/virtual-core solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/virtual-core" from /private<package-root>/dist/esm/index.js; acquire a verified dependency contract and pass its receipt-issued exact import through --ac... _(stderr truncated for readability)_
- **@tanstack/table-devtools@9.2.0** (only, export-kind-unresolved): solid-checker: no certifiable artifact case; 3 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file /private<package-root>/dist/index.js exports "TableDevtoolsCore", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "value" would certify it invokes no caller-supplied callback

### Solid Devtools

- Compatible packages: 12
- Probes run: 12
- Declared entrypoints: 21
- Generated entrypoints: 7
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 20
- Success (complete contracts): 1/12 (8.33%)
- Partial contracts: 6
- Failures: 5

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @solid-devtools/babel-plugin | 0.3.1 | only | failure | no-exported-surface |
| @solid-devtools/debugger | 0.28.1 | only | partial-success | partial-success |
| @solid-devtools/ext-adapter | 0.17.0 | only | failure | no-exported-surface |
| @solid-devtools/extension-adapter | 0.12.1 | only | failure | no-exported-surface |
| @solid-devtools/frontend | 0.15.4 | only | partial-success | partial-success |
| @solid-devtools/locator | 0.16.7 | only | partial-success | partial-success |
| @solid-devtools/logger | 0.9.11 | only | partial-success | partial-success |
| @solid-devtools/overlay | 0.33.5 | only | partial-success | partial-success |
| @solid-devtools/shared | 0.20.0 | only | failure | unclassified |
| @solid-devtools/transform | 0.10.4 | only | success | success |
| @solid-devtools/ui | 0.10.3 | only | partial-success | partial-success |
| solid-devtools | 0.34.5 | only | failure | no-exported-surface |

Failure groups:
- 3x no-exported-surface: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file has no runtime ESM exports (packages: @solid-devtools/babel-plugin, @solid-devtools/ext-adapter, @solid-devtools/extension-adapter)
- 1x no-exported-surface: no certifiable artifact case; 40 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file has no runtime ESM exports (packages: solid-devtools)
- 1x unclassified: no certifiable artifact case; 3 case(s) refused; first refusal: ./*: wildcard export requires an explicit finite --entrypoint census (packages: @solid-devtools/shared)

Failure details:
- **@solid-devtools/babel-plugin@0.3.1** (only, no-exported-surface): solid-checker: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file /private<package-root>/dist/index.js has no runtime ESM exports
- **@solid-devtools/ext-adapter@0.17.0** (only, no-exported-surface): solid-checker: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file /private<package-root>/dist/index.cjs has no runtime ESM exports
- **@solid-devtools/extension-adapter@0.12.1** (only, no-exported-surface): solid-checker: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file /private<package-root>/dist/index.cjs has no runtime ESM exports
- **@solid-devtools/shared@0.20.0** (only, unclassified): solid-checker: no certifiable artifact case; 3 case(s) refused; first refusal: ./*: wildcard export requires an explicit finite --entrypoint census
- **solid-devtools@0.34.5** (only, no-exported-surface): solid-checker: no certifiable artifact case; 40 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file /private<package-root>/dist/index_noop.js has no runtime ESM exports

### Solid Recharts

- Compatible packages: 1
- Probes run: 1
- Declared entrypoints: 1
- Generated entrypoints: 1
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 1
- Success (complete contracts): 0/1 (0%)
- Partial contracts: 1
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| solid-recharts | 1.0.1 | only | partial-success | partial-success |

### Motion for Solid

- Compatible packages: 1
- Probes run: 1
- Declared entrypoints: 2
- Generated entrypoints: 0
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 0
- Success (complete contracts): 0/1 (0%)
- Partial contracts: 0
- Failures: 1

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| motion-solidjs | 0.6.0 | only | failure | export-kind-unresolved |

Failure groups:
- 1x export-kind-unresolved: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: motion-solidjs)

Failure details:
- **motion-solidjs@0.6.0** (only, export-kind-unresolved): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file /private<package-root>/dist/v1/index.mjs exports "AnimatePresence", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "value" would certify it invokes no caller-supplied callback

**Solid 1.x totals:** 20/168 (11.9%) complete, 105 partial, 43 failed

## Solid 2.x

### Official Solid

- Compatible packages: 12
- Probes run: 15
- Declared entrypoints: 46
- Generated entrypoints: 15
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 576
- Success (complete contracts): 4/15 (26.67%)
- Partial contracts: 11
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @solidjs/diagnostics | 2.0.0-rc.3 | only | partial-success | partial-success |
| @solidjs/element | 2.0.0-rc.3 | only | success | success |
| @solidjs/h | 2.0.0-rc.3 | only | partial-success | partial-success |
| @solidjs/html | 2.0.0-rc.3 | only | success | success |
| @solidjs/meta | 1.0.0-next.2 | floor | success | success |
| @solidjs/meta | 1.0.0-next.2 | head | success | success |
| @solidjs/router | 2.0.0-next.18 | only | partial-success | partial-success |
| @solidjs/signals | 2.0.0-rc.3 | only | partial-success | partial-success |
| @solidjs/start-devtools | 1.0.0-next.4 | floor | partial-success | partial-success |
| @solidjs/start-devtools | 1.0.0-next.4 | head | partial-success | partial-success |
| @solidjs/universal | 2.0.0-rc.3 | only | partial-success | partial-success |
| @solidjs/vite-plugin | 3.0.0-next.34 | floor | partial-success | partial-success |
| @solidjs/vite-plugin | 3.0.0-next.34 | head | partial-success | partial-success |
| @solidjs/web | 2.0.0-rc.3 | only | partial-success | partial-success |
| solid-js | 2.0.0-rc.3 | only | partial-success | partial-success |

### Kobalte

- Compatible packages: 2
- Probes run: 2
- Declared entrypoints: 3
- Generated entrypoints: 1
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 1
- Success (complete contracts): 0/2 (0%)
- Partial contracts: 1
- Failures: 1

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @kobalte/core | 2.0.0-alpha.0 | only | failure | unclassified |
| @kobalte/utils | 2.0.0-alpha.0 | only | partial-success | partial-success |

Failure groups:
- 1x unclassified: package exports ./*; pass each finite --entrypoint explicitly so generation does not guess the public surface (packages: @kobalte/core)

Failure details:
- **@kobalte/core@2.0.0-alpha.0** (only, unclassified): solid-checker: package exports ./*; pass each finite --entrypoint explicitly so generation does not guess the public surface

### Solid Primitives

- Compatible packages: 97
- Probes run: 194
- Declared entrypoints: 212
- Generated entrypoints: 190
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 234
- Success (complete contracts): 0/194 (0%)
- Partial contracts: 190
- Failures: 4

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @solid-primitives/a11y | 1.0.0-next.3 | floor | partial-success | partial-success |
| @solid-primitives/a11y | 1.0.0-next.3 | head | partial-success | partial-success |
| @solid-primitives/active-element | 3.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/active-element | 3.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/analytics | 2.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/analytics | 2.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/animation | 1.0.0-next.1 | floor | failure | unclassified |
| @solid-primitives/animation | 1.0.0-next.1 | head | failure | unclassified |
| @solid-primitives/async | 0.0.101-next.3 | floor | partial-success | partial-success |
| @solid-primitives/async | 0.0.101-next.3 | head | partial-success | partial-success |
| @solid-primitives/audio | 3.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/audio | 3.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/bounds | 1.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/bounds | 1.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/broadcast-channel | 1.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/broadcast-channel | 1.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/clipboard | 2.0.0-next.17 | floor | partial-success | partial-success |
| @solid-primitives/clipboard | 2.0.0-next.17 | head | partial-success | partial-success |
| @solid-primitives/connectivity | 1.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/connectivity | 1.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/context | 2.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/context | 2.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/controlled-props | 1.0.0-next.3 | floor | partial-success | partial-success |
| @solid-primitives/controlled-props | 1.0.0-next.3 | head | partial-success | partial-success |
| @solid-primitives/controlled-signal | 1.0.0-next.3 | floor | partial-success | partial-success |
| @solid-primitives/controlled-signal | 1.0.0-next.3 | head | partial-success | partial-success |
| @solid-primitives/cookies | 1.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/cookies | 1.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/cursor | 1.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/cursor | 1.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/date | 3.0.0-next.3 | floor | partial-success | partial-success |
| @solid-primitives/date | 3.0.0-next.3 | head | partial-success | partial-success |
| @solid-primitives/deep | 1.0.0-next.3 | floor | partial-success | partial-success |
| @solid-primitives/deep | 1.0.0-next.3 | head | partial-success | partial-success |
| @solid-primitives/destructure | 1.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/destructure | 1.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/devices | 3.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/devices | 3.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/drag-drop | 0.1.0-next.0 | floor | partial-success | partial-success |
| @solid-primitives/drag-drop | 0.1.0-next.0 | head | partial-success | partial-success |
| @solid-primitives/event-bus | 3.0.0-next.3 | floor | partial-success | partial-success |
| @solid-primitives/event-bus | 3.0.0-next.3 | head | partial-success | partial-success |
| @solid-primitives/event-dispatcher | 1.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/event-dispatcher | 1.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/event-listener | 3.0.0-next.3 | floor | partial-success | partial-success |
| @solid-primitives/event-listener | 3.0.0-next.3 | head | partial-success | partial-success |
| @solid-primitives/event-props | 1.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/event-props | 1.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/favicon | 1.0.0-next.1 | floor | partial-success | partial-success |
| @solid-primitives/favicon | 1.0.0-next.1 | head | partial-success | partial-success |
| @solid-primitives/filesystem | 3.0.0-next.3 | floor | partial-success | partial-success |
| @solid-primitives/filesystem | 3.0.0-next.3 | head | partial-success | partial-success |
| @solid-primitives/flux-store | 1.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/flux-store | 1.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/focus | 1.0.0-next.4 | floor | partial-success | partial-success |
| @solid-primitives/focus | 1.0.0-next.4 | head | partial-success | partial-success |
| @solid-primitives/form | 1.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/form | 1.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/fullscreen | 2.0.0-next.3 | floor | partial-success | partial-success |
| @solid-primitives/fullscreen | 2.0.0-next.3 | head | partial-success | partial-success |
| @solid-primitives/geolocation | 3.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/geolocation | 3.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/gestures | 3.0.0-next.3 | floor | partial-success | partial-success |
| @solid-primitives/gestures | 3.0.0-next.3 | head | partial-success | partial-success |
| @solid-primitives/history | 1.0.0-next.3 | floor | partial-success | partial-success |
| @solid-primitives/history | 1.0.0-next.3 | head | partial-success | partial-success |
| @solid-primitives/i18n | 3.0.0-next.4 | floor | partial-success | partial-success |
| @solid-primitives/i18n | 3.0.0-next.4 | head | partial-success | partial-success |
| @solid-primitives/idle | 1.0.0-next.3 | floor | partial-success | partial-success |
| @solid-primitives/idle | 1.0.0-next.3 | head | partial-success | partial-success |
| @solid-primitives/input-mask | 1.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/input-mask | 1.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/interaction | 1.0.0-next.4 | floor | partial-success | partial-success |
| @solid-primitives/interaction | 1.0.0-next.4 | head | partial-success | partial-success |
| @solid-primitives/intersection-observer | 3.0.0-next.3 | floor | partial-success | partial-success |
| @solid-primitives/intersection-observer | 3.0.0-next.3 | head | partial-success | partial-success |
| @solid-primitives/jsx-tokenizer | 3.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/jsx-tokenizer | 3.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/keyboard | 2.0.0-next.5 | floor | partial-success | partial-success |
| @solid-primitives/keyboard | 2.0.0-next.5 | head | partial-success | partial-success |
| @solid-primitives/keyed | 3.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/keyed | 3.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/lifecycle | 1.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/lifecycle | 1.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/list | 1.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/list | 1.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/list-state | 1.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/list-state | 1.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/map | 1.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/map | 1.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/marker | 2.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/marker | 2.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/masonry | 2.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/masonry | 2.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/match | 1.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/match | 1.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/media | 4.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/media | 4.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/mediastream | 1.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/mediastream | 1.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/memo | 2.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/memo | 2.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/mouse | 4.0.0-next.3 | floor | partial-success | partial-success |
| @solid-primitives/mouse | 4.0.0-next.3 | head | partial-success | partial-success |
| @solid-primitives/mutable | 3.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/mutable | 3.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/mutation-observer | 3.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/mutation-observer | 3.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/notification | 1.0.0-next.3 | floor | partial-success | partial-success |
| @solid-primitives/notification | 1.0.0-next.3 | head | partial-success | partial-success |
| @solid-primitives/orientation | 1.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/orientation | 1.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/page-utilities | 3.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/page-utilities | 3.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/pagination | 1.0.0-next.6 | floor | partial-success | partial-success |
| @solid-primitives/pagination | 1.0.0-next.6 | head | partial-success | partial-success |
| @solid-primitives/permission | 2.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/permission | 2.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/platform | 1.0.0-next.2 | floor | failure | export-kind-unresolved |
| @solid-primitives/platform | 1.0.0-next.2 | head | failure | export-kind-unresolved |
| @solid-primitives/pointer | 1.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/pointer | 1.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/presence | 1.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/presence | 1.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/promise | 2.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/promise | 2.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/props | 4.0.0-next.3 | floor | partial-success | partial-success |
| @solid-primitives/props | 4.0.0-next.3 | head | partial-success | partial-success |
| @solid-primitives/queue | 1.0.0-next.3 | floor | partial-success | partial-success |
| @solid-primitives/queue | 1.0.0-next.3 | head | partial-success | partial-success |
| @solid-primitives/raf | 4.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/raf | 4.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/range | 1.0.0-next.3 | floor | partial-success | partial-success |
| @solid-primitives/range | 1.0.0-next.3 | head | partial-success | partial-success |
| @solid-primitives/refs | 3.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/refs | 3.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/resize-observer | 4.0.0-next.3 | floor | partial-success | partial-success |
| @solid-primitives/resize-observer | 4.0.0-next.3 | head | partial-success | partial-success |
| @solid-primitives/rootless | 2.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/rootless | 2.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/scheduled | 2.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/scheduled | 2.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/script-loader | 3.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/script-loader | 3.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/scroll | 3.0.0-next.4 | floor | partial-success | partial-success |
| @solid-primitives/scroll | 3.0.0-next.4 | head | partial-success | partial-success |
| @solid-primitives/selection | 1.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/selection | 1.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/sensors | 1.0.0-next.3 | floor | partial-success | partial-success |
| @solid-primitives/sensors | 1.0.0-next.3 | head | partial-success | partial-success |
| @solid-primitives/set | 1.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/set | 1.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/share | 4.0.0-next.4 | floor | partial-success | partial-success |
| @solid-primitives/share | 4.0.0-next.4 | head | partial-success | partial-success |
| @solid-primitives/signal-builders | 1.0.0-next.4 | floor | partial-success | partial-success |
| @solid-primitives/signal-builders | 1.0.0-next.4 | head | partial-success | partial-success |
| @solid-primitives/sortable | 1.0.0-next.0 | floor | partial-success | partial-success |
| @solid-primitives/sortable | 1.0.0-next.0 | head | partial-success | partial-success |
| @solid-primitives/spring | 1.0.0-next.3 | floor | partial-success | partial-success |
| @solid-primitives/spring | 1.0.0-next.3 | head | partial-success | partial-success |
| @solid-primitives/sse | 1.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/sse | 1.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/state-machine | 1.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/state-machine | 1.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/static-store | 1.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/static-store | 1.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/storage | 5.0.0-next.4 | floor | partial-success | partial-success |
| @solid-primitives/storage | 5.0.0-next.4 | head | partial-success | partial-success |
| @solid-primitives/styles | 1.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/styles | 1.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/timer | 1.4.5-next.1 | floor | partial-success | partial-success |
| @solid-primitives/timer | 1.4.5-next.1 | head | partial-success | partial-success |
| @solid-primitives/transition-group | 2.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/transition-group | 2.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/trigger | 3.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/trigger | 3.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/tween | 2.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/tween | 2.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/upload | 1.0.0-next.4 | floor | partial-success | partial-success |
| @solid-primitives/upload | 1.0.0-next.4 | head | partial-success | partial-success |
| @solid-primitives/url | 0.2.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/url | 0.2.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/utils | 7.0.0-next.4 | floor | partial-success | partial-success |
| @solid-primitives/utils | 7.0.0-next.4 | head | partial-success | partial-success |
| @solid-primitives/vibrate | 1.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/vibrate | 1.0.0-next.2 | head | partial-success | partial-success |
| @solid-primitives/video | 1.0.0-next.3 | floor | partial-success | partial-success |
| @solid-primitives/video | 1.0.0-next.3 | head | partial-success | partial-success |
| @solid-primitives/virtual | 1.0.0-next.4 | floor | partial-success | partial-success |
| @solid-primitives/virtual | 1.0.0-next.4 | head | partial-success | partial-success |
| @solid-primitives/websocket | 2.0.0-next.3 | floor | partial-success | partial-success |
| @solid-primitives/websocket | 2.0.0-next.3 | head | partial-success | partial-success |
| @solid-primitives/workers | 2.0.1-next.1 | floor | partial-success | partial-success |
| @solid-primitives/workers | 2.0.1-next.1 | head | partial-success | partial-success |

Failure groups:
- 2x export-kind-unresolved: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/platform)
- 2x unclassified: no certifiable artifact case; 2 case(s) refused; first refusal: .: resolved <callee> <package-root>/dist/index.js is not a file (packages: @solid-primitives/animation)

Failure details:
- **@solid-primitives/animation@1.0.0-next.1** (floor, unclassified): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: resolved target <package-root>/dist/index.js is not a file
- **@solid-primitives/animation@1.0.0-next.1** (head, unclassified): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: resolved target <package-root>/dist/index.js is not a file
- **@solid-primitives/platform@1.0.0-next.2** (floor, export-kind-unresolved): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file /private<package-root>/dist/index.js exports "isBrave", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "value" would certify it invokes no caller-supplied callback
- **@solid-primitives/platform@1.0.0-next.2** (head, export-kind-unresolved): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file /private<package-root>/dist/index.js exports "isBrave", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "value" would certify it invokes no caller-supplied callback

### Corvu

- Compatible packages: 17
- Probes run: 17
- Declared entrypoints: 20
- Generated entrypoints: 17
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 3
- Success (complete contracts): 16/17 (94.12%)
- Partial contracts: 1
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @corvu-next/accordion | 0.1.5 | only | success | success |
| @corvu-next/calendar | 0.1.5 | only | success | success |
| @corvu-next/dialog | 0.1.5 | only | success | success |
| @corvu-next/disclosure | 0.1.5 | only | success | success |
| @corvu-next/dismissible | 0.1.5 | only | success | success |
| @corvu-next/drawer | 0.1.5 | only | success | success |
| @corvu-next/focus-trap | 0.1.5 | only | success | success |
| @corvu-next/list | 0.1.5 | only | success | success |
| @corvu-next/otp-field | 0.1.5 | only | success | success |
| @corvu-next/persistent | 0.1.5 | only | success | success |
| @corvu-next/popover | 0.1.5 | only | success | success |
| @corvu-next/presence | 0.1.5 | only | success | success |
| @corvu-next/prevent-scroll | 0.1.5 | only | success | success |
| @corvu-next/resizable | 0.1.5 | only | success | success |
| @corvu-next/tooltip | 0.1.5 | only | success | success |
| @corvu-next/transition-size | 0.1.5 | only | success | success |
| @corvu-next/utils | 0.1.5 | only | partial-success | partial-success |

### TanStack

- Compatible packages: 9
- Probes run: 18
- Declared entrypoints: 60
- Generated entrypoints: 10
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 32
- Success (complete contracts): 0/18 (0%)
- Partial contracts: 10
- Failures: 8

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @tanstack/solid-query | 6.0.0-rc.0 | floor | failure | dependency-contract-obligation |
| @tanstack/solid-query | 6.0.0-rc.0 | head | failure | dependency-contract-obligation |
| @tanstack/solid-query-devtools | 6.0.0-rc.0 | floor | partial-success | partial-success |
| @tanstack/solid-query-devtools | 6.0.0-rc.0 | head | partial-success | partial-success |
| @tanstack/solid-query-persist-client | 6.0.0-rc.0 | floor | failure | dependency-contract-obligation |
| @tanstack/solid-query-persist-client | 6.0.0-rc.0 | head | failure | dependency-contract-obligation |
| @tanstack/solid-router | 2.0.0-rc.2 | floor | partial-success | partial-success |
| @tanstack/solid-router | 2.0.0-rc.2 | head | partial-success | partial-success |
| @tanstack/solid-router-devtools | 2.0.0-rc.2 | floor | partial-success | partial-success |
| @tanstack/solid-router-devtools | 2.0.0-rc.2 | head | partial-success | partial-success |
| @tanstack/solid-router-ssr-query | 2.0.0-rc.2 | floor | partial-success | partial-success |
| @tanstack/solid-router-ssr-query | 2.0.0-rc.2 | head | partial-success | partial-success |
| @tanstack/solid-start | 2.0.0-rc.2 | floor | failure | dependency-contract-obligation |
| @tanstack/solid-start | 2.0.0-rc.2 | head | failure | dependency-contract-obligation |
| @tanstack/solid-start-client | 2.0.0-rc.2 | floor | partial-success | partial-success |
| @tanstack/solid-start-client | 2.0.0-rc.2 | head | partial-success | partial-success |
| @tanstack/solid-start-server | 2.0.0-rc.2 | floor | failure | dependency-contract-obligation |
| @tanstack/solid-start-server | 2.0.0-rc.2 | head | failure | dependency-contract-obligation |

Failure groups:
- 2x dependency-contract-obligation: no certifiable artifact case; 13 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/start-client-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-start)
- 2x dependency-contract-obligation: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/start-server-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-start-server)
- 2x dependency-contract-obligation: no certifiable artifact case; 4 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/query-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-query)
- 2x dependency-contract-obligation: no certifiable artifact case; 4 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/query-persist-client-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-query-persist-client)

Failure details:
- **@tanstack/solid-query@6.0.0-rc.0** (floor, dependency-contract-obligation): solid-checker: no certifiable artifact case; 4 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/query-core solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/query-core" from /private<package-root>/build/index.js; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-... _(stderr truncated for readability)_
- **@tanstack/solid-query@6.0.0-rc.0** (head, dependency-contract-obligation): solid-checker: no certifiable artifact case; 4 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/query-core solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/query-core" from /private<package-root>/build/index.js; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-... _(stderr truncated for readability)_
- **@tanstack/solid-query-persist-client@6.0.0-rc.0** (floor, dependency-contract-obligation): solid-checker: no certifiable artifact case; 4 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/query-persist-client-core solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/query-persist-client-core" from /private<package-root>/build/index.js; acquire a verified dependency contract and pass its receipt-issued ex... _(stderr truncated for readability)_
- **@tanstack/solid-query-persist-client@6.0.0-rc.0** (head, dependency-contract-obligation): solid-checker: no certifiable artifact case; 4 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/query-persist-client-core solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/query-persist-client-core" from /private<package-root>/build/index.js; acquire a verified dependency contract and pass its receipt-issued ex... _(stderr truncated for readability)_
- **@tanstack/solid-start@2.0.0-rc.2** (floor, dependency-contract-obligation): solid-checker: no certifiable artifact case; 13 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/start-client-core solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/start-client-core" from /private<package-root>/dist/esm/index.js; acquire a verified dependency contract and pass its receipt-issued exact import t... _(stderr truncated for readability)_
- **@tanstack/solid-start@2.0.0-rc.2** (head, dependency-contract-obligation): solid-checker: no certifiable artifact case; 13 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/start-client-core solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/start-client-core" from /private<package-root>/dist/esm/index.js; acquire a verified dependency contract and pass its receipt-issued exact import t... _(stderr truncated for readability)_
- **@tanstack/solid-start-server@2.0.0-rc.2** (floor, dependency-contract-obligation): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/start-server-core solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/start-server-core" from /private<package-root>/dist/esm/index.js; acquire a verified dependency contract and pass its receipt-issued exact import th... _(stderr truncated for readability)_
- **@tanstack/solid-start-server@2.0.0-rc.2** (head, dependency-contract-obligation): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/start-server-core solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/start-server-core" from /private<package-root>/dist/esm/index.js; acquire a verified dependency contract and pass its receipt-issued exact import th... _(stderr truncated for readability)_

### Solid Devtools

- Compatible packages: 0
- Probes run: 0
- Declared entrypoints: 0
- Generated entrypoints: 0
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 0
- Success (complete contracts): 0/0 (no probes run)
- Partial contracts: 0
- Failures: 0

### Solid Recharts

- Compatible packages: 1
- Probes run: 2
- Declared entrypoints: 2
- Generated entrypoints: 0
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 0
- Success (complete contracts): 0/2 (0%)
- Partial contracts: 0
- Failures: 2

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| solid-recharts | 2.0.0-beta.1 | floor | failure | unclassified |
| solid-recharts | 2.0.0-beta.1 | head | failure | unclassified |

Failure groups:
- 2x unclassified: no certifiable artifact case; 1 case(s) refused; first refusal: .: local closure module ./animation/easing from <package-root>/src/index.ts was not found (packages: solid-recharts)

Failure details:
- **solid-recharts@2.0.0-beta.1** (floor, unclassified): solid-checker: no certifiable artifact case; 1 case(s) refused; first refusal: .: local closure module ./animation/easing from <package-root>/src/index.ts was not found
- **solid-recharts@2.0.0-beta.1** (head, unclassified): solid-checker: no certifiable artifact case; 1 case(s) refused; first refusal: .: local closure module ./animation/easing from <package-root>/src/index.ts was not found

### Motion for Solid

- Compatible packages: 1
- Probes run: 2
- Declared entrypoints: 6
- Generated entrypoints: 0
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 0
- Success (complete contracts): 0/2 (0%)
- Partial contracts: 0
- Failures: 2

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| motion-solidjs | 0.7.0-beta.4 | floor | failure | export-kind-unresolved |
| motion-solidjs | 0.7.0-beta.4 | head | failure | export-kind-unresolved |

Failure groups:
- 2x export-kind-unresolved: no certifiable artifact case; 3 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: motion-solidjs)

Failure details:
- **motion-solidjs@0.7.0-beta.4** (floor, export-kind-unresolved): solid-checker: no certifiable artifact case; 3 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file /private<package-root>/dist/v2/index.mjs exports "AnimatePresence", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "value" would certify it invokes no caller-supplied callback
- **motion-solidjs@0.7.0-beta.4** (head, export-kind-unresolved): solid-checker: no certifiable artifact case; 3 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file /private<package-root>/dist/v2/index.mjs exports "AnimatePresence", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "value" would certify it invokes no caller-supplied callback

**Solid 2.x totals:** 20/250 (8%) complete, 213 partial, 17 failed

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

- Contracts measured: 358 probe(s) across 177 package(s)
- Probes fully proven (no unknown claim, no refused entrypoint, no closure note): 0/358 (0%)
- Packages fully proven (every one of their probes): 0/177 (0%)
- Probes with at least one unknown claim: 358
- Probes with at least one refused entrypoint: 318
- Probes with at least one closure note: 0
- Exports proven: 0/4788 (0%) (with unknown: 4788, without a summary: 0)
- Of those unknown exports: 13 unknown in every measured domain (the generator said nothing about them at all), 0 unknown only inside a conditional variant (the default resolution is fully claimed)
- Entrypoints: 358 emitted, 0 refused; 1458 artifact cases refused
- Closure notes (block byte-attested verification): 0
- Attested closure notes (record complete, runtime unbounded): 0

### Proposal wire size

| Artifact | Samples | p50 bytes | p95 bytes | max bytes |
| --- | ---: | ---: | ---: | ---: |
| Pretty main | 358 | 2080 | 7364 | 50378 |
| Canonical minified main | 358 | 1599 | 4813 | 43055 |
| Proposal plan (not evidence) | 358 | 49172 | 303349 | 3676782 |
| Canonical bytes per export | 358 | 332.25 | 1074 | 1936 |
| Canonical bytes per operation | 63 | 712.4 | 2138 | 3349 |

Proposal-plan bytes are construction obligations, not proof evidence and not acceptance authority. Proof-transcript and receipt bytes are measured separately by the Phase 16 accepted-corpus gate.

### Unknown claims by domain

| Domain | Exports carrying an unknown |
| --- | --- |
| callbacks | 4788 |
| reads | 4788 |
| writes | 4788 |
| creates | 4788 |
| invalidates | 4788 |
| throws | 4788 |
| returns | 4788 |
| cleanups | 4788 |
| disposals | 4788 |
| recursiveValue | 13 |
| **total** | **43105** |

Read the domain columns together, not separately: 13 of the 4788 unknown exports are unknown in every measured domain at once, so the same export can contribute to several columns.

### Positive behavioral rows (what a probe step would have to drive)

| Row kind | Count |
| --- | --- |
| invoke | 169 |
| return | 120 |
| read | 101 |
| write | 0 |
| invalidate | 0 |
| create | 38 |
| cleanup | 0 |
| dispose | 0 |

### Contract content by family

| Family | Contracts | Fully proven | With unknowns | With refusals | Exports proven | Unknown claims |
| --- | --- | --- | --- | --- | --- | --- |
| Official Solid | 20 | 0/20 (0%) | 20 | 14 | 0/1274 (0%) | 11472 |
| Kobalte | 2 | 0/2 (0%) | 2 | 2 | 0/214 (0%) | 1926 |
| Solid Primitives | 274 | 0/274 (0%) | 274 | 268 | 0/1788 (0%) | 16099 |
| Corvu | 27 | 0/27 (0%) | 27 | 2 | 0/290 (0%) | 2610 |
| TanStack | 27 | 0/27 (0%) | 27 | 25 | 0/816 (0%) | 7344 |
| Solid Devtools | 7 | 0/7 (0%) | 7 | 6 | 0/79 (0%) | 711 |
| Solid Recharts | 1 | 0/1 (0%) | 1 | 1 | 0/327 (0%) | 2943 |
| Motion for Solid | 0 | 0/0 (nothing measured) | 0 | 0 | 0/0 (nothing measured) | 0 |

### Most unknown claims

| Package | Solid | Unknown claims | Exports with unknown / total | All five domains | Variant-only | Dominant cause |
| --- | --- | --- | --- | --- | --- | --- |
| @solidjs/web@2.0.0-rc.3 | solid2 | 4068 | 452/452 | 0 | 0 | callbacks |
| solid-recharts@1.0.1 | solid1 | 2943 | 327/327 | 0 | 0 | callbacks |
| solid-js@2.0.0-rc.3 | solid2 | 2736 | 304/304 | 0 | 0 | callbacks |
| @tanstack/solid-router@1.170.30 | solid1 | 2619 | 291/291 | 0 | 0 | callbacks |
| @kobalte/core@0.13.13 | solid1 | 1746 | 194/194 | 0 | 0 | callbacks |
| @tanstack/solid-router@2.0.0-rc.2 | solid2 | 1746 | 194/194 | 0 | 0 | callbacks |
| @tanstack/solid-router@2.0.0-rc.2 | solid2 | 1746 | 194/194 | 0 | 0 | callbacks |
| solid-js@1.9.14 | solid1 | 1455 | 161/161 | 6 | 0 | callbacks |
| @solidjs/signals@2.0.0-rc.3 | solid2 | 1098 | 122/122 | 0 | 0 | callbacks |
| @solidjs/router@1.0.0 | solid1 | 684 | 76/76 | 0 | 0 | callbacks |
| @solidjs/router@2.0.0-next.18 | solid2 | 504 | 56/56 | 0 | 0 | callbacks |
| @solid-primitives/utils@7.0.0-next.4 | solid2 | 405 | 45/45 | 0 | 0 | callbacks |
| @solid-primitives/utils@7.0.0-next.4 | solid2 | 405 | 45/45 | 0 | 0 | callbacks |
| @solid-primitives/utils@6.4.1 | solid1 | 360 | 40/40 | 0 | 0 | callbacks |
| @solid-primitives/signal-builders@1.0.0-next.4 | solid2 | 351 | 39/39 | 0 | 0 | callbacks |

These figures describe the GENERATED DRAFT, not consumer findings. An unknown claim becomes a finding only when a consumer actually touches that surface, so a package with many unknowns on exports nobody imports costs a real project nothing. Nothing here has been reviewed or probed: every claim counted as proven is still inferred evidence awaiting review, and a closure note means the contract cannot be byte-attested at all.

## Combined

### Worker timings

- Worker time: 2985908 ms
- Phases: install 57391 ms, generation 2927437 ms, harness 1080 ms

### Top failure signatures

- 7x no-exported-surface: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file has no runtime ESM exports (packages: @solid-devtools/babel-plugin, @solid-devtools/ext-adapter, @solid-devtools/extension-adapter, @solid-primitives/countdown, @solid-primitives/date-difference, @solid-primitives/reducer, @solid-primitives/until)
- 7x export-kind-unresolved: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/audio, @solid-primitives/intersection-observer, @solid-primitives/platform, @tanstack/solid-hotkeys-devtools, motion-solidjs)
- 5x export-kind-unresolved: no certifiable artifact case; 3 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @tanstack/hotkeys-devtools, @tanstack/solid-table-devtools, @tanstack/table-devtools, motion-solidjs)
- 3x dependency-contract-obligation: no certifiable artifact case; 13 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/start-client-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-start)
- 3x dependency-contract-obligation: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/start-server-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-start-server)
- 3x dependency-contract-obligation: no certifiable artifact case; 4 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/query-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-query)
- 3x dependency-contract-obligation: no certifiable artifact case; 4 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/query-persist-client-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-query-persist-client)
- 2x export-kind-unresolved: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/analytics, @solid-primitives/cookies-store)
- 2x unclassified: no certifiable artifact case; 1 case(s) refused; first refusal: .: local closure module ./animation/easing from <package-root>/src/index.ts was not found (packages: solid-recharts)
- 2x unclassified: no certifiable artifact case; 2 case(s) refused; first refusal: .: resolved <callee> <package-root>/dist/index.js is not a file (packages: @solid-primitives/animation)
- 2x unclassified: package exports ./*; pass each finite --entrypoint explicitly so generation does not guess the public surface (packages: @kobalte/core, corvu)
- 1x no-exported-surface: no certifiable artifact case; 40 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file has no runtime ESM exports (packages: solid-devtools)
- 1x unresolved-parameter-behavior: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker-rust: package contract value export .:createGeolocation cannot have function effects (packages: @solid-primitives/geolocation)
- 1x dependency-contract-obligation: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@testing-library/dom solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @solidjs/testing-library)
- 1x dependency-contract-obligation: no certifiable artifact case; 10 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/table-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-table)
- 1x dependency-contract-obligation: no certifiable artifact case; 15 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/pacer solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-pacer)
- 1x dependency-contract-obligation: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/db solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-db)
- 1x dependency-contract-obligation: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/form-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-form)
- 1x dependency-contract-obligation: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/hotkeys solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-hotkeys)
- 1x dependency-contract-obligation: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/store solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-store)
- 1x dependency-contract-obligation: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/virtual-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-virtual)
- 1x package-contract-export-missing: no certifiable artifact case; 32 case(s) refused; first refusal: ./solid: solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker-rust: normalized operation graph is invalid: inference has no entrypoint "<value>" (packages: @tanstack/charts)
- 1x package-contract-export-missing: no certifiable artifact case; 4 case(s) refused; first refusal: ./core: solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker:unknown-claim-attribution={ "<value>", "<value>", "<value>", "<value>", "<value>", "<value>"], "<value>", "<value>", "<value>", "<value>", "<value>", "<value>" } solid-checker-rust: normalized operation graph is invalid: inference has no entrypoint "<value>" (packages: @tanstack/devtools-a11y)
- 1x unclassified: no certifiable artifact case; 1 case(s) refused; first refusal: .: resolved <callee> <package-root>/dist/index.cjs is not a file (packages: @solid-primitives/composites)
- 1x unclassified: no certifiable artifact case; 15 case(s) refused; first refusal: ./default-theme/*: wildcard export requires an explicit finite --entrypoint census (packages: @kobalte/solidbase)
- 1x unclassified: no certifiable artifact case; 2 case(s) refused; first refusal: .: local closure module ../node_modules/solid-js/types/reactive/signal.js from <package-root>/dist/index.d.ts was not found (packages: @solid-primitives/context)
- 1x unclassified: no certifiable artifact case; 2 case(s) refused; first refusal: .: local closure module ./chat from <package-root>/src/index.ts was not found (packages: @tanstack/ai-solid-ui)
- 1x unclassified: no certifiable artifact case; 2 case(s) refused; first refusal: .: local closure module ./types.js from <package-root>/dist/index.d.ts was not found (packages: @solid-primitives/workers)
- 1x unclassified: no certifiable artifact case; 2 case(s) refused; first refusal: .: resolved <callee> <package-root>/dist/index.jsx is not a file (packages: @kobalte/themes)
- 1x unclassified: no certifiable artifact case; 3 case(s) refused; first refusal: ./*: wildcard export requires an explicit finite --entrypoint census (packages: @solid-devtools/shared)
- 1x unclassified: no certifiable artifact case; 3 case(s) refused; first refusal: ./src/*: wildcard export requires an explicit finite --entrypoint census (packages: @kobalte/utils)
- 1x unclassified: no certifiable artifact case; 32 case(s) refused; first refusal: ./solid: solid-checker-rust: normalized operation graph is invalid: inference has no entrypoint "<value>" (packages: @tanstack/devtools-utils)

### Partial contracts

- @corvu-next/utils@0.1.5 (corvu): 1 entrypoint(s) generated, 0 entrypoint(s) and 3 artifact case(s) refused
- @corvu/utils@0.4.2 (corvu): 1 entrypoint(s) generated, 0 entrypoint(s) and 3 artifact case(s) refused
- @kobalte/core@0.13.13 (kobalte): 1 entrypoint(s) generated, 0 entrypoint(s) and 2 artifact case(s) refused
- @kobalte/utils@2.0.0-alpha.0 (kobalte): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-devtools/debugger@0.28.1 (solid-devtools): 1 entrypoint(s) generated, 0 entrypoint(s) and 4 artifact case(s) refused
- @solid-devtools/frontend@0.15.4 (solid-devtools): 1 entrypoint(s) generated, 0 entrypoint(s) and 3 artifact case(s) refused
- @solid-devtools/locator@0.16.7 (solid-devtools): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-devtools/logger@0.9.11 (solid-devtools): 1 entrypoint(s) generated, 0 entrypoint(s) and 4 artifact case(s) refused
- @solid-devtools/overlay@0.33.5 (solid-devtools): 1 entrypoint(s) generated, 0 entrypoint(s) and 4 artifact case(s) refused
- @solid-devtools/ui@0.10.3 (solid-devtools): 1 entrypoint(s) generated, 0 entrypoint(s) and 4 artifact case(s) refused
- @solid-primitives/a11y@1.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/a11y@1.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/active-element@2.1.6 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/active-element@3.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/active-element@3.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/analytics@2.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 7 artifact case(s) refused
- @solid-primitives/analytics@2.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 7 artifact case(s) refused
- @solid-primitives/async@0.0.101-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/async@0.0.101-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/audio@3.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/audio@3.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/autofocus@0.1.5 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/bounds@0.1.7 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/bounds@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/bounds@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/broadcast-channel@0.1.1 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/broadcast-channel@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/broadcast-channel@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/clipboard@1.6.6 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/clipboard@2.0.0-next.17 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/clipboard@2.0.0-next.17 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/connectivity@0.4.6 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/connectivity@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/connectivity@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/context@2.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/context@2.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/controlled-props@0.1.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 2 artifact case(s) refused
- @solid-primitives/controlled-props@1.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 3 artifact case(s) refused
- @solid-primitives/controlled-props@1.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 3 artifact case(s) refused
- @solid-primitives/controlled-signal@1.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/controlled-signal@1.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/cookies@0.0.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/cookies@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/cookies@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/cursor@0.1.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/cursor@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/cursor@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/date@2.1.8 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/date@3.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/date@3.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/db-store@1.1.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/debounce@1.3.0 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/deep@0.3.7 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/deep@1.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/deep@1.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/destructure@0.2.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/destructure@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/destructure@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/devices@1.3.1 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/devices@3.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/devices@3.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/drag-drop@0.1.0-next.0 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/drag-drop@0.1.0-next.0 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/event-bus@1.1.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/event-bus@3.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/event-bus@3.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/event-dispatcher@0.1.1 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/event-dispatcher@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/event-dispatcher@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/event-listener@2.4.6 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/event-listener@3.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/event-listener@3.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/event-props@0.3.1 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/event-props@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/event-props@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/favicon@1.0.0-next.1 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/favicon@1.0.0-next.1 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/fetch@2.5.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/filesystem@1.3.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/filesystem@3.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/filesystem@3.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/flux-store@0.1.1 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/flux-store@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/flux-store@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/focus@1.0.0-next.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/focus@1.0.0-next.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/form@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/form@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/fullscreen@1.3.5 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/fullscreen@2.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/fullscreen@2.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/geolocation@3.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/geolocation@3.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/gestures@3.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/gestures@3.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/graphql@3.0.0-next.0 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/history@0.2.5 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/history@1.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/history@1.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/i18n@2.2.1 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/i18n@3.0.0-next.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/i18n@3.0.0-next.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/idle@0.2.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/idle@1.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/idle@1.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/immutable@2.0.0-next.0 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/input-mask@0.3.1 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/input-mask@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/input-mask@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/interaction@1.0.0-next.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/interaction@1.0.0-next.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/intersection-observer@3.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/intersection-observer@3.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/jsx-tokenizer@1.1.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/jsx-tokenizer@3.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/jsx-tokenizer@3.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/keyboard@1.3.7 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/keyboard@2.0.0-next.5 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/keyboard@2.0.0-next.5 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/keyed@1.5.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/keyed@3.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/keyed@3.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/lifecycle@0.1.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/lifecycle@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/lifecycle@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/list@0.1.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/list@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/list@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/list-state@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/list-state@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/map@0.7.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/map@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/map@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/marker@0.2.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/marker@2.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/marker@2.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/masonry@0.1.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/masonry@2.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/masonry@2.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/match@0.0.100 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/match@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/match@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/media@2.3.6 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/media@4.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/media@4.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/mediastream@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/mediastream@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/memo@1.5.1 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/memo@2.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/memo@2.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/mouse@2.1.7 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/mouse@4.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/mouse@4.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/mutable@1.1.1 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/mutable@3.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/mutable@3.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/mutation-observer@1.2.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/mutation-observer@3.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/mutation-observer@3.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/notification@1.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/notification@1.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/orientation@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/orientation@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/page-utilities@3.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/page-utilities@3.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/page-visibility@2.1.6 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/pagination@0.5.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/pagination@1.0.0-next.6 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/pagination@1.0.0-next.6 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/permission@1.3.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/permission@2.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/permission@2.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/pointer@0.3.6 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/pointer@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/pointer@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/presence@0.1.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/presence@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/presence@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/promise@1.1.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/promise@2.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/promise@2.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/props@3.2.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/props@4.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/props@4.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/queue@1.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/queue@1.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/raf@2.3.5 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/raf@4.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/raf@4.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/range@0.2.5 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/range@1.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/range@1.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/refs@1.1.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/refs@3.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/refs@3.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/resize-observer@2.2.0 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/resize-observer@4.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/resize-observer@4.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/resource@0.4.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/rootless@1.5.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/rootless@2.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/rootless@2.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/scheduled@1.5.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/scheduled@2.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/scheduled@2.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/script-loader@2.3.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/script-loader@3.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/script-loader@3.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/scroll@2.1.6 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/scroll@3.0.0-next.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/scroll@3.0.0-next.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/selection@0.1.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/selection@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/selection@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/sensors@1.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/sensors@1.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/set@0.7.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/set@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/set@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/share@2.2.5 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/share@4.0.0-next.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/share@4.0.0-next.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/signal-builders@0.2.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/signal-builders@1.0.0-next.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/signal-builders@1.0.0-next.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/sortable@1.0.0-next.0 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/sortable@1.0.0-next.0 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/spring@0.1.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/spring@1.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/spring@1.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/sse@0.0.103 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 5 artifact case(s) refused
- @solid-primitives/sse@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 5 artifact case(s) refused
- @solid-primitives/sse@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 5 artifact case(s) refused
- @solid-primitives/state-machine@0.1.1 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/state-machine@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/state-machine@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/static-store@0.1.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/static-store@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/static-store@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/storage@4.4.0 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 3 artifact case(s) refused
- @solid-primitives/storage@5.0.0-next.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 3 artifact case(s) refused
- @solid-primitives/storage@5.0.0-next.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 3 artifact case(s) refused
- @solid-primitives/stream@0.7.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/styles@0.1.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/styles@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/styles@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/timer@1.4.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/timer@1.4.5-next.1 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/timer@1.4.5-next.1 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/transition-group@1.1.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/transition-group@2.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/transition-group@2.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/trigger@1.2.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/trigger@3.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/trigger@3.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/tween@1.4.1 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/tween@2.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/tween@2.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/upload@0.1.5 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/upload@1.0.0-next.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/upload@1.0.0-next.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/url@0.2.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/url@0.2.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/utils@6.4.1 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 3 artifact case(s) refused
- @solid-primitives/utils@7.0.0-next.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 5 artifact case(s) refused
- @solid-primitives/utils@7.0.0-next.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 5 artifact case(s) refused
- @solid-primitives/vibrate@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/vibrate@1.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/video@1.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/video@1.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/virtual@0.2.5 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 2 artifact case(s) refused
- @solid-primitives/virtual@1.0.0-next.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 3 artifact case(s) refused
- @solid-primitives/virtual@1.0.0-next.4 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 3 artifact case(s) refused
- @solid-primitives/websocket@1.4.0 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/websocket@2.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/websocket@2.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/workers@2.0.1-next.1 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 3 artifact case(s) refused
- @solid-primitives/workers@2.0.1-next.1 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 3 artifact case(s) refused
- @solidjs/diagnostics@2.0.0-rc.3 (official-solid): 1 entrypoint(s) generated, 0 entrypoint(s) and 5 artifact case(s) refused
- @solidjs/h@2.0.0-rc.3 (official-solid): 1 entrypoint(s) generated, 0 entrypoint(s) and 3 artifact case(s) refused
- @solidjs/image@0.1.0 (official-solid): 1 entrypoint(s) generated, 0 entrypoint(s) and 4 artifact case(s) refused
- @solidjs/router@2.0.0-next.18 (official-solid): 1 entrypoint(s) generated, 0 entrypoint(s) and 4 artifact case(s) refused
- @solidjs/signals@2.0.0-rc.3 (official-solid): 1 entrypoint(s) generated, 0 entrypoint(s) and 5 artifact case(s) refused
- @solidjs/start@2.0.3 (official-solid): 1 entrypoint(s) generated, 0 entrypoint(s) and 12 artifact case(s) refused
- @solidjs/start-devtools@1.0.0-next.4 (official-solid): 1 entrypoint(s) generated, 0 entrypoint(s) and 22 artifact case(s) refused
- @solidjs/start-devtools@1.0.0-next.4 (official-solid): 1 entrypoint(s) generated, 0 entrypoint(s) and 22 artifact case(s) refused
- @solidjs/universal@2.0.0-rc.3 (official-solid): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solidjs/vite-plugin@3.0.0-next.34 (official-solid): 1 entrypoint(s) generated, 0 entrypoint(s) and 4 artifact case(s) refused
- @solidjs/vite-plugin@3.0.0-next.34 (official-solid): 1 entrypoint(s) generated, 0 entrypoint(s) and 4 artifact case(s) refused
- @solidjs/web@2.0.0-rc.3 (official-solid): 1 entrypoint(s) generated, 0 entrypoint(s) and 413 artifact case(s) refused
- @tanstack/ai-devtools-core@0.5.8 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 36 artifact case(s) refused
- @tanstack/devtools@0.14.2 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 4 artifact case(s) refused
- @tanstack/devtools-ui@0.7.1 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 3 artifact case(s) refused
- @tanstack/form-devtools@1.0.0-alpha.2 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 3 artifact case(s) refused
- @tanstack/pacer-devtools@1.4.0 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 18 artifact case(s) refused
- @tanstack/solid-ai-devtools@0.2.71 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @tanstack/solid-devtools@0.8.12 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @tanstack/solid-form-devtools@1.0.0-alpha.2 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @tanstack/solid-pacer-devtools@0.14.0 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 2 artifact case(s) refused
- @tanstack/solid-query-devtools@5.102.5 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 4 artifact case(s) refused
- @tanstack/solid-query-devtools@6.0.0-rc.0 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 4 artifact case(s) refused
- @tanstack/solid-query-devtools@6.0.0-rc.0 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 4 artifact case(s) refused
- @tanstack/solid-router@1.170.30 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 13 artifact case(s) refused
- @tanstack/solid-router@2.0.0-rc.2 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 6 artifact case(s) refused
- @tanstack/solid-router@2.0.0-rc.2 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 6 artifact case(s) refused
- @tanstack/solid-router-devtools@1.167.1 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 2 artifact case(s) refused
- @tanstack/solid-router-devtools@2.0.0-rc.2 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 2 artifact case(s) refused
- @tanstack/solid-router-devtools@2.0.0-rc.2 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 2 artifact case(s) refused
- @tanstack/solid-router-ssr-query@1.167.2-pre.0 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @tanstack/solid-router-ssr-query@2.0.0-rc.2 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @tanstack/solid-router-ssr-query@2.0.0-rc.2 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @tanstack/solid-start-client@1.168.29 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 3 artifact case(s) refused
- @tanstack/solid-start-client@2.0.0-rc.2 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 3 artifact case(s) refused
- @tanstack/solid-start-client@2.0.0-rc.2 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 3 artifact case(s) refused
- @tanstack/solid-start-config@1.120.20 (tanstack): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- solid-js@1.9.14 (official-solid): 1 entrypoint(s) generated, 0 entrypoint(s) and 389 artifact case(s) refused
- solid-js@2.0.0-rc.3 (official-solid): 1 entrypoint(s) generated, 0 entrypoint(s) and 93 artifact case(s) refused
- solid-recharts@1.0.1 (solid-recharts): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused

### Shared dependency blockers

- @tanstack/db: estimated 1 package(s) unlocked (@tanstack/solid-db)
- @tanstack/form-core: estimated 1 package(s) unlocked (@tanstack/solid-form)
- @tanstack/hotkeys: estimated 1 package(s) unlocked (@tanstack/solid-hotkeys)
- @tanstack/pacer: estimated 1 package(s) unlocked (@tanstack/solid-pacer)
- @tanstack/query-core: estimated 1 package(s) unlocked (@tanstack/solid-query)
- @tanstack/query-persist-client-core: estimated 1 package(s) unlocked (@tanstack/solid-query-persist-client)
- @tanstack/start-client-core: estimated 1 package(s) unlocked (@tanstack/solid-start)
- @tanstack/start-server-core: estimated 1 package(s) unlocked (@tanstack/solid-start-server)
- @tanstack/store: estimated 1 package(s) unlocked (@tanstack/solid-store)
- @tanstack/table-core: estimated 1 package(s) unlocked (@tanstack/solid-table)
- @tanstack/virtual-core: estimated 1 package(s) unlocked (@tanstack/solid-virtual)
- @testing-library/dom: estimated 1 package(s) unlocked (@solidjs/testing-library)

### Multi-blocker packages

None.

### Family comparison (Solid 1.x vs Solid 2.x)

| Family | Solid 1.x complete/total | Solid 2.x complete/total |
| --- | --- | --- |
| Official Solid | 2/6 (33.33%) | 4/15 (26.67%) |
| Kobalte | 0/4 (0%) | 0/2 (0%) |
| Solid Primitives | 6/97 (6.19%) | 0/194 (0%) |
| Corvu | 9/11 (81.82%) | 16/17 (94.12%) |
| TanStack | 2/36 (5.56%) | 0/18 (0%) |
| Solid Devtools | 1/12 (8.33%) | 0/0 (no probes run) |
| Solid Recharts | 0/1 (0%) | 0/2 (0%) |
| Motion for Solid | 0/1 (0%) | 0/2 (0%) |

### Discovery limitations

- packument for "@tanstack/tests-adapters" is unavailable (registry returned nothing for it)

### Unavailable metadata

- 358 contract-producing probe(s) missing checklistItems

### Baseline comparison

No baseline supplied.
