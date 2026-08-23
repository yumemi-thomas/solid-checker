# Ecosystem Benchmark Report

- Started: 2026-08-22T18:35:02.872Z
- Finished: 2026-08-22T18:36:48.141Z
- Duration: 105269 ms
- Checker native binary: /Users/thomas/Documents/Github/solid-checker/rust/target/debug/solid-checker-rust
- Type Facts binary: /Users/thomas/Documents/Github/solid-checker/bin/solid-typefacts
- Manifest generated at: 2026-08-22T07:44:17.857Z (rows: 305, probes: 416)
- Scope: PARTIAL -- sentinel subset (23 probes run). Not comparable to a full-corpus run.

## Solid 1.x

### Official Solid

- Compatible packages: 3
- Probes run: 3
- Declared entrypoints: 25
- Generated entrypoints: 13
- Refused entrypoints (partial contracts): 0
- Success (complete contracts): 3/3 (100%)
- Partial contracts: 0
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @solidjs/meta | 0.29.4 | only | success | success |
| @solidjs/router | 1.0.0 | only | success | success |
| solid-js | 1.9.14 | only | success | success |

### Kobalte

- Compatible packages: 1
- Probes run: 1
- Declared entrypoints: 3
- Generated entrypoints: 69
- Refused entrypoints (partial contracts): 0
- Success (complete contracts): 1/1 (100%)
- Partial contracts: 0
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @kobalte/core | 0.13.13 | only | success | success |

### Solid Primitives

- Compatible packages: 3
- Probes run: 3
- Declared entrypoints: 3
- Generated entrypoints: 3
- Refused entrypoints (partial contracts): 0
- Success (complete contracts): 3/3 (100%)
- Partial contracts: 0
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @solid-primitives/active-element | 2.1.6 | only | success | success |
| @solid-primitives/analytics | 0.2.1 | only | success | success |
| @solid-primitives/audio | 1.4.5 | only | success | success |

### Corvu

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
| @corvu/accordion | 0.2.5 | only | success | success |

### TanStack

- Compatible packages: 1
- Probes run: 1
- Declared entrypoints: 3
- Generated entrypoints: 2
- Refused entrypoints (partial contracts): 0
- Success (complete contracts): 1/1 (100%)
- Partial contracts: 0
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @tanstack/ai-devtools-core | 0.5.6 | only | success | success |

### Solid Devtools

- Compatible packages: 1
- Probes run: 1
- Declared entrypoints: 0
- Generated entrypoints: 0
- Refused entrypoints (partial contracts): 0
- Success (complete contracts): 0/1 (0%)
- Partial contracts: 0
- Failures: 1

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @solid-devtools/babel-plugin | 0.3.1 | only | failure | cjs-only-entrypoint |

Failure groups:
- 1x cjs-only-entrypoint: . has only a CJS runtime target; CJS contract generation is unsupported (packages: @solid-devtools/babel-plugin)

Failure details:
- **@solid-devtools/babel-plugin@0.3.1** (only, cjs-only-entrypoint): solid-checker: . has only a CJS runtime target; CJS contract generation is unsupported

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

**Solid 1.x totals:** 11/12 (91.67%) complete, 0 partial, 1 failed

## Solid 2.x

### Official Solid

- Compatible packages: 3
- Probes run: 3
- Declared entrypoints: 8
- Generated entrypoints: 6
- Refused entrypoints (partial contracts): 0
- Success (complete contracts): 3/3 (100%)
- Partial contracts: 0
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @solidjs/meta | 1.0.0-next.2 | floor | success | success |
| @solidjs/router | 2.0.0-next.17 | only | success | success |
| solid-js | 2.0.0-rc.1 | floor | success | success |

### Kobalte

- Compatible packages: 1
- Probes run: 1
- Declared entrypoints: 1
- Generated entrypoints: 61
- Refused entrypoints (partial contracts): 0
- Success (complete contracts): 1/1 (100%)
- Partial contracts: 0
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @kobalte/core | 2.0.0-alpha.0 | only | success | success |

### Solid Primitives

