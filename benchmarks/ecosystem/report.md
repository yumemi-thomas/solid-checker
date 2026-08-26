# Ecosystem Benchmark Report

- Started: 2026-08-26T17:05:43.891Z
- Finished: 2026-08-26T17:16:10.857Z
- Duration: 626966 ms
- Checker native binary: /tmp/solid-checker-phase0-binaries/solid-checker-rust
- Type Facts binary: /tmp/solid-checker-phase0-binaries/solid-typefacts
- Manifest generated at: 2026-08-26T14:21:49.573Z (rows: 307, probes: 418)
- Scope: full corpus (418 probes run)

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
- Declared entrypoints: 14
- Generated entrypoints: 74
- Refused entrypoints (partial contracts): 1
- Success (complete contracts): 1/4 (25%)
- Partial contracts: 1
- Failures: 2

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @kobalte/core | 0.13.13 | only | success | success |
| @kobalte/solidbase | 0.6.13 | only | partial-success | partial-success |
| @kobalte/themes | 0.0.1-next.0 | only | failure | no-esm-runtime-target |
| @kobalte/utils | 0.9.2 | only | failure | export-kind-unresolved |

Failure groups:
- 1x no-esm-runtime-target: @kobalte/themes has no supported ESM runtime entrypoints (packages: @kobalte/themes)
- 1x export-kind-unresolved: @kobalte/utils has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @kobalte/utils)

Failure details:
- **@kobalte/themes@0.0.1-next.0** (only, no-esm-runtime-target): solid-checker: @kobalte/themes has no supported ESM runtime entrypoints
- **@kobalte/utils@0.9.2** (only, export-kind-unresolved): solid-checker: @kobalte/utils has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file /private/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-frrhXM/node_modules/@kobalte/utils/dist/index.js exports "EventKey", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "value" would certify it invokes no caller-sup... _(stderr truncated for readability)_

### Solid Primitives

- Compatible packages: 97
- Probes run: 97
- Declared entrypoints: 94
- Generated entrypoints: 93
- Refused entrypoints (partial contracts): 0
- Success (complete contracts): 90/97 (92.78%)
- Partial contracts: 0
- Failures: 7

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @solid-primitives/active-element | 2.1.6 | only | success | success |
| @solid-primitives/analytics | 0.2.1 | only | failure | export-kind-unresolved |
| @solid-primitives/audio | 1.4.5 | only | failure | export-kind-unresolved |
| @solid-primitives/autofocus | 0.1.5 | only | success | success |
| @solid-primitives/bounds | 0.1.7 | only | success | success |
| @solid-primitives/broadcast-channel | 0.1.1 | only | success | success |
| @solid-primitives/clipboard | 1.6.6 | only | success | success |
| @solid-primitives/composites | 1.1.1 | only | failure | no-esm-runtime-target |
| @solid-primitives/connectivity | 0.4.6 | only | success | success |
| @solid-primitives/context | 0.3.2 | only | success | success |
| @solid-primitives/controlled-props | 0.1.4 | only | success | success |
| @solid-primitives/cookies | 0.0.3 | only | success | success |
| @solid-primitives/cookies-store | 1.1.11 | only | failure | export-kind-unresolved |
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
| @solid-primitives/geolocation | 1.5.5 | only | failure | export-kind-conflict |
| @solid-primitives/gestures | 1.2.1 | only | success | success |
| @solid-primitives/graphql | 3.0.0-next.0 | only | success | success |
| @solid-primitives/history | 0.2.5 | only | success | success |
| @solid-primitives/i18n | 2.2.1 | only | success | success |
| @solid-primitives/idle | 0.2.3 | only | success | success |
| @solid-primitives/immutable | 2.0.0-next.0 | only | success | success |
| @solid-primitives/input-mask | 0.3.1 | only | success | success |
| @solid-primitives/intersection-observer | 2.2.5 | only | failure | export-kind-unresolved |
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
| @solid-primitives/platform | 0.2.1 | only | failure | export-kind-unresolved |
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
- 1x export-kind-unresolved: @solid-primitives/analytics has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/analytics)
- 1x export-kind-unresolved: @solid-primitives/audio has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/audio)
- 1x export-kind-unresolved: @solid-primitives/cookies-store has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/cookies-store)
- 1x export-kind-unresolved: @solid-primitives/intersection-observer has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/intersection-observer)
- 1x export-kind-unresolved: @solid-primitives/platform has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/platform)
- 1x export-kind-conflict: package contract value export .:createGeolocation cannot have function effects (packages: @solid-primitives/geolocation)

