# Phase 0 worktree audit

**Agent:** `current_contract_review`

**Status:** Historical pre-execution audit

**Date:** 2026-08-27

**Repository state:** Dirty worktree at `ec4cb425a87386357d1e8066d625998df3b97874`

**Repository changes made by the audit:** None. This report was added later to
preserve the read-only findings.

This audit predates execution of the reproducible Phase 0 baseline. It records
the evidence and gaps visible in the initial dirty worktree. Later Phase 0
artifacts may supersede its incomplete findings when they bind a then-current
worktree, exact binaries, cache policy, package artifacts, refusal ownership,
size and memory measurements, and frozen fixtures.

## Verdict

The worktree separated cleanly into two changes:

- package-contract design and planning: 7 modified files and 19 new files;
- pre-existing ecosystem benchmark and RC.3 authority work: 16 modified files
  and 2 new files.

The initial tree could not honestly be called "Phase 0 complete." The
implementation plan requires eight baseline deliverables and a reproducible
exit state. Several were absent or only review evidence.

The safe sequence identified by the audit was:

1. preserve or land the ecosystem benchmark change independently;
2. land the package-contract design as a design and roadmap change;
3. run a separate Phase 0 execution change with fresh, cache-disabled baseline
   artifacts.

## Worktree ownership

`git diff --name-only` and
`git ls-files --others --exclude-standard` reported 23 modified and 21
untracked files.

### Package-contract planning change

The following modified files belonged to the package-contract planning change:

- `AGENTS.md`;
- `CONTEXT.md`;
- `docs/README.md`;
- `docs/glossary.md`;
- `docs/monorepo.md`;
- `docs/package-contracts.md`;
- `docs/rfcs/0002-machine-verified-contracts.md`.

The following 19 untracked files belonged to the same planning change:

- all three ADRs under `docs/adr/`;
- all sixteen files then present under `docs/package-contract-v2/`.

The diffs formed one coherent design change: package-contract version policy,
canonical vocabulary, future source ownership, navigation from current docs,
legacy-RFC supersession, ADRs, semantic and wire drafts, an implementation
roadmap, and preserved review reports. The design README explicitly said that
the replacement was approved design rather than current analyzer behavior.

A read-only relative-link check over the 19 new Markdown files reported zero
missing local targets. `git diff --check` exited successfully.

These files could credibly be preserved in a design or roadmap change. A change
titled narrowly as a completed Phase 0 baseline could not use the future
compiler/Type Facts policy or later-phase semantic and wire drafts as proof that
Phase 0 had executed.

### Ecosystem benchmark change

The following files belonged to the separate ecosystem benchmark change:

- `benchmarks/ecosystem/report.json`;
- `benchmarks/ecosystem/report.md`;
- `benchmarks/ecosystem/verification-report.json`;
- `benchmarks/ecosystem/verification-report.md`;
- `docs/ecosystem-benchmark.md`;
- `docs/precision-backlog.md`;
- the ten modified files under `scripts/ecosystem-benchmark/`;
- untracked `scripts/ecosystem-benchmark/framework-scopes.test.mjs`;
- untracked `scripts/ecosystem-benchmark/lib/framework-scopes.mjs`.

That change pinned the audited RC.3 runtime tuple, added exact-version
framework-entrypoint scopes and export-map hashes, refreshed the manifest, and
regenerated both report families. Its generated report and manifest churn was
large and belonged with its code and tests.

Although topically useful to Phase 0, it was not part of the package-contract
design edit. In particular, `docs/precision-backlog.md` could not safely be
cherry-picked alone: its new RC.3 section documented a run produced by the
benchmark scripts, manifest, reports, and tests.

## Phase 0 requirement audit

The governing Phase 0 requirements were:

1. inspect the dirty worktree and preserve unrelated changes;
2. record checker, Type Facts, compiler, and package pins;
3. verify exact `solid-js@2.0.0-rc.3` and related `@solidjs/*` integrity,
   runtime artifacts, declarations, manifests, and export maps;
4. run the legacy contract corpus with caches disabled;
5. save machine-readable and human-readable reports;
6. classify every refusal by schema, Type Facts, compiler facts, generator,
   resolver, probe, runtime, or TypeScript ownership;
7. measure legacy main-contract, expanded, evidence, time, and memory
   baselines;
8. freeze representative false-negative and over-refusal fixtures.

The initial audit found:

| Requirement | Evidence initially present | Initial result |
| --- | --- | --- |
| Dirty-worktree inspection | This audit separated the two complete changes, but no repository baseline artifact recorded that split. | Partial |
| Checker, Type Facts, compiler, and package pins | `baseline.md` recorded compiler and Type Facts revisions. Cargo confirmed `26e744fb4feb973a3652bfc45a8c3938ece667f0` and `92c53392388518d69ef27220729f5c061479deed`; `bin/solid-typefacts.buildinfo` matched the latter. Existing reports hashed both binaries. | Partial |
| Exact RC.3 authority | The Solid evidence review recorded SRI and selected SHA-256 values, but the planning directory contained no checked machine manifest, raw export maps, or replay command. | Review-only |
| Cache-disabled legacy corpus | A 418-row report existed, but neither JSON report recorded `SOLID_CHECKER_GATE_CACHE=0`; the audit found no cache-disable identity in the reports or benchmark documentation. | Not proven |
| Machine and human reports | JSON and Markdown reports existed only in the separate benchmark change and predated the planning worktree. No Phase 0 JSON existed under `docs/package-contract-v2/`. | Partial |
| Stable owner for every refusal | The verifier classified 90 refusals into four legacy blocker roots, but 12 of 18 generation failures were literally `unclassified`. No complete schema/Type Facts/compiler/generator/resolver/probe/runtime/TypeScript mapping existed. | Not met |
| Main, expanded, evidence, time, and memory measurements | Representative main/expanded sizes were reproducible, and reports included generation/probe/verification time. Contract/evidence byte fields, memory fields, and load/query timing were absent. | Not met |
| Frozen representative fixtures | `git status --short -- fixtures` returned no changes, and no Phase 0 fixture manifest was added. | Not met |

