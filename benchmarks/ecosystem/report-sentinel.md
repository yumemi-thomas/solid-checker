# Ecosystem Benchmark Report

- Started: 2026-08-28T08:18:06.316Z
- Finished: 2026-08-28T08:23:15.138Z
- Duration: 308822 ms
- Checker native binary: /Users/thomas/Documents/Github/solid-checker/rust/target/debug/solid-checker-rust
- Type Facts binary: /Users/thomas/Documents/Github/solid-checker/bin/solid-typefacts
- Manifest generated at: 2026-08-26T14:21:49.573Z (rows: 307, probes: 418)
- Scope: PARTIAL -- sentinel subset (23 probes run). Not comparable to a full-corpus run.

## Solid 1.x

### Official Solid

- Compatible packages: 3
- Probes run: 3
- Declared entrypoints: 25
- Generated entrypoints: 3
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 389
- Success (complete contracts): 2/3 (66.67%)
- Partial contracts: 1
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @solidjs/meta | 0.29.4 | only | success | success |
| @solidjs/router | 1.0.0 | only | success | success |
| solid-js | 1.9.14 | only | partial-success | partial-success |

### Kobalte

- Compatible packages: 1
- Probes run: 1
- Declared entrypoints: 3
- Generated entrypoints: 1
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 2
- Success (complete contracts): 0/1 (0%)
- Partial contracts: 1
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @kobalte/core | 0.13.13 | only | partial-success | partial-success |

### Solid Primitives

- Compatible packages: 3
- Probes run: 3
- Declared entrypoints: 3
- Generated entrypoints: 1
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 1
- Success (complete contracts): 0/3 (0%)
- Partial contracts: 1
- Failures: 2

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @solid-primitives/active-element | 2.1.6 | only | partial-success | partial-success |
| @solid-primitives/analytics | 0.2.1 | only | failure | export-kind-unresolved |
| @solid-primitives/audio | 1.4.5 | only | failure | export-kind-unresolved |

Failure groups:
- 1x export-kind-unresolved: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/analytics)
- 1x export-kind-unresolved: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/audio)

Failure details:
- **@solid-primitives/analytics@0.2.1** (only, export-kind-unresolved): solid-checker: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file /private<package-root>/dist/index.js exports "EventType", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "value" would certify it invokes no caller-supplied callback
- **@solid-primitives/audio@1.4.5** (only, export-kind-unresolved): solid-checker: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file /private<package-root>/dist/index.js exports "AudioState", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "value" would certify it invokes no caller-supplied callback

### Corvu

- Compatible packages: 1
- Probes run: 1
- Declared entrypoints: 1
- Generated entrypoints: 1
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 0
- Success (complete contracts): 1/1 (100%)
- Partial contracts: 0
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @corvu/accordion | 0.2.5 | only | success | success |

### TanStack

- Compatible packages: 1
- Probes run: 1
- Declared entrypoints: 3
- Generated entrypoints: 0
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 0
- Success (complete contracts): 0/1 (0%)
- Partial contracts: 0
- Failures: 1

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @tanstack/ai-devtools-core | 0.5.8 | only | failure | timeout |

Failure groups:
- 1x timeout: timeout during generate (packages: @tanstack/ai-devtools-core)

Failure details:
- **@tanstack/ai-devtools-core@0.5.8** (only, timeout): (no stderr captured)

### Solid Devtools

- Compatible packages: 1
- Probes run: 1
- Declared entrypoints: 0
- Generated entrypoints: 0
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 0
- Success (complete contracts): 0/1 (0%)
- Partial contracts: 0
- Failures: 1

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @solid-devtools/babel-plugin | 0.3.1 | only | failure | no-exported-surface |

Failure groups:
- 1x no-exported-surface: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file has no runtime ESM exports (packages: @solid-devtools/babel-plugin)

Failure details:
- **@solid-devtools/babel-plugin@0.3.1** (only, no-exported-surface): solid-checker: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file /private<package-root>/dist/index.js has no runtime ESM exports

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

**Solid 1.x totals:** 3/12 (25%) complete, 4 partial, 5 failed

## Solid 2.x

### Official Solid

- Compatible packages: 3
- Probes run: 3
- Declared entrypoints: 8
- Generated entrypoints: 3
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 97
- Success (complete contracts): 1/3 (33.33%)
- Partial contracts: 2
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @solidjs/meta | 1.0.0-next.2 | floor | success | success |
| @solidjs/router | 2.0.0-next.18 | only | partial-success | partial-success |
| solid-js | 2.0.0-rc.3 | only | partial-success | partial-success |

### Kobalte