Failure details:
- **@solid-primitives/analytics@0.2.1** (only, export-kind-unresolved): solid-checker: @solid-primitives/analytics has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file /private/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-ZXYnvM/node_modules/@solid-primitives/analytics/dist/index.js exports "EventType", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "value" would certi... _(stderr truncated for readability)_
- **@solid-primitives/audio@1.4.5** (only, export-kind-unresolved): solid-checker: @solid-primitives/audio has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file /private/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-cgEhE5/node_modules/@solid-primitives/audio/dist/index.js exports "AudioState", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "value" would certify it i... _(stderr truncated for readability)_
- **@solid-primitives/composites@1.1.1** (only, no-esm-runtime-target): solid-checker: @solid-primitives/composites has no supported ESM runtime entrypoints; legacy module target does not exist or is unsupported: ./dist/index.js
- **@solid-primitives/cookies-store@1.1.11** (only, export-kind-unresolved): solid-checker: @solid-primitives/cookies-store has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file /private/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-GoBfpg/node_modules/@solid-primitives/cookies-store/dist/index.js exports "CookieSitePolicy", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "val... _(stderr truncated for readability)_
- **@solid-primitives/geolocation@1.5.5** (only, export-kind-conflict): solid-checker: solid-checker-rust: package contract value export .:createGeolocation cannot have function effects
- **@solid-primitives/intersection-observer@2.2.5** (only, export-kind-unresolved): solid-checker: @solid-primitives/intersection-observer has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file /private/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-vBtqnT/node_modules/@solid-primitives/intersection-observer/dist/index.js exports "DirectionX", whose runtime kind no closed type answers (Unknown, Unknown); publishing... _(stderr truncated for readability)_
- **@solid-primitives/platform@0.2.1** (only, export-kind-unresolved): solid-checker: @solid-primitives/platform has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file /private/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-7KzQUW/node_modules/@solid-primitives/platform/dist/index.js exports "isBrave", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "value" would certify i... _(stderr truncated for readability)_

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
- Generated entrypoints: 73
- Refused entrypoints (partial contracts): 5
- Success (complete contracts): 30/36 (83.33%)
- Partial contracts: 5
- Failures: 1

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @tanstack/ai-devtools-core | 0.5.8 | only | success | success |
| @tanstack/ai-solid | 0.19.1 | only | success | success |
| @tanstack/ai-solid-ui | 0.7.20 | only | success | success |
| @tanstack/charts | 0.15.0 | only | success | success |
| @tanstack/devtools | 0.14.2 | only | success | success |
| @tanstack/devtools-a11y | 0.2.2 | only | success | success |
| @tanstack/devtools-ui | 0.7.1 | only | success | success |
| @tanstack/devtools-utils | 0.7.0 | only | success | success |
| @tanstack/form-devtools | 1.0.0-alpha.2 | only | success | success |
| @tanstack/hotkeys-devtools | 0.9.0 | only | partial-success | partial-success |
| @tanstack/pacer-devtools | 1.4.0 | only | success | success |
| @tanstack/solid-ai-devtools | 0.2.71 | only | success | success |
| @tanstack/solid-charts | 0.15.0 | only | success | success |
| @tanstack/solid-db | 0.2.40 | only | success | success |
| @tanstack/solid-devtools | 0.8.12 | only | success | success |
| @tanstack/solid-form | 2.0.0-alpha.2 | only | success | success |
| @tanstack/solid-form-devtools | 1.0.0-alpha.2 | only | success | success |
| @tanstack/solid-hotkeys | 0.10.0 | only | success | success |
| @tanstack/solid-hotkeys-devtools | 0.7.0 | only | failure | export-kind-unresolved |
| @tanstack/solid-pacer | 0.22.0 | only | partial-success | partial-success |
| @tanstack/solid-pacer-devtools | 0.14.0 | only | success | success |
| @tanstack/solid-query | 5.102.5 | only | success | success |
| @tanstack/solid-query-devtools | 5.102.5 | only | success | success |
| @tanstack/solid-query-persist-client | 5.102.5 | only | success | success |
| @tanstack/solid-router | 1.170.30 | only | partial-success | partial-success |
| @tanstack/solid-router-devtools | 1.167.1 | only | success | success |
| @tanstack/solid-router-ssr-query | 1.167.2-pre.0 | only | success | success |
| @tanstack/solid-start | 1.168.47 | only | success | success |
| @tanstack/solid-start-client | 1.168.29 | only | success | success |
| @tanstack/solid-start-config | 1.120.20 | only | success | success |
| @tanstack/solid-start-server | 1.167.36 | only | success | success |
| @tanstack/solid-store | 0.11.1 | only | success | success |
| @tanstack/solid-table | 9.1.2 | only | success | success |
| @tanstack/solid-table-devtools | 9.2.0 | only | partial-success | partial-success |
| @tanstack/solid-virtual | 3.13.37 | only | success | success |
| @tanstack/table-devtools | 9.2.0 | only | partial-success | partial-success |

