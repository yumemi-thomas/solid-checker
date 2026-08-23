# Ecosystem Benchmark Report

- Started: 2026-08-23T15:44:48.474Z
- Finished: 2026-08-23T15:46:23.686Z
- Duration: 95212 ms
- Checker native binary: /private/tmp/claude-501/-Users-thomas-Documents-Github-solid-checker/0284cce1-b5a3-4bdc-98fc-13749b2e3f46/scratchpad/bins-kind/solid-checker-rust
- Type Facts binary: /private/tmp/claude-501/-Users-thomas-Documents-Github-solid-checker/0284cce1-b5a3-4bdc-98fc-13749b2e3f46/scratchpad/bins-kind/solid-typefacts
- Manifest generated at: 2026-08-22T07:44:17.857Z (rows: 305, probes: 416)
- Scope: full corpus (416 probes run)

## Solid 1.x

### Official Solid

- Compatible packages: 6
- Probes run: 6
- Declared entrypoints: 44
- Generated entrypoints: 27
- Refused entrypoints (partial contracts): 1
- Success (complete contracts): 5/6 (83.33%)
- Partial contracts: 1
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @solidjs/image | 0.1.0 | only | success | success |
| @solidjs/meta | 0.29.4 | only | success | success |
| @solidjs/router | 1.0.0 | only | success | success |
| @solidjs/start | 2.0.3 | only | success | success |
| @solidjs/testing-library | 0.8.10 | only | success | success |
| solid-js | 1.9.14 | only | partial-success | partial-success |

### Kobalte

- Compatible packages: 4
- Probes run: 4
- Declared entrypoints: 6
- Generated entrypoints: 69
- Refused entrypoints (partial contracts): 0
- Success (complete contracts): 1/4 (25%)
- Partial contracts: 0
- Failures: 3

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @kobalte/core | 0.13.13 | only | success | success |
| @kobalte/solidbase | 0.6.13 | only | failure | install-failure |
| @kobalte/themes | 0.0.1-next.0 | only | failure | no-esm-runtime-target |
| @kobalte/utils | 0.9.2 | only | failure | unclassified |

Failure groups:
- 1x no-esm-runtime-target: @kobalte/themes has no supported ESM runtime entrypoints (packages: @kobalte/themes)
- 1x install-failure: npm error code ERESOLVE npm error ERESOLVE could not resolve npm error npm error While resolving: @solidjs/start@ npm error Found: @solidjs/router@ npm error node_modules/@solidjs/router npm error @solidjs/router@"<value>" from @kobalte/solidbase@ npm error node_modules/@kobalte/solidbase npm error @kobalte/solidbase@"<value>" from the root project npm error npm error Could not resolve dependency: npm error peerOptional @solidjs/router@"<value>" from @solidjs/start@ npm error node_modules/@solidjs/start npm error peer @solidjs/start@"<value>" from @kobalte/solidbase@ npm error node_modules/@kobalte/solidbase npm error @kobalte/solidbase@"<value>" from the root project npm error npm error Conflicting peer dependency: @solidjs/router@ npm error node_modules/@solidjs/router npm error peerOptional @solidjs/router@"<value>" from @solidjs/start@ npm error node_modules/@solidjs/start npm error peer @solidjs/start@"<value>" from @kobalte/solidbase@ npm error node_modules/@kobalte/solidbase npm error @kobalte/solidbase@"<value>" from the root project npm error npm error Fix the upstream dependency conflict, or retry npm error this command with --force or --legacy-peer-deps npm error to accept an incorrect (and potentially broken) dependency resolution. npm error npm error npm error For a full report see: npm error npm error A complete log of this run can be found in: (packages: @kobalte/solidbase)
- 1x unclassified: @kobalte/utils has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @kobalte/utils)

Failure details:
- **@kobalte/solidbase@0.6.13** (only, install-failure): npm error code ERESOLVE npm error ERESOLVE could not resolve npm error npm error While resolving: @solidjs/start@2.0.3 npm error Found: @solidjs/router@0.15.4 npm error node_modules/@solidjs/router npm error @solidjs/router@"^0.15.3" from @kobalte/solidbase@0.6.13 npm error node_modules/@kobalte/solidbase npm error @kobalte/solidbase@"0.6.13" from the root project npm error npm error Could not res... _(stderr truncated for readability)_
- **@kobalte/themes@0.0.1-next.0** (only, no-esm-runtime-target): solid-checker: @kobalte/themes has no supported ESM runtime entrypoints
- **@kobalte/utils@0.9.2** (only, unclassified): solid-checker: @kobalte/utils has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file /private/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-08Lkff/node_modules/@kobalte/utils/dist/index.js exports "EventKey", whose runtime kind no closed type answers (Unknown); publishing kind "value" would certify it invokes no caller-supplied cal... _(stderr truncated for readability)_

### Solid Primitives

