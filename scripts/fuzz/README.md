# Differential fuzzer

Finds checker violations that the real Solid 2.0.0-rc.17 runtime contradicts (false positives), automatically. For each random program it compares what the checker says at each site (violation, uncertifiable, none) with what the runtime says (dev diagnostics and thrown errors after driving the program). Only disagreements go to Chrome, the authority. It is operator tooling: it is not part of `make verify`, and the checker and the gates do not read it.

## What a finding means

| verdict | meaning |
| --- | --- |
| `candidate-false-positive` | The checker proves a defect at a site. The runtime ran the site (the page shows its witness), with no diagnostic that claims the rule and no throw. Chrome decides. |
| `candidate-false-clean` | The runtime reports a defect no finding covers. Classified and reported, not hunted: phase 4. |
| `inconclusive` | The evidence cannot decide: the site never ran, nothing shows it ran, the rule has no runtime diagnostic, the program threw or halted reactivity, the checker or the runtime failed. Never counted as agreement. |
| `agree` | Both say the same at every site, or both are silent. |

A fast-path candidate is a hypothesis. A **confirmed** finding is a fast-path candidate that Chrome (`scripts/runtime-oracle.mjs`) also classifies `candidate-false-positive`. A candidate Chrome refutes is `refuted-by-chrome`: the fast path misled, which is a fast-path bug to note, not a checker bug.

### Stop and report

When a run confirms a false positive (exit status 3), stop the sweep. Do not change the checker from inside a fuzzing session. Report the shrunk repro directory (`repros/<hash>/`, an oracle case: `App.tsx`, `oracle.json`), the rule and finding id, the signature, and the Chrome row. The owner decides whether it is a checker bug, a witness that does not prove what it should, or a runtime behaviour the rule should respect (invariants 1 and 2 of AGENTS.md).

## Layout

| file | role |
| --- | --- |
| `model.mjs` | the construct tree and its validator (a small type checker for the node language) |
| `generate.mjs` | seed -> program: a payload (write or read) in layers (closures, defaults, IIFE, forEach/map, helpers), a host scope, a disposition |
| `print.mjs`, `emit.mjs` | tree -> `App.tsx` with a site table; an oracle case directory (`App.tsx`, `oracle.json` steps, `program.json`, `sites.json`) |
| `typecheck.mjs` | `tsrs -p ... --noEmit` against the published typings; a program that does not type-check is dropped |
| `checker.mjs` | materializes a case as the oracle does and runs the native checker; per-site verdicts |
| `fast.mjs`, `fast/` | the Happy DOM + `@solidjs/diagnostics` runtime path (Vitest) |
| `classify.mjs`, `codes.mjs` | the comparator; the oracle's code/rule tables (a test fails when they drift) |
| `shrink.mjs`, `dedupe.mjs` | delta debugging over the tree; signatures by construct path |
| `chrome.mjs` | Chrome confirmation through `scripts/runtime-oracle.mjs` |
| `run.mjs` | the command line |

## Install

The fast path runs in its own install, `scripts/fuzz/install`: the same pinned Solid packages and overrides as `scripts/runtime-oracle-install` (invariant 7, ADR 0453), plus `vitest`, `happy-dom` and `@solidjs/diagnostics`. A run refuses an install whose Solid packages are not 2.0.0-rc.17.

```sh
cd scripts/fuzz/install && npm install        # first time; commit the resulting package-lock.json
make runtime-oracle-install                   # the checker's, the type check's and Chrome's install
```

The type checker is `tsrs` (`/Users/thomas/Documents/Github/tsrs/target/release/tsrs`, or `--tsc` / `FUZZ_TSC`), never `tsc` from node_modules.

## Running