Failure groups:
- 1x export-kind-unresolved: @tanstack/solid-hotkeys-devtools has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @tanstack/solid-hotkeys-devtools)

Failure details:
- **@tanstack/solid-hotkeys-devtools@0.7.0** (only, export-kind-unresolved): solid-checker: @tanstack/solid-hotkeys-devtools has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file /private/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-1ctzql/node_modules/@tanstack/solid-hotkeys-devtools/dist/index.js exports "HotkeysDevtoolsPanel", whose runtime kind no closed type answers (Unknown, Unknown); publishing kin... _(stderr truncated for readability)_

### Solid Devtools

- Compatible packages: 12
- Probes run: 12
- Declared entrypoints: 21
- Generated entrypoints: 20
- Refused entrypoints (partial contracts): 3
- Success (complete contracts): 7/12 (58.33%)
- Partial contracts: 2
- Failures: 3

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @solid-devtools/babel-plugin | 0.3.1 | only | failure | cjs-only-entrypoint |
| @solid-devtools/debugger | 0.28.1 | only | partial-success | partial-success |
| @solid-devtools/ext-adapter | 0.17.0 | only | failure | no-exported-surface |
| @solid-devtools/extension-adapter | 0.12.1 | only | success | success |
| @solid-devtools/frontend | 0.15.4 | only | success | success |
| @solid-devtools/locator | 0.16.7 | only | failure | export-kind-unresolved |
| @solid-devtools/logger | 0.9.11 | only | success | success |
| @solid-devtools/overlay | 0.33.5 | only | success | success |
| @solid-devtools/shared | 0.20.0 | only | success | success |
| @solid-devtools/transform | 0.10.4 | only | success | success |
| @solid-devtools/ui | 0.10.3 | only | success | success |
| solid-devtools | 0.34.5 | only | partial-success | partial-success |

Failure groups:
- 1x no-exported-surface: @solid-devtools/ext-adapter has no runtime ESM exports (packages: @solid-devtools/ext-adapter)
- 1x cjs-only-entrypoint: . has only a CJS runtime target; CJS contract generation is unsupported (packages: @solid-devtools/babel-plugin)
- 1x export-kind-unresolved: @solid-devtools/locator has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-devtools/locator)

Failure details:
- **@solid-devtools/babel-plugin@0.3.1** (only, cjs-only-entrypoint): solid-checker: . has only a CJS runtime target; CJS contract generation is unsupported
- **@solid-devtools/ext-adapter@0.17.0** (only, no-exported-surface): solid-checker: @solid-devtools/ext-adapter has no runtime ESM exports
- **@solid-devtools/locator@0.16.7** (only, export-kind-unresolved): solid-checker: @solid-devtools/locator has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file /private/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-GcRLlS/node_modules/@solid-devtools/locator/dist/index.js exports "addClickInterceptor", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "value" would cer... _(stderr truncated for readability)_

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
- Generated entrypoints: 0
- Refused entrypoints (partial contracts): 0
- Success (complete contracts): 0/1 (0%)
- Partial contracts: 0
- Failures: 1

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| motion-solidjs | 0.6.0 | only | failure | export-kind-unresolved |