- Compatible packages: 97
- Probes run: 97
- Declared entrypoints: 94
- Generated entrypoints: 94
- Refused entrypoints (partial contracts): 0
- Success (complete contracts): 91/97 (93.81%)
- Partial contracts: 0
- Failures: 6

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @solid-primitives/active-element | 2.1.6 | only | success | success |
| @solid-primitives/analytics | 0.2.1 | only | failure | unclassified |
| @solid-primitives/audio | 1.4.5 | only | failure | unclassified |
| @solid-primitives/autofocus | 0.1.5 | only | success | success |
| @solid-primitives/bounds | 0.1.7 | only | success | success |
| @solid-primitives/broadcast-channel | 0.1.1 | only | success | success |
| @solid-primitives/clipboard | 1.6.6 | only | success | success |
| @solid-primitives/composites | 1.1.1 | only | failure | no-esm-runtime-target |
| @solid-primitives/connectivity | 0.4.6 | only | success | success |
| @solid-primitives/context | 0.3.2 | only | success | success |
| @solid-primitives/controlled-props | 0.1.4 | only | success | success |
| @solid-primitives/cookies | 0.0.3 | only | success | success |
| @solid-primitives/cookies-store | 1.1.11 | only | failure | unclassified |
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
| @solid-primitives/intersection-observer | 2.2.5 | only | failure | unclassified |
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
| @solid-primitives/platform | 0.2.1 | only | failure | unclassified |
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
| @solid-primitives/sse | 0.0.103 | only | success | success |
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
| @solid-primitives/utils | 6.4.1 | only | success | success |
| @solid-primitives/virtual | 0.2.5 | only | success | success |
| @solid-primitives/visibility-observer | 2.0.1 | only | success | success |
| @solid-primitives/websocket | 1.4.0 | only | success | success |
| @solid-primitives/workers | 0.4.3 | only | success | success |

Failure groups:
- 1x no-esm-runtime-target: @solid-primitives/composites has no supported ESM runtime entrypoints; legacy module target does not exist or is unsupported: ./dist/index.js (packages: @solid-primitives/composites)
- 1x unclassified: @solid-primitives/analytics has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/analytics)
- 1x unclassified: @solid-primitives/audio has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/audio)
- 1x unclassified: @solid-primitives/cookies-store has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/cookies-store)
- 1x unclassified: @solid-primitives/intersection-observer has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/intersection-observer)
- 1x unclassified: @solid-primitives/platform has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/platform)

Failure details:
- **@solid-primitives/analytics@0.2.1** (only, unclassified): solid-checker: @solid-primitives/analytics has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file /private/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-0oTpon/node_modules/@solid-primitives/analytics/dist/index.js exports "EventType", whose runtime kind no closed type answers (Unknown); publishing kind "value" would certify it inv... _(stderr truncated for readability)_
- **@solid-primitives/audio@1.4.5** (only, unclassified): solid-checker: @solid-primitives/audio has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file /private/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-fstESr/node_modules/@solid-primitives/audio/dist/index.js exports "AudioState", whose runtime kind no closed type answers (Unknown); publishing kind "value" would certify it invokes no... _(stderr truncated for readability)_
- **@solid-primitives/composites@1.1.1** (only, no-esm-runtime-target): solid-checker: @solid-primitives/composites has no supported ESM runtime entrypoints; legacy module target does not exist or is unsupported: ./dist/index.js
- **@solid-primitives/cookies-store@1.1.11** (only, unclassified): solid-checker: @solid-primitives/cookies-store has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file /private/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-ICYZRq/node_modules/@solid-primitives/cookies-store/dist/index.js exports "CookieSitePolicy", whose runtime kind no closed type answers (Unknown); publishing kind "value" would... _(stderr truncated for readability)_
- **@solid-primitives/intersection-observer@2.2.5** (only, unclassified): solid-checker: @solid-primitives/intersection-observer has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file /private/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-fVYhSl/node_modules/@solid-primitives/intersection-observer/dist/index.js exports "DirectionX", whose runtime kind no closed type answers (Unknown); publishing kind "va... _(stderr truncated for readability)_
- **@solid-primitives/platform@0.2.1** (only, unclassified): solid-checker: @solid-primitives/platform has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file /private/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-oLP2Py/node_modules/@solid-primitives/platform/dist/index.js exports "isBrave", whose runtime kind no closed type answers (Unknown); publishing kind "value" would certify it invokes... _(stderr truncated for readability)_

### Corvu

- Compatible packages: 11
- Probes run: 11
- Declared entrypoints: 14
- Generated entrypoints: 35
- Refused entrypoints (partial contracts): 0
- Success (complete contracts): 11/11 (100%)
- Partial contracts: 0
- Failures: 0

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
| @corvu/utils | 0.4.2 | only | success | success |
| corvu | 0.7.2 | only | success | success |

### TanStack