- Compatible packages: 1
- Probes run: 1
- Declared entrypoints: 1
- Generated entrypoints: 0
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 0
- Success (complete contracts): 0/1 (0%)
- Partial contracts: 0
- Failures: 1

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @kobalte/core | 2.0.0-alpha.0 | only | failure | unclassified |

Failure groups:
- 1x unclassified: package exports ./*; pass each finite --entrypoint explicitly so generation does not guess the public surface (packages: @kobalte/core)

Failure details:
- **@kobalte/core@2.0.0-alpha.0** (only, unclassified): solid-checker: package exports ./*; pass each finite --entrypoint explicitly so generation does not guess the public surface

### Solid Primitives

- Compatible packages: 3
- Probes run: 3
- Declared entrypoints: 6
- Generated entrypoints: 3
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 9
- Success (complete contracts): 0/3 (0%)
- Partial contracts: 3
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @solid-primitives/a11y | 1.0.0-next.3 | floor | partial-success | partial-success |
| @solid-primitives/active-element | 3.0.0-next.2 | floor | partial-success | partial-success |
| @solid-primitives/analytics | 2.0.0-next.2 | floor | partial-success | partial-success |

### Corvu

- Compatible packages: 1
- Probes run: 1
- Declared entrypoints: 1
- Generated entrypoints: 1
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 0
- Success (complete contracts): 1/1 (100%)
- Partial contracts: 0
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @corvu-next/accordion | 0.1.5 | only | success | success |

### TanStack

- Compatible packages: 1
- Probes run: 1
- Declared entrypoints: 1
- Generated entrypoints: 0
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 0
- Success (complete contracts): 0/1 (0%)
- Partial contracts: 0
- Failures: 1

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @tanstack/solid-query | 6.0.0-rc.0 | floor | failure | dependency-contract-obligation |

Failure groups:
- 1x dependency-contract-obligation: no certifiable artifact case; 4 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/query-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-query)

Failure details:
- **@tanstack/solid-query@6.0.0-rc.0** (floor, dependency-contract-obligation): solid-checker: no certifiable artifact case; 4 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/query-core solid-checker-rust: emit package contract: cannot statically expand external export-all "@tanstack/query-core" from /private<package-root>/build/index.js; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-... _(stderr truncated for readability)_

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
- Probes run: 1
- Declared entrypoints: 1
- Generated entrypoints: 0
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 0
- Success (complete contracts): 0/1 (0%)
- Partial contracts: 0
- Failures: 1

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| solid-recharts | 2.0.0-beta.1 | floor | failure | unclassified |

Failure groups:
- 1x unclassified: no certifiable artifact case; 1 case(s) refused; first refusal: .: local closure module ./animation/easing from <package-root>/src/index.ts was not found (packages: solid-recharts)

Failure details:
- **solid-recharts@2.0.0-beta.1** (floor, unclassified): solid-checker: no certifiable artifact case; 1 case(s) refused; first refusal: .: local closure module ./animation/easing from <package-root>/src/index.ts was not found

### Motion for Solid

- Compatible packages: 1
- Probes run: 1
- Declared entrypoints: 3
- Generated entrypoints: 0
- Refused entrypoints (partial contracts): 0
- Refused artifact cases (partial contracts): 0
- Success (complete contracts): 0/1 (0%)
- Partial contracts: 0
- Failures: 1

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| motion-solidjs | 0.7.0-beta.4 | floor | failure | export-kind-unresolved |

Failure groups:
- 1x export-kind-unresolved: no certifiable artifact case; 3 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: motion-solidjs)

Failure details:
- **motion-solidjs@0.7.0-beta.4** (floor, export-kind-unresolved): solid-checker: no certifiable artifact case; 3 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file /private<package-root>/dist/v2/index.mjs exports "AnimatePresence", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "value" would certify it invokes no caller-supplied callback

**Solid 2.x totals:** 2/11 (18.18%) complete, 5 partial, 4 failed

### Beta-only packages

- @corvu-next/accordion@0.1.5 (corvu)

### RC-only packages

- @kobalte/core@2.0.0-alpha.0 (kobalte)
- @solidjs/router@2.0.0-next.18 (official-solid)
- solid-js@2.0.0-rc.3 (official-solid)

### Worse at head than at floor

None.

### Better at head than at floor

None.

## Contract content (what the emitted contracts claim)

- Contracts measured: 14 probe(s) across 10 package(s)
- Probes fully proven (no unknown claim, no refused entrypoint, no closure note): 0/14 (0%)
- Packages fully proven (every one of their probes): 0/10 (0%)
- Probes with at least one unknown claim: 14
- Probes with at least one refused entrypoint: 9
- Probes with at least one closure note: 0
- Exports proven: 0/1198 (0%) (with unknown: 1198, without a summary: 0)
- Of those unknown exports: 6 unknown in every measured domain (the generator said nothing about them at all), 0 unknown only inside a conditional variant (the default resolution is fully claimed)
- Entrypoints: 14 emitted, 0 refused; 499 artifact cases refused
- Closure notes (block byte-attested verification): 0
- Attested closure notes (record complete, runtime unbounded): 0

### Proposal wire size

| Artifact | Samples | p50 bytes | p95 bytes | max bytes |
| --- | ---: | ---: | ---: | ---: |
| Pretty main | 14 | 3533 | 47265 | 47265 |
| Canonical minified main | 14 | 2757 | 31721 | 31721 |
| Proposal plan (not evidence) | 14 | 130222 | 2718647 | 2718647 |
| Canonical bytes per export | 14 | 172.31 | 421 | 421 |
| Canonical bytes per operation | 3 | 954 | 3349 | 3349 |

Proposal-plan bytes are construction obligations, not proof evidence and not acceptance authority. Proof-transcript and receipt bytes are measured separately by the Phase 16 accepted-corpus gate.

### Unknown claims by domain

| Domain | Exports carrying an unknown |
| --- | --- |
| callbacks | 1198 |
| reads | 1198 |
| writes | 1198 |
| creates | 1198 |
| invalidates | 1198 |
| throws | 1198 |
| returns | 1198 |
| cleanups | 1198 |
| disposals | 1198 |
| recursiveValue | 6 |
| **total** | **10788** |

Read the domain columns together, not separately: 6 of the 1198 unknown exports are unknown in every measured domain at once, so the same export can contribute to several columns.

### Positive behavioral rows (what a probe step would have to drive)

| Row kind | Count |
| --- | --- |
| invoke | 57 |
| return | 33 |
| read | 47 |
| write | 0 |
| invalidate | 0 |
| create | 7 |
| cleanup | 0 |
| dispose | 0 |

### Contract content by family

| Family | Contracts | Fully proven | With unknowns | With refusals | Exports proven | Unknown claims |
| --- | --- | --- | --- | --- | --- | --- |
| Official Solid | 6 | 0/6 (0%) | 6 | 3 | 0/623 (0%) | 5613 |
| Kobalte | 1 | 0/1 (0%) | 1 | 1 | 0/194 (0%) | 1746 |
| Solid Primitives | 4 | 0/4 (0%) | 4 | 4 | 0/22 (0%) | 198 |
| Corvu | 2 | 0/2 (0%) | 2 | 0 | 0/32 (0%) | 288 |
| TanStack | 0 | 0/0 (nothing measured) | 0 | 0 | 0/0 (nothing measured) | 0 |
| Solid Devtools | 0 | 0/0 (nothing measured) | 0 | 0 | 0/0 (nothing measured) | 0 |
| Solid Recharts | 1 | 0/1 (0%) | 1 | 1 | 0/327 (0%) | 2943 |
| Motion for Solid | 0 | 0/0 (nothing measured) | 0 | 0 | 0/0 (nothing measured) | 0 |

### Most unknown claims

| Package | Solid | Unknown claims | Exports with unknown / total | All five domains | Variant-only | Dominant cause |
| --- | --- | --- | --- | --- | --- | --- |
| solid-recharts@1.0.1 | solid1 | 2943 | 327/327 | 0 | 0 | callbacks |
| solid-js@2.0.0-rc.3 | solid2 | 2736 | 304/304 | 0 | 0 | callbacks |
| @kobalte/core@0.13.13 | solid1 | 1746 | 194/194 | 0 | 0 | callbacks |
| solid-js@1.9.14 | solid1 | 1455 | 161/161 | 6 | 0 | callbacks |
| @solidjs/router@1.0.0 | solid1 | 684 | 76/76 | 0 | 0 | callbacks |
| @solidjs/router@2.0.0-next.18 | solid2 | 504 | 56/56 | 0 | 0 | callbacks |
| @solidjs/meta@0.29.4 | solid1 | 162 | 18/18 | 0 | 0 | callbacks |
| @corvu-next/accordion@0.1.5 | solid2 | 144 | 16/16 | 0 | 0 | callbacks |
| @corvu/accordion@0.2.5 | solid1 | 144 | 16/16 | 0 | 0 | callbacks |
| @solidjs/meta@1.0.0-next.2 | solid2 | 72 | 8/8 | 0 | 0 | callbacks |
| @solid-primitives/a11y@1.0.0-next.3 | solid2 | 63 | 7/7 | 0 | 0 | callbacks |
| @solid-primitives/analytics@2.0.0-next.2 | solid2 | 63 | 7/7 | 0 | 0 | callbacks |
| @solid-primitives/active-element@2.1.6 | solid1 | 45 | 5/5 | 0 | 0 | callbacks |
| @solid-primitives/active-element@3.0.0-next.2 | solid2 | 27 | 3/3 | 0 | 0 | callbacks |

These figures describe the GENERATED DRAFT, not consumer findings. An unknown claim becomes a finding only when a consumer actually touches that surface, so a package with many unknowns on exports nobody imports costs a real project nothing. Nothing here has been reviewed or probed: every claim counted as proven is still inferred evidence awaiting review, and a closure note means the contract cannot be byte-attested at all.

## Combined

### Worker timings

- Worker time: 1223423 ms
- Phases: install 8903 ms, generation 1214467 ms, harness 53 ms

### Top failure signatures

- 2x export-kind-unresolved: no certifiable artifact case; 2 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/audio, motion-solidjs)
- 1x no-exported-surface: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file has no runtime ESM exports (packages: @solid-devtools/babel-plugin)
- 1x dependency-contract-obligation: no certifiable artifact case; 4 case(s) refused; first refusal: .: solid-checker:unresolved-dependency-module=@tanstack/query-core solid-checker-rust: emit package contract: cannot statically expand external export-all "<value>" from ; acquire a verified dependency contract and pass its receipt-issued exact import through --accepted-contracts (packages: @tanstack/solid-query)
- 1x export-kind-unresolved: no certifiable artifact case; 1 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: @solid-primitives/analytics)
- 1x export-kind-unresolved: no certifiable artifact case; 3 case(s) refused; first refusal: .: solid-checker-rust: emit package contract: entry file exports "<value>", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "<value>" would certify it invokes no caller-supplied callback (packages: motion-solidjs)
- 1x timeout: timeout during generate (packages: @tanstack/ai-devtools-core)
- 1x unclassified: no certifiable artifact case; 1 case(s) refused; first refusal: .: local closure module ./animation/easing from <package-root>/src/index.ts was not found (packages: solid-recharts)
- 1x unclassified: package exports ./*; pass each finite --entrypoint explicitly so generation does not guess the public surface (packages: @kobalte/core)

### Partial contracts

- @kobalte/core@0.13.13 (kobalte): 1 entrypoint(s) generated, 0 entrypoint(s) and 2 artifact case(s) refused
- @solid-primitives/a11y@1.0.0-next.3 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/active-element@2.1.6 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/active-element@3.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused
- @solid-primitives/analytics@2.0.0-next.2 (solid-primitives): 1 entrypoint(s) generated, 0 entrypoint(s) and 7 artifact case(s) refused
- @solidjs/router@2.0.0-next.18 (official-solid): 1 entrypoint(s) generated, 0 entrypoint(s) and 4 artifact case(s) refused
- solid-js@1.9.14 (official-solid): 1 entrypoint(s) generated, 0 entrypoint(s) and 389 artifact case(s) refused
- solid-js@2.0.0-rc.3 (official-solid): 1 entrypoint(s) generated, 0 entrypoint(s) and 93 artifact case(s) refused
- solid-recharts@1.0.1 (solid-recharts): 1 entrypoint(s) generated, 0 entrypoint(s) and 1 artifact case(s) refused

### Shared dependency blockers

- @tanstack/query-core: estimated 1 package(s) unlocked (@tanstack/solid-query)

### Multi-blocker packages

None.

### Family comparison (Solid 1.x vs Solid 2.x)

| Family | Solid 1.x complete/total | Solid 2.x complete/total |
| --- | --- | --- |
| Official Solid | 2/3 (66.67%) | 1/3 (33.33%) |
| Kobalte | 0/1 (0%) | 0/1 (0%) |
| Solid Primitives | 0/3 (0%) | 0/3 (0%) |
| Corvu | 1/1 (100%) | 1/1 (100%) |
| TanStack | 0/1 (0%) | 0/1 (0%) |
| Solid Devtools | 0/1 (0%) | 0/0 (no probes run) |
| Solid Recharts | 0/1 (0%) | 0/1 (0%) |
| Motion for Solid | 0/1 (0%) | 0/1 (0%) |

### Discovery limitations

- packument for "@tanstack/tests-adapters" is unavailable (registry returned nothing for it)

### Unavailable metadata

- 14 contract-producing probe(s) missing checklistItems

### Baseline comparison

No baseline supplied.