Failure groups:
- 1x export-kind-unresolved: motion-solidjs has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: motion-solidjs)

Failure details:
- **motion-solidjs@0.6.0** (only, export-kind-unresolved): solid-checker: motion-solidjs has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file /private/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-ZKuAdI/node_modules/motion-solidjs/dist/v1/index.mjs exports "AnimatePresence", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "value" would certify it invokes no... _(stderr truncated for readability)_

**Solid 1.x totals:** 145/168 (86.31%) complete, 9 partial, 14 failed

## Solid 2.x

### Official Solid

- Compatible packages: 12
- Probes run: 15
- Declared entrypoints: 46
- Generated entrypoints: 35
- Refused entrypoints (partial contracts): 0
- Success (complete contracts): 15/15 (100%)
- Partial contracts: 0
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @solidjs/diagnostics | 2.0.0-rc.3 | only | success | success |
| @solidjs/element | 2.0.0-rc.3 | only | success | success |
| @solidjs/h | 2.0.0-rc.3 | only | success | success |
| @solidjs/html | 2.0.0-rc.3 | only | success | success |
| @solidjs/meta | 1.0.0-next.2 | floor | success | success |
| @solidjs/meta | 1.0.0-next.2 | head | success | success |
| @solidjs/router | 2.0.0-next.18 | only | success | success |
| @solidjs/signals | 2.0.0-rc.3 | only | success | success |
| @solidjs/start-devtools | 1.0.0-next.4 | floor | success | success |
| @solidjs/start-devtools | 1.0.0-next.4 | head | success | success |
| @solidjs/universal | 2.0.0-rc.3 | only | success | success |
| @solidjs/vite-plugin | 3.0.0-next.34 | floor | success | success |
| @solidjs/vite-plugin | 3.0.0-next.34 | head | success | success |
| @solidjs/web | 2.0.0-rc.3 | only | success | success |
| solid-js | 2.0.0-rc.3 | only | success | success |

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

- Compatible packages: 97
- Probes run: 194
- Declared entrypoints: 212
- Generated entrypoints: 206
- Refused entrypoints (partial contracts): 0
- Success (complete contracts): 190/194 (97.94%)
- Partial contracts: 0
- Failures: 4

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @solid-primitives/a11y | 1.0.0-next.3 | floor | success | success |
| @solid-primitives/a11y | 1.0.0-next.3 | head | success | success |
| @solid-primitives/active-element | 3.0.0-next.2 | floor | success | success |
| @solid-primitives/active-element | 3.0.0-next.2 | head | success | success |
| @solid-primitives/analytics | 2.0.0-next.2 | floor | success | success |
| @solid-primitives/analytics | 2.0.0-next.2 | head | success | success |
| @solid-primitives/animation | 1.0.0-next.1 | floor | failure | no-esm-runtime-target |
| @solid-primitives/animation | 1.0.0-next.1 | head | failure | no-esm-runtime-target |
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
| @solid-primitives/platform | 1.0.0-next.2 | floor | failure | export-kind-unresolved |
| @solid-primitives/platform | 1.0.0-next.2 | head | failure | export-kind-unresolved |
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
- 2x no-esm-runtime-target: @solid-primitives/animation has no supported ESM runtime entrypoints (packages: @solid-primitives/animation)
- 2x export-kind-unresolved: @solid-primitives/platform has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/platform)