- Compatible packages: 36
- Probes run: 36
- Declared entrypoints: 230
- Generated entrypoints: 188
- Refused entrypoints (partial contracts): 8
- Success (complete contracts): 27/36 (75%)
- Partial contracts: 7
- Failures: 2

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @tanstack/ai-devtools-core | 0.5.6 | only | failure | unclassified |
| @tanstack/ai-solid | 0.18.3 | only | success | success |
| @tanstack/ai-solid-ui | 0.7.18 | only | success | success |
| @tanstack/charts | 0.14.0 | only | partial-success | partial-success |
| @tanstack/devtools | 0.14.2 | only | success | success |
| @tanstack/devtools-a11y | 0.2.2 | only | success | success |
| @tanstack/devtools-ui | 0.7.1 | only | success | success |
| @tanstack/devtools-utils | 0.7.0 | only | success | success |
| @tanstack/form-devtools | 1.0.0-alpha.2 | only | partial-success | partial-success |
| @tanstack/hotkeys-devtools | 0.9.0 | only | partial-success | partial-success |
| @tanstack/pacer-devtools | 1.4.0 | only | success | success |
| @tanstack/solid-ai-devtools | 0.2.70 | only | success | success |
| @tanstack/solid-charts | 0.14.0 | only | success | success |
| @tanstack/solid-db | 0.2.37 | only | success | success |
| @tanstack/solid-devtools | 0.8.12 | only | success | success |
| @tanstack/solid-form | 2.0.0-alpha.2 | only | success | success |
| @tanstack/solid-form-devtools | 1.0.0-alpha.2 | only | success | success |
| @tanstack/solid-hotkeys | 0.10.0 | only | success | success |
| @tanstack/solid-hotkeys-devtools | 0.7.0 | only | failure | unclassified |
| @tanstack/solid-pacer | 0.22.0 | only | partial-success | partial-success |
| @tanstack/solid-pacer-devtools | 0.14.0 | only | success | success |
| @tanstack/solid-query | 5.101.4 | only | success | success |
| @tanstack/solid-query-devtools | 5.101.4 | only | success | success |
| @tanstack/solid-query-persist-client | 5.101.4 | only | success | success |
| @tanstack/solid-router | 1.170.29 | only | partial-success | partial-success |
| @tanstack/solid-router-devtools | 1.167.1 | only | success | success |
| @tanstack/solid-router-ssr-query | 1.167.2-pre.0 | only | success | success |
| @tanstack/solid-start | 1.168.46 | only | success | success |
| @tanstack/solid-start-client | 1.168.28 | only | success | success |
| @tanstack/solid-start-config | 1.120.20 | only | success | success |
| @tanstack/solid-start-server | 1.167.35 | only | success | success |
| @tanstack/solid-store | 0.11.1 | only | success | success |
| @tanstack/solid-table | 9.1.2 | only | success | success |
| @tanstack/solid-table-devtools | 9.2.0 | only | partial-success | partial-success |
| @tanstack/solid-virtual | 3.13.37 | only | success | success |
| @tanstack/table-devtools | 9.2.0 | only | partial-success | partial-success |

Failure groups:
- 1x unclassified: @tanstack/ai-devtools-core has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", which destructures a member of another value, so no fact here rules out a class; publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @tanstack/ai-devtools-core)
- 1x unclassified: @tanstack/solid-hotkeys-devtools has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @tanstack/solid-hotkeys-devtools)

Failure details:
- **@tanstack/ai-devtools-core@0.5.6** (only, unclassified): solid-checker: @tanstack/ai-devtools-core has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file /private/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-cOIkcQ/node_modules/@tanstack/ai-devtools-core/dist/server.js exports "AiDevtoolsCore", which destructures a member of another value, so no fact here rules out a class; publishing k... _(stderr truncated for readability)_
- **@tanstack/solid-hotkeys-devtools@0.7.0** (only, unclassified): solid-checker: @tanstack/solid-hotkeys-devtools has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file /private/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-Y8hUdv/node_modules/@tanstack/solid-hotkeys-devtools/dist/index.js exports "HotkeysDevtoolsPanel", whose runtime kind no closed type answers (Unknown); publishing kind "value"... _(stderr truncated for readability)_

### Solid Devtools

- Compatible packages: 12
- Probes run: 12
- Declared entrypoints: 21
- Generated entrypoints: 19
- Refused entrypoints (partial contracts): 4
- Success (complete contracts): 6/12 (50%)
- Partial contracts: 3
- Failures: 3

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @solid-devtools/babel-plugin | 0.3.1 | only | failure | cjs-only-entrypoint |
| @solid-devtools/debugger | 0.28.1 | only | partial-success | partial-success |
| @solid-devtools/ext-adapter | 0.17.0 | only | failure | no-exported-surface |
| @solid-devtools/extension-adapter | 0.12.1 | only | success | success |
| @solid-devtools/frontend | 0.15.4 | only | success | success |
| @solid-devtools/locator | 0.16.7 | only | failure | unclassified |
| @solid-devtools/logger | 0.9.11 | only | success | success |
| @solid-devtools/overlay | 0.33.5 | only | success | success |
| @solid-devtools/shared | 0.20.0 | only | success | success |
| @solid-devtools/transform | 0.10.4 | only | success | success |
| @solid-devtools/ui | 0.10.3 | only | partial-success | partial-success |
| solid-devtools | 0.34.5 | only | partial-success | partial-success |