```sh
export SOLID_CHECKER_NATIVE_BIN=<release solid-checker>   # a release build; debug inflates certification about 19x
export SOLID_TYPEFACTS_BIN=<solid-typefacts>

# Phase 1: generate and type-check only (no checker, no runtime)
node scripts/fuzz/run.mjs --generate-only --seed 1 --count 200 --out /tmp/fuzz-gen-1

# Fast path: checker + Happy DOM runtime, classify
node scripts/fuzz/run.mjs --seed 1 --count 200 --out /tmp/fuzz-run-1

# Confirm candidates in Chrome, shrink what Chrome confirms
node scripts/fuzz/run.mjs --seed 1 --count 200 --out /tmp/fuzz-run-2 --confirm-with-chrome --shrink
```

`--out` must be a fresh directory. It receives `cases/<id>/` (every program as an oracle case), `results.jsonl` (verdict, reasons and items per program), `findings.json` (candidates grouped by signature, smallest repro first), `summary.json`, and `repros/` (shrunk cases). `--shrink` without `--confirm-with-chrome` shrinks fast-path candidates and leaves them `unconfirmed`. `--shrink-mode focused` keeps the exact signature; the default `discovery` keeps any candidate of the same rule. Other options: `--profile`, `--components`, `--workers`, `--concurrency`, `--budget`, `--install`, `--fast-install`, `--browser`.

Any case directory is also runnable by the oracle directly: `node scripts/runtime-oracle.mjs --cases <out>/cases --only <id> --out <fresh.json>`.

## Phases

1. Generator, type-check, classifier, shrinker, dedupe, CLI (this slice). Unit tests: `packages/cli/node_modules/.bin/vitest run --config packages/cli/vitest.config.mjs scripts/fuzz`.
2. Rediscover this week's false positives on the pre-fix checker (ADR 0452 families), then confirm the fix clears them.
3. Sweeps on the current checker; confirmed findings go to the owner.
4. False cleans: a runtime diagnostic no finding covers is already classified (`candidate-false-clean`, provisional); hunting, confirming and shrinking them is not built.

## How the evidence works

* **Sites.** Every payload read or write is a site. Its line is where its call is written.
* **Witnesses.** A site carries the page text that shows it ran and landed: a write to its own probe signal shows its unique value; a read's value reaches a sink signal or a rendered child. A site without a witness is never evidence of silence. Decoy constructs (a tracked memo read, a write in an apply, a write in a handler) are legal by construction and have witnesses too, so a violation on one is a candidate.
* **One component per page.** A thrown dev error halts reactivity for every root on the page, so a second component would hide or fake the first one's behaviour. The default is one component; `--components` is for experiments.
* **Disturbance.** Any throw, `REACTIVITY_HALTED`, or runner failure makes the other findings of that program inconclusive.
* **Same driver both sides.** The fast runner repeats the oracle's two passes of DOM events on every element, then clicks the plan's buttons and reads the witnesses; the oracle runs the same events and the same plan as scenario steps (`oracle.json`).
* **Signature.** The identity of a finding is its rule plus the construct path from the component to the site (host, closure forms, defaults, helper form, IIFE/forEach/map) and what the program does with each closure on that chain (called, returned, stored, passed to a helper of some form, called from a handler). Names, values and the rest of the file are ignored.

## Provenance

The shrinker's search loop (execution budget, seen-set of canonical candidates, validity filter before any run, sweep schedule, control hook, complexity ordering), its halving-width chunk removal, and the idea of weighted generation come from Solid's semantic fuzzer, solidjs/solid PR #3446 "test(signals): add experimental semantic fuzzer" (open, MIT), commit 030e77f2b, `packages/signals/tests/semantics/` (`search.ts`, `shrink.ts`, `complexity.ts`, `candidates.ts`, `generate.ts`). `shrink.mjs` carries the per-file note. That fuzzer interprets a scenario DSL against the signals engine and checks written rules; this one emits Solid source and compares the checker with the runtime, so the generator, the candidate families, the oracle and the dedupe differ (`footprints.ts` dedupes by an engine work ledger; a checker finding has none, so the construct path replaces it). The fast path's runner and shims are ported from the Happy DOM experiment (`happy-patched-runner.tsx`, `happy-dom-fixes.ts`), without its corpus-specific doubles.