Failure details:
- **@solid-primitives/animation@1.0.0-next.1** (floor, no-esm-runtime-target): solid-checker: @solid-primitives/animation has no supported ESM runtime entrypoints
- **@solid-primitives/animation@1.0.0-next.1** (head, no-esm-runtime-target): solid-checker: @solid-primitives/animation has no supported ESM runtime entrypoints
- **@solid-primitives/platform@1.0.0-next.2** (floor, export-kind-unresolved): solid-checker: @solid-primitives/platform has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file /private/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-H6mN6k/node_modules/@solid-primitives/platform/dist/index.js exports "isBrave", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "value" would certify i... _(stderr truncated for readability)_
- **@solid-primitives/platform@1.0.0-next.2** (head, export-kind-unresolved): solid-checker: @solid-primitives/platform has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file /private/var/folders/y3/kgy_4tp56z717bf03m_v9cc00000gn/T/solid-checker-ecosystem-1Cknrw/node_modules/@solid-primitives/platform/dist/index.js exports "isBrave", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "value" would certify i... _(stderr truncated for readability)_

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
- Probes run: 18
- Declared entrypoints: 58
- Generated entrypoints: 43
- Refused entrypoints (partial contracts): 0
- Success (complete contracts): 17/18 (94.44%)
- Partial contracts: 0
- Failures: 1

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @tanstack/solid-query | 6.0.0-rc.0 | floor | success | success |
| @tanstack/solid-query | 6.0.0-rc.0 | head | success | success |
| @tanstack/solid-query-devtools | 6.0.0-rc.0 | floor | success | success |
| @tanstack/solid-query-devtools | 6.0.0-rc.0 | head | success | success |
| @tanstack/solid-query-persist-client | 6.0.0-rc.0 | floor | success | success |
| @tanstack/solid-query-persist-client | 6.0.0-rc.0 | head | success | success |
| @tanstack/solid-router | 2.0.0-rc.2 | floor | success | success |
| @tanstack/solid-router | 2.0.0-rc.2 | head | success | success |
| @tanstack/solid-router-devtools | 2.0.0-rc.2 | floor | success | success |
| @tanstack/solid-router-devtools | 2.0.0-rc.2 | head | failure | timeout |
| @tanstack/solid-router-ssr-query | 2.0.0-rc.2 | floor | success | success |
| @tanstack/solid-router-ssr-query | 2.0.0-rc.2 | head | success | success |
| @tanstack/solid-start | 2.0.0-rc.2 | floor | success | success |
| @tanstack/solid-start | 2.0.0-rc.2 | head | success | success |
| @tanstack/solid-start-client | 2.0.0-rc.2 | floor | success | success |
| @tanstack/solid-start-client | 2.0.0-rc.2 | head | success | success |
| @tanstack/solid-start-server | 2.0.0-rc.2 | floor | success | success |
| @tanstack/solid-start-server | 2.0.0-rc.2 | head | success | success |

Failure groups:
- 1x timeout: timeout during install (packages: @tanstack/solid-router-devtools)

Failure details:
- **@tanstack/solid-router-devtools@2.0.0-rc.2** (head, timeout): Resolving dependencies

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
- Generated entrypoints: 2
- Refused entrypoints (partial contracts): 4
- Success (complete contracts): 0/2 (0%)
- Partial contracts: 2
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| motion-solidjs | 0.7.0-beta.4 | floor | partial-success | partial-success |
| motion-solidjs | 0.7.0-beta.4 | head | partial-success | partial-success |

**Solid 2.x totals:** 243/250 (97.2%) complete, 2 partial, 5 failed

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

- @tanstack/solid-router-devtools (tanstack): success -> failure

### Better at head than at floor

None.

## Contract content (what the emitted contracts claim)

- Contracts measured: 399 probe(s) across 205 package(s)
- Probes fully proven (no unknown claim, no refused entrypoint, no closure note): 94/399 (23.56%)
- Packages fully proven (every one of their probes): 35/205 (17.07%)
- Probes with at least one unknown claim: 303
- Probes with at least one refused entrypoint: 11
- Probes with at least one closure note: 1
- Exports proven: 3586/7150 (50.15%) (with unknown: 3564, without a summary: 0)
- Of those unknown exports: 622 unknown in ALL five domains (the generator said nothing about them at all), 0 unknown only inside a conditional variant (the default resolution is fully claimed)
- Entrypoints: 706 emitted, 14 refused
- Closure notes (block byte-attested verification): 4
- Attested closure notes (record complete, runtime unbounded): 10

### Unknown claims by domain

| Domain | Exports carrying an unknown |
| --- | --- |
| callbacks | 2419 |
| reactiveReads | 2137 |
| returns | 2190 |
| ownerRequirements | 622 |
| asyncBehavior | 622 |
| **total** | **7990** |