Failure groups:
- 1x no-exported-surface: @solid-devtools/ext-adapter has no runtime ESM exports (packages: @solid-devtools/ext-adapter)
- 1x cjs-only-entrypoint: . has only a CJS runtime target; CJS contract generation is unsupported (packages: @solid-devtools/babel-plugin)
- 1x unclassified: @solid-devtools/locator has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-devtools/locator)

Failure details:
- **@solid-devtools/babel-plugin@0.3.1** (only, cjs-only-entrypoint): solid-checker: . has only a CJS runtime target; CJS contract generation is unsupported
- **@solid-devtools/ext-adapter@0.17.0** (only, no-exported-surface): solid-checker: @solid-devtools/ext-adapter has no runtime ESM exports
- **@solid-devtools/locator@0.16.7** (only, unclassified): solid-checker: @solid-devtools/locator has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file /private/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-Ar5V6K/node_modules/@solid-devtools/locator/dist/index.js exports "addClickInterceptor", whose runtime kind no closed type answers (Unknown); publishing kind "value" would certify it i... _(stderr truncated for readability)_

### Solid Recharts

- Compatible packages: 1
- Probes run: 1
- Declared entrypoints: 1
- Generated entrypoints: 1
- Refused entrypoints (partial contracts): 0
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
- Generated entrypoints: 2
- Refused entrypoints (partial contracts): 0
- Success (complete contracts): 1/1 (100%)
- Partial contracts: 0
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| motion-solidjs | 0.6.0 | only | success | success |

**Solid 1.x totals:** 143/168 (85.12%) complete, 11 partial, 14 failed

## Solid 2.x

### Official Solid

- Compatible packages: 11
- Probes run: 17
- Declared entrypoints: 60
- Generated entrypoints: 47
- Refused entrypoints (partial contracts): 0
- Success (complete contracts): 17/17 (100%)
- Partial contracts: 0
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @solidjs/element | 2.0.0-rc.1 | only | success | success |
| @solidjs/h | 2.0.0-rc.1 | only | success | success |
| @solidjs/html | 2.0.0-rc.1 | only | success | success |
| @solidjs/meta | 1.0.0-next.2 | floor | success | success |
| @solidjs/meta | 1.0.0-next.2 | head | success | success |
| @solidjs/router | 2.0.0-next.17 | only | success | success |
| @solidjs/signals | 2.0.0-rc.1 | floor | success | success |
| @solidjs/signals | 2.0.0-rc.1 | head | success | success |
| @solidjs/start-devtools | 1.0.0-next.3 | floor | success | success |
| @solidjs/start-devtools | 1.0.0-next.3 | head | success | success |
| @solidjs/universal | 2.0.0-rc.1 | only | success | success |
| @solidjs/vite-plugin | 3.0.0-next.31 | floor | success | success |
| @solidjs/vite-plugin | 3.0.0-next.31 | head | success | success |
| @solidjs/web | 2.0.0-rc.1 | floor | success | success |
| @solidjs/web | 2.0.0-rc.1 | head | success | success |
| solid-js | 2.0.0-rc.1 | floor | success | success |
| solid-js | 2.0.0-rc.1 | head | success | success |

### Kobalte

- Compatible packages: 2
- Probes run: 2
- Declared entrypoints: 3
- Generated entrypoints: 62
- Refused entrypoints (partial contracts): 0
- Success (complete contracts): 2/2 (100%)
- Partial contracts: 0
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @kobalte/core | 2.0.0-alpha.0 | only | success | success |
| @kobalte/utils | 2.0.0-alpha.0 | only | success | success |

### Solid Primitives