- Compatible packages: 3
- Probes run: 3
- Declared entrypoints: 6
- Generated entrypoints: 6
- Refused entrypoints (partial contracts): 0
- Success (complete contracts): 3/3 (100%)
- Partial contracts: 0
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| @solid-primitives/a11y | 1.0.0-next.3 | floor | success | success |
| @solid-primitives/active-element | 3.0.0-next.2 | floor | success | success |
| @solid-primitives/analytics | 2.0.0-next.2 | floor | success | success |

### Corvu

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
| @corvu-next/accordion | 0.1.5 | only | success | success |

### TanStack

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
| @tanstack/solid-query | 6.0.0-rc.0 | floor | success | success |

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
- Probes run: 1
- Declared entrypoints: 1
- Generated entrypoints: 1
- Refused entrypoints (partial contracts): 0
- Success (complete contracts): 1/1 (100%)
- Partial contracts: 0
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| solid-recharts | 2.0.0-beta.1 | floor | success | success |

### Motion for Solid

- Compatible packages: 1
- Probes run: 1
- Declared entrypoints: 3
- Generated entrypoints: 3
- Refused entrypoints (partial contracts): 0
- Success (complete contracts): 1/1 (100%)
- Partial contracts: 0
- Failures: 0

| Package | Version | Probe | Outcome | Class |
| --- | --- | --- | --- | --- |
| motion-solidjs | 0.7.0-beta.4 | floor | success | success |

**Solid 2.x totals:** 11/11 (100%) complete, 0 partial, 0 failed

### Beta-only packages

- @corvu-next/accordion@0.1.5 (corvu)

### RC-only packages

- @kobalte/core@2.0.0-alpha.0 (kobalte)
- @solidjs/router@2.0.0-next.17 (official-solid)

### Worse at head than at floor

None.

### Better at head than at floor

None.

## Contract content (what the emitted contracts claim)

- Contracts measured: 22 probe(s) across 14 package(s)
- Probes fully proven (no unknown claim, no refused entrypoint, no closure note): 7/22 (31.82%)
- Packages fully proven (every one of their probes): 5/14 (35.71%)
- Probes with at least one unknown claim: 15
- Probes with at least one refused entrypoint: 0
- Probes with at least one closure note: 0
- Exports proven: 643/2185 (29.43%) (with unknown: 1542, without a summary: 0)
- Of those unknown exports: 450 unknown in ALL five domains (the generator said nothing about them at all), 0 unknown only inside a conditional variant (the default resolution is fully claimed)
- Entrypoints: 170 emitted, 0 refused
- Closure notes (block byte-attested verification): 0

### Unknown claims by domain

| Domain | Exports carrying an unknown |
| --- | --- |
| callbacks | 465 |
| reactiveReads | 1474 |
| returns | 1531 |
| ownerRequirements | 450 |
| asyncBehavior | 450 |
| **total** | **4370** |

Read the five columns together, not separately: 450 of the 1542 unknown exports are unknown in every domain at once, so most of each column is the same exports counted five times.

### Positive behavioral rows (what a probe step would have to drive)

| Row kind | Count |
| --- | --- |
| callbackExecution | 687 |
| reactiveRead | 250 |
| returnTree | 247 |
| ownerRequirement | 284 |
| asyncBehavior | 0 |

### Contract content by family

| Family | Contracts | Fully proven | With unknowns | With refusals | Exports proven | Unknown claims |
| --- | --- | --- | --- | --- | --- | --- |
| Official Solid | 6 | 0/6 (0%) | 6 | 0 | 239/368 (64.95%) | 214 |
| Kobalte | 2 | 0/2 (0%) | 2 | 0 | 298/1137 (26.21%) | 2263 |
| Solid Primitives | 6 | 4/6 (66.67%) | 2 | 0 | 26/31 (83.87%) | 7 |
| Corvu | 2 | 2/2 (100%) | 0 | 0 | 16/16 (100%) | 0 |
| TanStack | 2 | 1/2 (50%) | 1 | 0 | 21/58 (36.21%) | 77 |
| Solid Devtools | 0 | 0/0 (nothing measured) | 0 | 0 | 0/0 (nothing measured) | 0 |
| Solid Recharts | 2 | 0/2 (0%) | 2 | 0 | 15/218 (6.88%) | 407 |
| Motion for Solid | 2 | 0/2 (0%) | 2 | 0 | 28/357 (7.84%) | 1402 |