Read the five columns together, not separately: 622 of the 3564 unknown exports are unknown in every domain at once, so most of each column is the same exports counted five times.

### Positive behavioral rows (what a probe step would have to drive)

| Row kind | Count |
| --- | --- |
| callbackExecution | 969 |
| reactiveRead | 1016 |
| returnTree | 763 |
| ownerRequirement | 449 |
| asyncBehavior | 82 |

### Contract content by family

| Family | Contracts | Fully proven | With unknowns | With refusals | Exports proven | Unknown claims |
| --- | --- | --- | --- | --- | --- | --- |
| Official Solid | 21 | 3/21 (14.29%) | 16 | 1 | 664/1093 (60.75%) | 600 |
| Kobalte | 4 | 0/4 (0%) | 4 | 1 | 218/1162 (18.76%) | 3019 |
| Solid Primitives | 280 | 81/280 (28.93%) | 199 | 0 | 1335/1947 (68.57%) | 937 |
| Corvu | 28 | 3/28 (10.71%) | 25 | 0 | 188/266 (70.68%) | 121 |
| TanStack | 52 | 5/52 (9.62%) | 47 | 5 | 1043/1868 (55.84%) | 1606 |
| Solid Devtools | 9 | 2/9 (22.22%) | 7 | 2 | 121/157 (77.07%) | 67 |
| Solid Recharts | 3 | 0/3 (0%) | 3 | 0 | 16/327 (4.89%) | 651 |
| Motion for Solid | 2 | 0/2 (0%) | 2 | 2 | 1/330 (0.3%) | 989 |

### Most unknown claims

| Package | Solid | Unknown claims | Exports with unknown / total | All five domains | Variant-only | Dominant cause |
| --- | --- | --- | --- | --- | --- | --- |
| @kobalte/core@2.0.0-alpha.0 | solid2 | 2060 | 471/526 | 384 | 0 | callbacks |
| @kobalte/core@0.13.13 | solid1 | 940 | 463/611 | 0 | 0 | returns |
| motion-solidjs@0.7.0-beta.4 | solid2 | 825 | 165/165 | 165 | 0 | all-domains |
| @tanstack/solid-router@2.0.0-rc.2 | solid2 | 309 | 118/120 | 0 | 0 | reactiveReads |
| @tanstack/solid-router@2.0.0-rc.2 | solid2 | 309 | 118/120 | 0 | 0 | reactiveReads |
| solid-recharts@1.0.1 | solid1 | 221 | 103/109 | 0 | 0 | reactiveReads |
| solid-recharts@2.0.0-beta.1 | solid2 | 215 | 104/109 | 0 | 0 | reactiveReads |
| solid-recharts@2.0.0-beta.1 | solid2 | 215 | 104/109 | 0 | 0 | reactiveReads |
| motion-solidjs@0.7.0-beta.4 | solid2 | 164 | 164/165 | 0 | 0 | callbacks |
| @tanstack/solid-db@0.2.40 | solid1 | 120 | 120/212 | 0 | 0 | callbacks |
| @tanstack/solid-query@6.0.0-rc.0 | solid2 | 119 | 47/57 | 0 | 0 | reactiveReads |
| @tanstack/solid-query@6.0.0-rc.0 | solid2 | 119 | 47/57 | 0 | 0 | reactiveReads |
| @tanstack/solid-router@1.170.30 | solid1 | 115 | 23/23 | 23 | 0 | all-domains |
| @tanstack/solid-query@5.102.5 | solid1 | 92 | 41/58 | 0 | 0 | returns |
| @solid-primitives/utils@6.4.1 | solid1 | 91 | 43/75 | 0 | 0 | reactiveReads |

These figures describe the GENERATED DRAFT, not consumer findings. An unknown claim becomes a finding only when a consumer actually touches that surface, so a package with many unknowns on exports nobody imports costs a real project nothing. Nothing here has been reviewed or probed: every claim counted as proven is still inferred evidence awaiting review, and a closure note means the contract cannot be byte-attested at all.

## Combined

### Worker timings

- Worker time: 884533 ms
- Phases: install 669226 ms, generation 214042 ms, harness 1265 ms

### Top failure signatures