- Compatible packages: 96
- Probes run: 192
- Declared entrypoints: 210
- Generated entrypoints: 206
- Refused entrypoints (partial contracts): 0
- Success (complete contracts): 190/192 (98.96%)
- Partial contracts: 0
- Failures: 2

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @solid-primitives/a11y | 1.0.0-next.3 | floor | success | success |
| @solid-primitives/a11y | 1.0.0-next.3 | head | success | success |
| @solid-primitives/active-element | 3.0.0-next.2 | floor | success | success |
| @solid-primitives/active-element | 3.0.0-next.2 | head | success | success |
| @solid-primitives/analytics | 2.0.0-next.2 | floor | success | success |
| @solid-primitives/analytics | 2.0.0-next.2 | head | success | success |
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
| @solid-primitives/controlled-props | 1.0.0-next.3 | floor | success | success |
| @solid-primitives/controlled-props | 1.0.0-next.3 | head | success | success |
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
| @solid-primitives/form | 1.0.0-next.2 | floor | success | success |
| @solid-primitives/form | 1.0.0-next.2 | head | success | success |
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
| @solid-primitives/intersection-observer | 3.0.0-next.3 | floor | success | success |
| @solid-primitives/intersection-observer | 3.0.0-next.3 | head | success | success |
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
| @solid-primitives/platform | 1.0.0-next.2 | floor | failure | unclassified |
| @solid-primitives/platform | 1.0.0-next.2 | head | failure | unclassified |
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
| @solid-primitives/sse | 1.0.0-next.2 | floor | success | success |
| @solid-primitives/sse | 1.0.0-next.2 | head | success | success |
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
| @solid-primitives/virtual | 1.0.0-next.4 | floor | success | success |
| @solid-primitives/virtual | 1.0.0-next.4 | head | success | success |
| @solid-primitives/websocket | 2.0.0-next.3 | floor | success | success |
| @solid-primitives/websocket | 2.0.0-next.3 | head | success | success |
| @solid-primitives/workers | 2.0.1-next.1 | floor | success | success |
| @solid-primitives/workers | 2.0.1-next.1 | head | success | success |

Failure groups:
- 2x unclassified: @solid-primitives/platform has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/platform)

Failure details:
- **@solid-primitives/platform@1.0.0-next.2** (floor, unclassified): solid-checker: @solid-primitives/platform has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file /private/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-6znJnm/node_modules/@solid-primitives/platform/dist/index.js exports "isBrave", whose runtime kind no closed type answers (Unknown); publishing kind "value" would certify it invokes... _(stderr truncated for readability)_
- **@solid-primitives/platform@1.0.0-next.2** (head, unclassified): solid-checker: @solid-primitives/platform has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file /private/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-F8b5C1/node_modules/@solid-primitives/platform/dist/index.js exports "isBrave", whose runtime kind no closed type answers (Unknown); publishing kind "value" would certify it invokes... _(stderr truncated for readability)_

### Corvu

- Compatible packages: 17
- Probes run: 17
- Declared entrypoints: 20
- Generated entrypoints: 33
- Refused entrypoints (partial contracts): 0
- Success (complete contracts): 17/17 (100%)
- Partial contracts: 0
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
| @corvu-next/utils | 0.1.5 | only | success | success |

### TanStack

- Compatible packages: 9
- Probes run: 16
- Declared entrypoints: 50
- Generated entrypoints: 38
- Refused entrypoints (partial contracts): 0
- Success (complete contracts): 14/16 (87.5%)
- Partial contracts: 0
- Failures: 2

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @tanstack/solid-query | 6.0.0-rc.0 | floor | success | success |
| @tanstack/solid-query | 6.0.0-rc.0 | head | success | success |
| @tanstack/solid-query-devtools | 6.0.0-rc.0 | floor | success | success |
| @tanstack/solid-query-devtools | 6.0.0-rc.0 | head | success | success |
| @tanstack/solid-query-persist-client | 6.0.0-rc.0 | floor | success | success |
| @tanstack/solid-query-persist-client | 6.0.0-rc.0 | head | success | success |
| @tanstack/solid-router | 2.0.0-rc.1 | only | success | success |
| @tanstack/solid-router-devtools | 2.0.0-rc.1 | only | success | success |
| @tanstack/solid-router-ssr-query | 2.0.0-rc.1 | floor | failure | install-failure |
| @tanstack/solid-router-ssr-query | 2.0.0-rc.1 | head | failure | install-failure |
| @tanstack/solid-start | 2.0.0-rc.1 | floor | success | success |
| @tanstack/solid-start | 2.0.0-rc.1 | head | success | success |
| @tanstack/solid-start-client | 2.0.0-rc.1 | floor | success | success |
| @tanstack/solid-start-client | 2.0.0-rc.1 | head | success | success |
| @tanstack/solid-start-server | 2.0.0-rc.1 | floor | success | success |
| @tanstack/solid-start-server | 2.0.0-rc.1 | head | success | success |

Failure groups:
- 2x install-failure: npm error code ERESOLVE npm error ERESOLVE unable to resolve dependency tree npm error npm error While resolving: solid-checker-ecosystem-probe@ npm error Found: solid-js@ npm error node_modules/solid-js npm error solid-js@"<value>" from the root project npm error peer solid-js@"<value>" from @tanstack/solid-router-ssr-query@ npm error node_modules/@tanstack/solid-router-ssr-query npm error @tanstack/solid-router-ssr-query@"<value>" from the root project npm error 1 more (@solidjs/web) npm error npm error Could not resolve dependency: npm error peer solid-js@"<value>" from @tanstack/solid-query@ npm error node_modules/@tanstack/solid-query npm error peer @tanstack/solid-query@"<value>" from @tanstack/solid-router-ssr-query@ npm error node_modules/@tanstack/solid-router-ssr-query npm error @tanstack/solid-router-ssr-query@"<value>" from the root project npm error npm error Fix the upstream dependency conflict, or retry npm error this command with --force or --legacy-peer-deps npm error to accept an incorrect (and potentially broken) dependency resolution. npm error npm error npm error For a full report see: npm error npm error A complete log of this run can be found in: (packages: @tanstack/solid-router-ssr-query)

