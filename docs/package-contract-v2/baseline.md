# Migration baseline

Phase 0 is now captured by the machine-readable
[baseline report](../../benchmarks/package-contract-v2/phase0/baseline.json) and
its [human rendering](../../benchmarks/package-contract-v2/phase0/baseline.md).
The generator fails unless every verifier-selected row is classified exactly
once and all raw measurements prove that gate caching was disabled. This page
retains the architectural interpretation and target gates; the benchmark
artifacts are the quantitative authority.

## Source-ownership observation

Observed on 2026-08-27:

| Source | Recorded identity | Current role |
| --- | --- | --- |
| `solidjs/solid#next` | `2f01f23e30d2840139dcbfbed79b270c676a09ad` | Official owner of `packages/compiler`; moving target to refresh at bootstrap |
| `yumemi-thomas/dom-expressions` checker pin | `26e744fb4feb973a3652bfc45a8c3938ece667f0` | Current Solid 2 semantic trace producer, trace version 2 |
| `yumemi-thomas/dom-expressions#next` | `46fe53df6bbe1bbc5fdcf96f35fc4305df09936b` | Port-ledger head observed during design review |
| `yumemi-thomas/solid-ts-facts` | `92c53392388518d69ef27220729f5c061479deed` | Current external producer/client pin and planned import baseline |

This table is the frozen pre-bootstrap observation. Type Facts was subsequently
imported from the recorded revision and is now built locally; the Solid 2
compiler bootstrap likewise has its own later conformance record.

The official compiler crate is now named `solidjs-compiler` and does not carry
the checker's total semantic trace. The next baseline run must record fresh
heads, but all consumed dependencies remain exact revisions. See
[Compiler and Type Facts bootstrap](compiler-and-typefacts-bootstrap.md).

## Current corpus authority

The fresh Phase 0 verification run reports:

| Measure | Count |
| --- | ---: |
| Verified probes | 309 |
| Refused probes | 90 |
| Generation failures | 18 |
| No runtime | 1 |
| Driven claims passing | 7,122 |
| Undriven claims | 3,692 |
| Incompleteness findings | 503 |
| Official Solid verified | 14 / 21 |
| Solid Primitives verified | 236 / 291 |
| Solid Primitives refused | 44 |
| Solid Primitives generation failures | 11 |

Every row has an exact owner and stable reason in the machine report. Primary
owners are 56 schema, 30 probe, 11 resolver, 11 Type Facts, and 1 generator;
309 verified rows have no blocking owner. Compiler-facts, runtime, and
TypeScript each own zero primary rows in this corpus. A richer schema cannot
convert a missing observation or opaque runtime edge into proof.

## Current schema structure

Measured review values:

| Metric | Value |
| --- | ---: |
| Pretty schema bytes | 10,754 |
| Minified schema bytes | 6,197 |
| `$defs` | 6 |
| Named property declarations | 69 |
| Required-name occurrences | 47 |
| `$ref` occurrences | 23 |
| `oneOf` occurrences | 9 |
| `anyOf` occurrences | 4 |
| `allOf` occurrences | 1 |
| Enum declarations | 11 |
| Enum values | 35 |
| Maximum schema-object depth | 13 |

The legacy schema is already structurally significant while lacking operation
graphs, exact artifact cases, proof closure, and recursive leaf-local knowledge.

## Representative legacy contract sizes

UTF-8 bytes measured through the current JavaScript expansion path:

| Contract | Pretty normalized | Minified normalized | Minified expanded | Summaries | Entrypoints | Expanded exports |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Solid 1 debounce | 1,083 | 692 | 883 | 1 | 1 | 2 |
| Solid 1 rootless | 2,958 | 1,917 | 2,489 | 5 | 1 | 8 |
| Solid 1 scheduled | 4,946 | 2,265 | 3,725 | 3 | 1 | 6 |
| Solid 1 `solid-js` | 12,556 | 6,717 | 15,510 | 15 | 9 | 259 |
| Solid 2 `solid-js` RC.0 | 42,983 | 17,368 | 20,461 | 19 | 2 | 81 |
| Solid 2 signals RC.0 | 546 | 415 | 394 | 1 | 1 | 1 |
| Solid 2 web RC.0 | 29,881 | 15,888 | 32,206 | 15 | 13 | 492 |

RC.0 rows are structural size examples only and are not RC.3 semantic authority.

## Known architectural baseline

- Rust and JavaScript independently expand and normalize compact contracts.
- JavaScript performs semantic variant collapse absent from Rust.
- Missing legacy claim fields commonly decode as complete negative knowledge.
- One unconfirmed callback or return leaf can erase an entire domain.
- Artifact identity is package-global and generally excludes module closure.
- Environment selection is flat-condition matching rather than exact package
  export resolution.
- The internal model remains close to the wire schema.
- Inline evidence inflates the main contract while verification sidecars are not
  authoritative during ordinary analysis.
- Current bundled Solid 2 contracts describe RC.0, not the RC.3 authority.

## Reproduction

The complete replay procedure and artifact inventory live beside the baseline
in [the Phase 0 benchmark README](../../benchmarks/package-contract-v2/phase0/README.md).
Run `bun scripts/package-contract-v2-phase0.mjs --check` after any legacy
contract, frozen fixture, ecosystem report, measurement, schema, or pin change.

## Target gates

Accuracy gates dominate coverage:

- zero known false certification;
- every closed claim has a replayable proof;
- every negative uses a complete census;
- probes never supply negative proof;
- exact artifact selection;
- no duplicated TypeScript diagnostic;
- 100% detection of seeded false-closure mutations.

Automation milestones:

- preserve every currently verified row;
- verify at least 85% of installable/generatable rows;
- verify at least 90% of Solid Primitives rows;
- halve schema-blocked refusals;
- report a stable reason for every remaining open domain.

Compactness targets:

- p50 distinct artifact case no more than 8 KiB;
- p95 distinct artifact case no more than 32 KiB;
- p50 package no more than 16 KiB;
- p95 package no more than 128 KiB;
- main document safety ceiling 1 MiB;
- evidence measured separately.