### Most unknown claims

| Package | Solid | Unknown claims | Exports with unknown / total | All five domains | Variant-only | Dominant cause |
| --- | --- | --- | --- | --- | --- | --- |
| @kobalte/core@2.0.0-alpha.0 | solid2 | 1387 | 395/526 | 199 | 0 | reactiveReads |
| motion-solidjs@0.7.0-beta.4 | solid2 | 1240 | 248/261 | 248 | 0 | all-domains |
| @kobalte/core@0.13.13 | solid1 | 876 | 444/611 | 0 | 0 | returns |
| solid-recharts@2.0.0-beta.1 | solid2 | 205 | 102/109 | 0 | 0 | reactiveReads |
| solid-recharts@1.0.1 | solid1 | 202 | 101/109 | 0 | 0 | reactiveReads |
| motion-solidjs@0.6.0 | solid1 | 162 | 81/96 | 0 | 0 | reactiveReads |
| @tanstack/solid-query@6.0.0-rc.0 | solid2 | 77 | 37/57 | 0 | 0 | reactiveReads |
| @solidjs/router@1.0.0 | solid1 | 74 | 37/38 | 0 | 0 | reactiveReads |
| solid-js@1.9.14 | solid1 | 61 | 41/202 | 3 | 0 | returns |
| @solidjs/router@2.0.0-next.17 | solid2 | 56 | 28/30 | 0 | 0 | reactiveReads |
| solid-js@2.0.0-rc.1 | solid2 | 15 | 15/81 | 0 | 0 | returns |
| @solidjs/meta@1.0.0-next.2 | solid2 | 7 | 7/8 | 0 | 0 | callbacks |
| @solid-primitives/active-element@2.1.6 | solid1 | 6 | 4/5 | 0 | 0 | callbacks |
| @solid-primitives/audio@1.4.5 | solid1 | 1 | 1/4 | 0 | 0 | callbacks |
| @solidjs/meta@0.29.4 | solid1 | 1 | 1/9 | 0 | 0 | callbacks |

These figures describe the GENERATED DRAFT, not consumer findings. An unknown claim becomes a finding only when a consumer actually touches that surface, so a package with many unknowns on exports nobody imports costs a real project nothing. Nothing here has been reviewed or probed: every claim counted as proven is still inferred evidence awaiting review, and a closure note means the contract cannot be byte-attested at all.

## Combined

### Worker timings

- Worker time: 238313 ms
- Phases: install 25995 ms, generation 212271 ms, harness 47 ms

### Top failure signatures

- 1x cjs-only-entrypoint: . has only a CJS runtime target; CJS contract generation is unsupported (packages: @solid-devtools/babel-plugin)

### Partial contracts

None.

### Shared dependency blockers

None.

### Multi-blocker packages

None.

### Family comparison (Solid 1.x vs Solid 2.x)

| Family | Solid 1.x complete/total | Solid 2.x complete/total |
| --- | --- | --- |
| Official Solid | 3/3 (100%) | 3/3 (100%) |
| Kobalte | 1/1 (100%) | 1/1 (100%) |
| Solid Primitives | 3/3 (100%) | 3/3 (100%) |
| Corvu | 1/1 (100%) | 1/1 (100%) |
| TanStack | 1/1 (100%) | 1/1 (100%) |
| Solid Devtools | 0/1 (0%) | 0/0 (no probes run) |
| Solid Recharts | 1/1 (100%) | 1/1 (100%) |
| Motion for Solid | 1/1 (100%) | 1/1 (100%) |

### Discovery limitations

- packument for "@tanstack/tests-adapters" is unavailable (registry returned nothing for it)

### Unavailable metadata

None.

### Baseline comparison

No baseline supplied.