Failure details:
- **@tanstack/solid-router-ssr-query@2.0.0-rc.1** (floor, install-failure): npm error code ERESOLVE npm error ERESOLVE unable to resolve dependency tree npm error npm error While resolving: solid-checker-ecosystem-probe@0.0.0 npm error Found: solid-js@2.0.0-rc.0 npm error node_modules/solid-js npm error solid-js@"2.0.0-rc.0" from the root project npm error peer solid-js@">=2.0.0-beta.17" from @tanstack/solid-router-ssr-query@2.0.0-rc.1 npm error node_modules/@tanstack/sol... _(stderr truncated for readability)_
- **@tanstack/solid-router-ssr-query@2.0.0-rc.1** (head, install-failure): npm error code ERESOLVE npm error ERESOLVE unable to resolve dependency tree npm error npm error While resolving: solid-checker-ecosystem-probe@0.0.0 npm error Found: solid-js@2.0.0-rc.1 npm error node_modules/solid-js npm error solid-js@"2.0.0-rc.1" from the root project npm error peer solid-js@">=2.0.0-beta.17" from @tanstack/solid-router-ssr-query@2.0.0-rc.1 npm error node_modules/@tanstack/sol... _(stderr truncated for readability)_

### Solid Devtools

- Compatible packages: 0
- Probes run: 0
- Declared entrypoints: 0
- Generated entrypoints: 0
- Refused entrypoints (partial contracts): 0
- Success (complete contracts): 0/0 (no probes run)
- Partial contracts: 0
- Failures: 0

### Solid Recharts

- Compatible packages: 1
- Probes run: 2
- Declared entrypoints: 2
- Generated entrypoints: 2
- Refused entrypoints (partial contracts): 0
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
- Generated entrypoints: 6
- Refused entrypoints (partial contracts): 0
- Success (complete contracts): 2/2 (100%)
- Partial contracts: 0
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| motion-solidjs | 0.7.0-beta.4 | floor | success | success |
| motion-solidjs | 0.7.0-beta.4 | head | success | success |

**Solid 2.x totals:** 244/248 (98.39%) complete, 0 partial, 4 failed

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
- @solidjs/element@2.0.0-rc.1 (official-solid)
- @solidjs/h@2.0.0-rc.1 (official-solid)
- @solidjs/html@2.0.0-rc.1 (official-solid)
- @solidjs/router@2.0.0-next.17 (official-solid)
- @solidjs/universal@2.0.0-rc.1 (official-solid)
- @tanstack/solid-router@2.0.0-rc.1 (tanstack)
- @tanstack/solid-router-devtools@2.0.0-rc.1 (tanstack)

### Worse at head than at floor

None.

### Better at head than at floor

None.

## Contract content (what the emitted contracts claim)

- Contracts measured: 398 probe(s) across 202 package(s)
- Probes fully proven (no unknown claim, no refused entrypoint, no closure note): 125/398 (31.41%)
- Packages fully proven (every one of their probes): 44/202 (21.78%)
- Probes with at least one unknown claim: 269
- Probes with at least one refused entrypoint: 11
- Probes with at least one closure note: 7
- Exports proven: 4444/8082 (54.99%) (with unknown: 3638, without a summary: 0)
- Of those unknown exports: 528 unknown in ALL five domains (the generator said nothing about them at all), 0 unknown only inside a conditional variant (the default resolution is fully claimed)
- Entrypoints: 829 emitted, 13 refused
- Closure notes (block byte-attested verification): 31

### Unknown claims by domain

| Domain | Exports carrying an unknown |
| --- | --- |
| callbacks | 2337 |
| reactiveReads | 2067 |
| returns | 2175 |
| ownerRequirements | 528 |
| asyncBehavior | 529 |
| **total** | **7636** |

Read the five columns together, not separately: 528 of the 3638 unknown exports are unknown in every domain at once, so most of each column is the same exports counted five times.

### Positive behavioral rows (what a probe step would have to drive)

| Row kind | Count |
| --- | --- |
| callbackExecution | 1247 |
| reactiveRead | 1156 |
| returnTree | 1165 |
| ownerRequirement | 533 |
| asyncBehavior | 100 |

### Contract content by family