- 3x export-kind-unresolved: @solid-primitives/platform has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/platform)
- 2x no-esm-runtime-target: @solid-primitives/animation has no supported ESM runtime entrypoints (packages: @solid-primitives/animation)
- 1x no-esm-runtime-target: @kobalte/themes has no supported ESM runtime entrypoints (packages: @kobalte/themes)
- 1x no-esm-runtime-target: @solid-primitives/composites has no supported ESM runtime entrypoints; legacy module target does not exist or is unsupported: ./dist/index.js (packages: @solid-primitives/composites)
- 1x no-exported-surface: @solid-devtools/ext-adapter has no runtime ESM exports (packages: @solid-devtools/ext-adapter)
- 1x cjs-only-entrypoint: . has only a CJS runtime target; CJS contract generation is unsupported (packages: @solid-devtools/babel-plugin)
- 1x export-kind-unresolved: @kobalte/utils has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @kobalte/utils)
- 1x export-kind-unresolved: @solid-devtools/locator has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-devtools/locator)
- 1x export-kind-unresolved: @solid-primitives/analytics has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/analytics)
- 1x export-kind-unresolved: @solid-primitives/audio has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/audio)
- 1x export-kind-unresolved: @solid-primitives/cookies-store has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/cookies-store)
- 1x export-kind-unresolved: @solid-primitives/intersection-observer has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/intersection-observer)
- 1x export-kind-unresolved: @tanstack/solid-hotkeys-devtools has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @tanstack/solid-hotkeys-devtools)
- 1x export-kind-unresolved: motion-solidjs has no certifiable runtime entrypoint; .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: motion-solidjs)
- 1x export-kind-conflict: package contract value export .:createGeolocation cannot have function effects (packages: @solid-primitives/geolocation)
- 1x timeout: timeout during install (packages: @tanstack/solid-router-devtools)

### Partial contracts

- @kobalte/solidbase@0.6.13 (kobalte): 5 entrypoint(s) generated, 1 refused
- @solid-devtools/debugger@0.28.1 (solid-devtools): 4 entrypoint(s) generated, 2 refused
- @tanstack/hotkeys-devtools@0.9.0 (tanstack): 1 entrypoint(s) generated, 1 refused
- @tanstack/solid-pacer@0.22.0 (tanstack): 13 entrypoint(s) generated, 1 refused
- @tanstack/solid-router@1.170.30 (tanstack): 2 entrypoint(s) generated, 1 refused
- @tanstack/solid-table-devtools@9.2.0 (tanstack): 1 entrypoint(s) generated, 1 refused
- @tanstack/table-devtools@9.2.0 (tanstack): 1 entrypoint(s) generated, 1 refused
- motion-solidjs@0.7.0-beta.4 (motion-solidjs): 1 entrypoint(s) generated, 2 refused
- motion-solidjs@0.7.0-beta.4 (motion-solidjs): 1 entrypoint(s) generated, 2 refused
- solid-devtools@0.34.5 (solid-devtools): 2 entrypoint(s) generated, 1 refused
- solid-js@1.9.14 (official-solid): 10 entrypoint(s) generated, 1 refused

### Shared dependency blockers

None.

### Multi-blocker packages

None.

### Family comparison (Solid 1.x vs Solid 2.x)

| Family | Solid 1.x complete/total | Solid 2.x complete/total |
| --- | --- | --- |
| Official Solid | 5/6 (83.33%) | 15/15 (100%) |
| Kobalte | 1/4 (25%) | 2/2 (100%) |
| Solid Primitives | 90/97 (92.78%) | 190/194 (97.94%) |
| Corvu | 11/11 (100%) | 17/17 (100%) |
| TanStack | 30/36 (83.33%) | 17/18 (94.44%) |
| Solid Devtools | 7/12 (58.33%) | 0/0 (no probes run) |
| Solid Recharts | 1/1 (100%) | 2/2 (100%) |
| Motion for Solid | 0/1 (0%) | 0/2 (0%) |

### Discovery limitations

- packument for "@tanstack/tests-adapters" is unavailable (registry returned nothing for it)

### Unavailable metadata

None.

### Baseline comparison

No baseline supplied.