The Phase 0 exit condition—one reproducible baseline and a stable reason for
every verified or refused row—was therefore not met at the time of this audit.

## Evidence retained with qualifications

### `baseline.md`

The file explicitly described itself as a review baseline rather than a fresh
clean-run artifact and required replacement or supplementation by a
then-current, cache-disabled run. It was honest design evidence, not Phase 0
completion evidence.

Its recorded source identities had mixed authority:

- the consumed DOM Expressions and Type Facts pins were exact and matched the
  repository;
- `solidjs/solid#next` and the DOM Expressions `next` head were moving or
  design-review observations explicitly marked for refresh.

### Sub-agent reports

The sub-agent report index explicitly characterized those reports as evidence
and counterexample catalogs rather than implementation authority. The RC.3
report's package integrity and file hashes were useful review evidence, but
they were prose observations without a checked machine artifact inventory and
replay command in the Phase 0 directory.

### Legacy contract sizes

The RC.0 Solid 2 rows were structural-size examples only and were not RC.3
semantic authority. A read-only rerun through the current JavaScript expansion
path reproduced the recorded values exactly:

| Contract | Pretty normalized | Minified normalized | Minified expanded | Summaries | Entrypoints | Expanded exports |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Solid 1 debounce | 1,083 | 692 | 883 | 1 | 1 | 2 |
| Solid 1 rootless | 2,958 | 1,917 | 2,489 | 5 | 1 | 8 |
| Solid 1 scheduled | 4,946 | 2,265 | 3,725 | 3 | 1 | 6 |
| Solid 1 `solid-js` | 12,556 | 6,717 | 15,510 | 15 | 9 | 259 |
| Solid 2 `solid-js` RC.0 | 42,983 | 17,368 | 20,461 | 19 | 2 | 81 |
| Solid 2 signals RC.0 | 546 | 415 | 394 | 1 | 1 | 1 |
| Solid 2 web RC.0 | 29,881 | 15,888 | 32,206 | 15 | 13 | 492 |

### Historical 418-row benchmark

The report was a useful historical observation with exact binary hashes and
timestamps, but not the required Phase 0 run. It did not record cache policy,
the current dirty-worktree identity, evidence bytes, memory, or complete
refusal ownership.

The report recorded:

- 309 verified rows;
- 90 refused rows;
- 18 generation failures;
- 1 no-runtime row;
- 7,122 driven and passing claims;
- 3,692 undriven claims;
- 503 incompleteness findings.

Its refusal roots were 56 `kind-observed`, 30 `incompleteness`, three
`attested-closure-note`, and one `closure-note`. Twelve generation failures
remained `unclassified`.

The benchmark binary hashes still matched the files on disk during the audit:

- checker: `b48a808ce65864dfc77cb8dd2a37b77deceed4bfcc4408c42645dc3a7edf7399`;
- Type Facts: `7d8d2ffbc472049660b8b9666da1990edaf86890fd834f45c7e642589abc645b`.

`baseline.md` attributed its 418-row figures to the new
`docs/precision-backlog.md` section. That section existed only in the
uncommitted benchmark diff, so excluding the benchmark change also removed the
baseline's stated provenance from `HEAD`. The audit recommended landing the
benchmark change first or making the later Phase 0 report cite an immutable
artifact and commit directly.

## Commands and material results

```sh
git diff --name-only | wc -l
# 23 modified

git ls-files --others --exclude-standard | wc -l
# 21 untracked files

git status --short -- schema packages/cli rust fixtures pkg/contracts
# no output: no schema, implementation, bundle, or fixture work was present

git diff --check
# exit 0
```

```sh
jq '.overall.outcomes, .overall.rootCauses, .preContractFailures.generateFailures' \
  benchmarks/ecosystem/verification-report.json
```

Material result: 309 verified, 90 refused, 18 generation failures, one
no-runtime row, and 12 generation failures classified as `unclassified`.

```sh
shasum -a 256 rust/target/release/solid-checker-rust bin/solid-typefacts
```

Material result: both hashes matched the historical verification report.

```sh
rg -n 'SOLID_CHECKER_GATE_CACHE|gateCache|cacheDisabled' \
  benchmarks/ecosystem/report.json \
  benchmarks/ecosystem/verification-report.json \
  scripts/ecosystem-benchmark docs/ecosystem-benchmark.md
# no matches
```

```sh
find docs/package-contract-v2 -type f -name '*.json' | wc -l
# 0

git status --short -- fixtures | wc -l
# 0
```

These findings describe only the initial pre-execution state. A later Phase 0
authority may close any of the gaps above, but should preserve this report as
the record of what had not yet been established.