| Family | Contracts | Fully proven | With unknowns | With refusals | Exports proven | Unknown claims |
| --- | --- | --- | --- | --- | --- | --- |
| Official Solid | 23 | 1/23 (4.35%) | 20 | 1 | 906/1470 (61.63%) | 740 |
| Kobalte | 3 | 0/3 (0%) | 3 | 0 | 252/1147 (21.97%) | 2376 |
| Solid Primitives | 281 | 112/281 (39.86%) | 169 | 0 | 1453/1949 (74.55%) | 790 |
| Corvu | 28 | 3/28 (10.71%) | 25 | 0 | 188/266 (70.68%) | 121 |
| TanStack | 48 | 7/48 (14.58%) | 39 | 7 | 1396/2121 (65.82%) | 1251 |
| Solid Devtools | 9 | 2/9 (22.22%) | 7 | 3 | 149/184 (80.98%) | 66 |
| Solid Recharts | 3 | 0/3 (0%) | 3 | 0 | 16/327 (4.89%) | 644 |
| Motion for Solid | 3 | 0/3 (0%) | 3 | 0 | 84/618 (13.59%) | 1648 |

### Most unknown claims

| Package | Solid | Unknown claims | Exports with unknown / total | All five domains | Variant-only | Dominant cause |
| --- | --- | --- | --- | --- | --- | --- |
| @kobalte/core@2.0.0-alpha.0 | solid2 | 1446 | 436/526 | 199 | 0 | reactiveReads |
| motion-solidjs@0.7.0-beta.4 | solid2 | 1245 | 249/261 | 249 | 0 | all-domains |
| @kobalte/core@0.13.13 | solid1 | 929 | 458/611 | 0 | 0 | returns |
| @tanstack/solid-router@2.0.0-rc.1 | solid2 | 272 | 107/120 | 0 | 0 | reactiveReads |
| solid-recharts@1.0.1 | solid1 | 216 | 103/109 | 0 | 0 | reactiveReads |
| solid-recharts@2.0.0-beta.1 | solid2 | 214 | 104/109 | 0 | 0 | reactiveReads |
| solid-recharts@2.0.0-beta.1 | solid2 | 214 | 104/109 | 0 | 0 | reactiveReads |
| motion-solidjs@0.7.0-beta.4 | solid2 | 203 | 203/261 | 0 | 0 | callbacks |
| motion-solidjs@0.6.0 | solid1 | 200 | 82/96 | 0 | 0 | reactiveReads |
| @tanstack/solid-query@6.0.0-rc.0 | solid2 | 119 | 47/57 | 0 | 0 | reactiveReads |
| @tanstack/solid-query@6.0.0-rc.0 | solid2 | 119 | 47/57 | 0 | 0 | reactiveReads |
| @tanstack/solid-router@1.170.29 | solid1 | 115 | 23/23 | 23 | 0 | all-domains |
| @tanstack/solid-db@0.2.37 | solid1 | 113 | 113/207 | 0 | 0 | callbacks |
| @tanstack/charts@0.14.0 | solid1 | 102 | 102/374 | 0 | 0 | callbacks |
| solid-js@1.9.14 | solid1 | 99 | 67/126 | 3 | 0 | callbacks |

These figures describe the GENERATED DRAFT, not consumer findings. An unknown claim becomes a finding only when a consumer actually touches that surface, so a package with many unknowns on exports nobody imports costs a real project nothing. Nothing here has been reviewed or probed: every claim counted as proven is still inferred evidence awaiting review, and a closure note means the contract cannot be byte-attested at all.

## Combined

### Worker timings

- Worker time: 679853 ms
- Phases: install 489800 ms, generation 189236 ms, harness 817 ms

### Top failure signatures

- 3x unclassified: @solid-primitives/platform has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/platform)
- 2x install-failure: npm error code ERESOLVE npm error ERESOLVE unable to resolve dependency tree npm error npm error While resolving: solid-checker-ecosystem-probe@ npm error Found: solid-js@ npm error node_modules/solid-js npm error solid-js@"<value>" from the root project npm error peer solid-js@"<value>" from @tanstack/solid-router-ssr-query@ npm error node_modules/@tanstack/solid-router-ssr-query npm error @tanstack/solid-router-ssr-query@"<value>" from the root project npm error 1 more (@solidjs/web) npm error npm error Could not resolve dependency: npm error peer solid-js@"<value>" from @tanstack/solid-query@ npm error node_modules/@tanstack/solid-query npm error peer @tanstack/solid-query@"<value>" from @tanstack/solid-router-ssr-query@ npm error node_modules/@tanstack/solid-router-ssr-query npm error @tanstack/solid-router-ssr-query@"<value>" from the root project npm error npm error Fix the upstream dependency conflict, or retry npm error this command with --force or --legacy-peer-deps npm error to accept an incorrect (and potentially broken) dependency resolution. npm error npm error npm error For a full report see: npm error npm error A complete log of this run can be found in: (packages: @tanstack/solid-router-ssr-query)
- 1x no-esm-runtime-target: @kobalte/themes has no supported ESM runtime entrypoints (packages: @kobalte/themes)
- 1x no-esm-runtime-target: @solid-primitives/composites has no supported ESM runtime entrypoints; legacy module target does not exist or is unsupported: ./dist/index.js (packages: @solid-primitives/composites)
- 1x no-exported-surface: @solid-devtools/ext-adapter has no runtime ESM exports (packages: @solid-devtools/ext-adapter)
- 1x cjs-only-entrypoint: . has only a CJS runtime target; CJS contract generation is unsupported (packages: @solid-devtools/babel-plugin)
- 1x install-failure: npm error code ERESOLVE npm error ERESOLVE could not resolve npm error npm error While resolving: @solidjs/start@ npm error Found: @solidjs/router@ npm error node_modules/@solidjs/router npm error @solidjs/router@"<value>" from @kobalte/solidbase@ npm error node_modules/@kobalte/solidbase npm error @kobalte/solidbase@"<value>" from the root project npm error npm error Could not resolve dependency: npm error peerOptional @solidjs/router@"<value>" from @solidjs/start@ npm error node_modules/@solidjs/start npm error peer @solidjs/start@"<value>" from @kobalte/solidbase@ npm error node_modules/@kobalte/solidbase npm error @kobalte/solidbase@"<value>" from the root project npm error npm error Conflicting peer dependency: @solidjs/router@ npm error node_modules/@solidjs/router npm error peerOptional @solidjs/router@"<value>" from @solidjs/start@ npm error node_modules/@solidjs/start npm error peer @solidjs/start@"<value>" from @kobalte/solidbase@ npm error node_modules/@kobalte/solidbase npm error @kobalte/solidbase@"<value>" from the root project npm error npm error Fix the upstream dependency conflict, or retry npm error this command with --force or --legacy-peer-deps npm error to accept an incorrect (and potentially broken) dependency resolution. npm error npm error npm error For a full report see: npm error npm error A complete log of this run can be found in: (packages: @kobalte/solidbase)
- 1x unclassified: @kobalte/utils has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @kobalte/utils)
- 1x unclassified: @solid-devtools/locator has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-devtools/locator)
- 1x unclassified: @solid-primitives/analytics has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/analytics)
- 1x unclassified: @solid-primitives/audio has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/audio)
- 1x unclassified: @solid-primitives/cookies-store has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/cookies-store)
- 1x unclassified: @solid-primitives/intersection-observer has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/intersection-observer)
- 1x unclassified: @tanstack/ai-devtools-core has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", which destructures a member of another value, so no fact here rules out a class; publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @tanstack/ai-devtools-core)
- 1x unclassified: @tanstack/solid-hotkeys-devtools has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @tanstack/solid-hotkeys-devtools)

### Partial contracts

- @solid-devtools/debugger@0.28.1 (solid-devtools): 4 entrypoint(s) generated, 2 refused
- @solid-devtools/ui@0.10.3 (solid-devtools): 3 entrypoint(s) generated, 1 refused
- @tanstack/charts@0.14.0 (tanstack): 110 entrypoint(s) generated, 2 refused
- @tanstack/form-devtools@1.0.0-alpha.2 (tanstack): 1 entrypoint(s) generated, 1 refused
- @tanstack/hotkeys-devtools@0.9.0 (tanstack): 1 entrypoint(s) generated, 1 refused
- @tanstack/solid-pacer@0.22.0 (tanstack): 13 entrypoint(s) generated, 1 refused
- @tanstack/solid-router@1.170.29 (tanstack): 2 entrypoint(s) generated, 1 refused
- @tanstack/solid-table-devtools@9.2.0 (tanstack): 1 entrypoint(s) generated, 1 refused
- @tanstack/table-devtools@9.2.0 (tanstack): 1 entrypoint(s) generated, 1 refused
- solid-devtools@0.34.5 (solid-devtools): 2 entrypoint(s) generated, 1 refused
- solid-js@1.9.14 (official-solid): 10 entrypoint(s) generated, 1 refused

### Shared dependency blockers

None.

### Multi-blocker packages

None.

### Family comparison (Solid 1.x vs Solid 2.x)

| Family | Solid 1.x complete/total | Solid 2.x complete/total |
| --- | --- | --- |
| Official Solid | 5/6 (83.33%) | 17/17 (100%) |
| Kobalte | 1/4 (25%) | 2/2 (100%) |
| Solid Primitives | 91/97 (93.81%) | 190/192 (98.96%) |
| Corvu | 11/11 (100%) | 17/17 (100%) |
| TanStack | 27/36 (75%) | 14/16 (87.5%) |
| Solid Devtools | 6/12 (50%) | 0/0 (no probes run) |
| Solid Recharts | 1/1 (100%) | 2/2 (100%) |
| Motion for Solid | 1/1 (100%) | 2/2 (100%) |

### Discovery limitations

- packument for "@tanstack/tests-adapters" is unavailable (registry returned nothing for it)

### Unavailable metadata

None.

### Baseline comparison

No baseline supplied.
