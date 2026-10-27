# Differential fuzzer

Finds checker violations that the real Solid 2.0.0-rc.17 runtime contradicts (false positives), automatically. For each random program it compares what the checker says at each site (violation, uncertifiable, none) with what the runtime says (dev diagnostics and thrown errors after driving the program). Only disagreements go to Chrome, the authority. It is operator tooling: it is not part of `make verify`, and the checker and the gates do not read it.

## What a finding means

| verdict | meaning |
| --- | --- |
| `candidate-false-positive` | The checker proves a defect at a site. The runtime ran the site (the page shows its witness), with no diagnostic that claims the rule and no throw. Chrome decides. |
| `candidate-false-clean` | Solid reports a defect (a code the policy table calls a defect) where no checker finding covers it. Grouped by coarse key, confirmed in Chrome, shrunk: see False cleans. |
| `candidate-wrong-rule` | The runtime reports the defect at the finding under a code another rule of the same family claims: the checker named a real defect wrongly. |
| `known-open` | A false clean the ledger (`known-open.json`) accepts, by code and exact coarse key, with a cited reference. Not a failure. |
| `recall-gap` | Solid reports a defect and the checker declines (an uncertifiable finding at the diagnostic). Counted and ranked, never a failure. |
| `inconclusive` | The evidence cannot decide: the site never ran, nothing shows it ran, the rule has no runtime diagnostic, the program threw or halted reactivity, the checker or the runtime failed. Never counted as agreement. |
| `agree` | Both say the same at every site, or both are silent. |

A fast-path candidate is a hypothesis. A **confirmed** finding is a fast-path candidate that Chrome (`scripts/runtime-oracle.mjs`) also classifies `candidate-false-positive`. A candidate Chrome refutes is `refuted-by-chrome`: the fast path misled, which is a fast-path bug to note, not a checker bug. A candidate Chrome cannot decide (an `unattributed` diagnostic the oracle could not place on a line, a page error, a failed execution) is `chrome-inconclusive`: it is neither confirmed nor refuted, and a diagnostic Chrome could not place is never read as silence.

### Stop and report

When a run confirms a false positive (exit status 3), stop the sweep. Do not change the checker from inside a fuzzing session. Report the shrunk repro directory (`repros/<hash>/`, an oracle case: `App.tsx`, `oracle.json`), the rule and finding id, the signature, and the Chrome row. The owner decides whether it is a checker bug, a witness that does not prove what it should, or a runtime behaviour the rule should respect (invariants 1 and 2 of AGENTS.md).

### False cleans (phase D)

A **false clean** is a runtime diagnostic of a defect code with no checker finding that covers it. Which codes are defects is the policy table in `codes.mjs`: every rc.17 dev code has `{ rules, disposition, severity }`, and `codes.test.mjs` greps `code: "X"` across the pinned dev dists of the fast install and fails on a code the table does not classify (so a Solid bump cannot add noise silently) or a severity that differs from the dist. Dispositions: `defect-claimed` (a rule claims it), `defect-unclaimed` (a recall gap by "never quieter than Solid"), `consequence` (the engine's reaction to an earlier failure), `advisory`, `out-of-scope` (SSR, host attribute checks, codes the published typings already reject).

Per runtime diagnostic of a defect code: a checker violation there is `agree`; an uncertifiable finding is a `recall-gap`; nothing that could claim it is a `candidate-false-clean`. A diagnostic no line could be given for is a false clean only when the checker has no claiming finding anywhere in the file.

The pipeline (`falseclean.mjs`, `falseclean-run.mjs`):

1. **Coarse key** = the code, the chain of primitives above the owner call (`root>untrack`, `memo`), and the dynamic stack shape (the roles of the diagnostic's frames: a primitive, a handler, JSX; wrapper functions and calls are left out). Closure layers, helpers and decoys do not change it; the fine signature (`dedupe.mjs`) keeps them apart. The frames are read through `print.mjs`'s `lineTable` (line to node path), not the nearest site by line. On the 179 Chrome-confirmed owner diagnostics of phase C the key gives 55 groups (15 to 18 per code).
2. **Ledger** `known-open.json`: `{ code, coarseKey, ref, reason }`, one exact key each, no wildcards. `ref` is `open-work.md#<anchor>` of a heading that exists, or `ADR nnnn` of a decision that exists; a bad ledger stops the run before it starts. A false clean matching an entry is `known-open`. A run reports the entries that did not fire (`stale-open` when the run covered the profile the entry came from). `--ledger <path>` or `--no-ledger`.
3. **Confirm**: with `--confirm-with-chrome`, one representative per new group (the smallest program) is run in Chrome. Confirmed means Chrome reports the same code at the same innermost App line with no covering finding. A diagnostic Chrome reports but could not place counts too (`confirmed-unplaced`): for a false clean, Solid spoke. Other outcomes: `line-differs`, `covered-in-chrome`, `refuted-by-chrome`, `chrome-inconclusive`.
4. **Shrink** (`--shrink`): the predicate is the same code, the same coarse key and no covering finding; the control is the legal twin, the program with its payload replaced by a literal, which must report no diagnostic of the code. The minimum is run in Chrome again and its status is reported (`minimum confirmed`); the fast path alone can drift.
5. **Exit status 4**: a confirmed false clean with a coarse key the ledger does not know. The sweep still runs to the end (exit 3, a confirmed false positive, stays the stop).

`findings.json` has a `falseClean` section (groups worst severity first, `knownOpen`, `staleOpen`, ranked `recallGaps`); `summary.json` carries the counts.

### The misuse profile and the recall matrix

The fuzzer above hunts a checker that is too loud (false positives) or silent where Solid speaks (false cleans) in programs it draws at random. The **misuse profile** (`--profile misuse`, `misuse.mjs`) asks the other question on purpose: for every code a rule claims (the `defect-claimed` rows of the policy table), does the checker report a violation when the primitive is misused? Each program targets one code at one marked site (a `use` node), so a matrix cell is unambiguous.

* **Context** (the matrix column): component body, memo, effect compute or apply, render-effect compute or apply, `createTrackedEffect`, `onSettled`, `untrack`, `createRoot`, `runWithOwner(null)`, event handler, timer, promise callback, microtask, after an `await`, a JSX child (plain, under `Show`, in a `For` row), an `action` body, an async memo or effect compute. A code's list (`SPEC[code].contexts`) holds only the contexts where the call is a defect and the runtime reports it; `misuse-fire.test.mjs` fires every cell in the fast runtime.
* **Layer** (random per program): none, a named closure called in place, a same-file helper taking the callback, an IIFE, or two of them.
* **Binding** (random per program): the API reached by a direct import, `import * as S`, an alias (`flush as flushAlias`), a destructuring (`const { flush } = S`), or a wrapper function (the wrapper's inner call line belongs to the site, `aux` in `sites.json`).
* **typed-away**: a code whose every misuse the published typings reject has no template (`MISSING_EFFECT_FN`: `createEffect(fn)` is TS2554). It is recorded with the type checker's error, never counted as a gap (invariant 1).

`matrix.mjs` runs `--per-cell N` programs per (code, context) and judges each at its site:

| column | meaning |
| --- | --- |
| typed-away | tsc rejects the program; not evidence |
| harness | the checker or runtime run failed; not evidence |
| not-fired | the program type-checks and the runtime did not report the code at the site: a generator problem |
| agree | runtime-confirmed, and a violation of a rule that claims the code covers the site |
| wrong-rule | runtime-confirmed, and only another rule's violation covers the site |
| recall-gap | runtime-confirmed, and only an uncertifiable finding covers the site |
| false-clean | runtime-confirmed, and no finding covers the site |

Only runtime-confirmed programs (agree + wrong-rule + recall-gap + false-clean) count in the checker columns. `ASYNC_OUTSIDE_LOADING_BOUNDARY` is `placeless`: Solid reports it from the render root, at a line that is not an authored one, so it counts wherever the program reports it.

```sh
node scripts/fuzz/matrix.mjs --out <fresh dir> --seed 1 --per-cell 20 --confirm-with-chrome --shrink --checker-label <commit>
node scripts/fuzz/matrix.mjs ... --baseline <saved matrix.json>         # gate: exit 1 on a regression
node scripts/fuzz/matrix.mjs --gate <baseline.json> <current.json>      # compare two recorded matrices
```

`--confirm-with-chrome` runs, per cell, up to `--chrome-sample` (3) agree programs, up to `--chrome-false-clean` (4) false-clean programs and every wrong-rule program in Chrome and records `same`, `differs`, `not-fired-in-chrome` or `chrome-inconclusive` beside the fast verdict. `--shrink` reduces the smallest Chrome-confirmed false-clean program of each cell (the control is the legal twin) and reconfirms the minimum in Chrome. Output: `matrix.json` (per cell counts, per binding and layer counts, per-seed verdicts, Chrome summary, minimum), `matrix.md`, `programs.jsonl`, `repros/`.

**Gate.** A cell's program seeds depend on the run seed, the code, the context and the index only (`cellSeed`), so a baseline and a later run with the same `--seed` and `--per-cell` judge the same programs. The gate fails (exit 1) when a cell's agree count drops or its false-clean count rises, or a baseline cell is missing; it names the seeds that moved. Different seeds, per-cell counts or generator versions are refused (exit 2). A cell where the checker improved is reported, not failed.

## Layout

| file | role |
| --- | --- |
| `model.mjs` | the construct tree and its validator (a small type checker for the node language) |
| `generate.mjs` | seed -> program: a payload (write or read) in layers (closures, defaults, IIFE, forEach/map, helpers), a host scope, a disposition |
| `print.mjs`, `emit.mjs` | tree -> the clean `App.tsx` and its instrumented twin `App.run.txt`, with the site table, glue (auto-sites) and hit ledger; an oracle case directory (`App.tsx`, `oracle.json` steps, `program.json`, `sites.json`, and the twin's files) |
| `hits.mjs` | the hit ledger: the counter's source, the strip, the readout parser, the per-site record, and the pure comparisons of the neutrality gate |
| `neutrality.mjs` | the regression: a seed range with and without instrumentation, verdict by verdict (`--profiles`, `--chunk`, `--order failing-first`, `--stop-on-mismatch`) |
| `fast-daemon.mjs`, `fast/serve.mjs` | the long-lived fast host: one Vitest process (Vite server, Solid plugin, transform cache) serving every batch; `--no-daemon` gives one process per batch |
| `cache.mjs` | the baseline cache of the regression and its mismatch history |
| `deadsite.mjs` | the `dead-site` conditions as code (off by default, `--dead-site`) |
| `typecheck.mjs` | `tsrs -p ... --noEmit` against the published typings; a program that does not type-check is dropped |
| `checker.mjs` | materializes a case as the oracle does and runs the native checker; per-site verdicts |
| `fast.mjs`, `fast/` | the Happy DOM + `@solidjs/diagnostics` runtime path (Vitest) |
| `classify.mjs`, `codes.mjs` | the comparator; the policy table per rc.17 code and the oracle's code/rule tables (tests fail when they drift) |
| `falseclean.mjs`, `falseclean-run.mjs`, `known-open.json` | the false-clean stream: coarse key, ledger, Chrome confirmation, focused shrink with a control |
| `shrink.mjs`, `dedupe.mjs` | delta debugging over the tree; signatures by construct path |
| `chrome.mjs` | Chrome confirmation through `scripts/runtime-oracle.mjs` |
| `misuse.mjs` | the misuse profile: a template per claimed code, the contexts, layers and binding forms |
| `matrix.mjs` | the recall matrix (code x context), its Chrome sample, the false-clean shrink and the baseline gate |
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

1. Generator, type-check, classifier, shrinker, dedupe, CLI. Unit tests: `packages/cli/node_modules/.bin/vitest run --config packages/cli/vitest.config.mjs scripts/fuzz`.
2. Rediscover this week's false positives on the pre-fix checker (ADR 0452 families), then confirm the fix clears them.
3. Sweeps on the current checker; confirmed findings go to the owner.
4. False cleans (phase D): the policy table and its drift test, the verdict tiers, the ledger, coarse dedupe, Chrome confirmation, focused shrink, exit status 4.
5. The hit ledger (phase A): the twin, ran/landed, glue sites, the neutrality gate. The `dead-site` subkind is written and off.

## How the evidence works

* **Sites.** Every payload read or write is a site. Its line is where its call is written.
* **Witnesses.** A site carries the page text that shows it ran and landed: a write to its own probe signal shows its unique value; a read's value reaches a sink signal or a rendered child. A site without a witness is never evidence of silence. Decoy constructs (a tracked memo read, a write in an apply, a write in a handler) are legal by construction and have witnesses too, so a violation on one is a candidate.
* **One component per page.** A thrown dev error halts reactivity for every root on the page, so a second component would hide or fake the first one's behaviour. The default is one component; `--components` is for experiments.
* **Disturbance.** Any throw, `REACTIVITY_HALTED`, or runner failure makes the other findings of that program inconclusive.
* **Same driver both sides.** The fast runner repeats the oracle's two passes of DOM events on every element, then clicks the plan's buttons and reads the witnesses; the oracle runs the same events and the same plan as scenario steps (`oracle.json`).
* **Signature.** The identity of a finding is its rule plus the construct path from the component to the site (host, closure forms, defaults, helper form, IIFE/forEach/map) and what the program does with each closure on that chain (called, returned, stored, passed to a helper of some form, called from a handler). Names, values and the rest of the file are ignored.

## The hit ledger

Every place the runtime can execute is counted. `print.mjs` emits two texts from one tree: `text` (clean, what the checker reads and what an owner reads in a repro) and `runText` (the twin: type check, Happy DOM, Chrome). The twin has `(__h(N), expr)` on each payload site and each *glue* call (every other emitted call: `m1()()`, sinks, memo accessors, primitives; only the top of a call chain is counted), `__h(N); ` before a statement that is itself a counted call, `{ __h(N);` at every function entry, and the counter's source after the last line, so no line moves. The counter touches no Solid state: it increments `globalThis.__scHits` and mirrors the counts as JSON into a plain-DOM `<output id="sc-hits">` outside `#root` (one pixel, transparent: the oracle's `wait-for-selector` needs it visible). The fast runner reads the global and polls until it is still (quiescence); Chrome reads the last two `#sc-hits` checkpoints, which the twin's `oracle.run.json` adds as steps.

A site is judged by two separate facts: `ran` (its counter is above zero) and `landed` (its witness text is on the page). A payload with a witness needs `landed`; a site with none (a glue call, a void payload) is judged by `ran`; a site that ran and did not land is inconclusive. A finding on a plumbing line (no payload site on its lines) is judged by the glue calls on them, replacing "the finding spans no site of interest". Every inconclusive item carries a `why` (`never-ran`, `ran-not-landed`, `no-witness`, `disturbed`, `perturbed-run`, ...). `results.jsonl` rows carry a `hits` summary per program.

### Neutrality

The counters must not move anything the fuzzer judges. Four checks, none needing Chrome except the last:

1. `strip(runText) == text` (`hits.mjs stripInstrumentation`, checked when the case is built; a failing program is inconclusive `not-neutral`).
2. `--control`: the clean text runs in the same engine and the (code, line) diagnostics, throws, witness texts and failure phase must match; the checker also runs on the twin's text and its (rule, kind, line) findings must equal the clean ones. A difference makes the program inconclusive `perturbed-run`.
3. `node scripts/fuzz/neutrality.mjs --seed 1 --count 300 --profile <p> --control --out <fresh>`: the same seed range with and without instrumentation (`--no-instrument` is the baseline mode of `run.mjs`). Only an `inconclusive` verdict may move, and only to a resolved one; any other change exits 1. It also reports how many findings that used to span no site now map to one. `--baseline <results.jsonl>` reuses a recorded baseline from the same checker; `--baseline a --candidate b` compares two recorded runs.
4. Chrome: the oracle runs the checker on the twin; its findings must equal the fast pipeline's clean ones, else `instrumentation-perturbed-checker` (inconclusive, `chrome-inconclusive`).

Observation: the checker reports nothing on `(f(), m1()())` as a statement in a component body where it reports `m1()()` written plainly (the comma expression hides the read). The counters avoid the form for that reason (statement counters); the gap is the checker's, and is recorded in the report that added the ledger.

### Speed

Nothing here changes a verdict (the acceptance run compares them): the fast path runs in one long-lived Vitest host, whose per-file environment is the isolation (`fast-daemon.test.mjs` has a program leave a global, a document attribute, a prototype property and a halted signal runtime, and the next must find none); `--concurrency` and `--workers` default to half the cores; the neutrality baseline is cached under `--cache-dir` (default `$TMPDIR/solid-checker-fuzz-cache`) keyed by generator version, program, checker and type-facts sha256, the fast install's lockfiles and the hash of this directory's judging sources, so a later round re-runs only the instrumented pass (`--no-cache` to disable); `--order failing-first` puts what mismatched last time first and `--stop-on-mismatch` ends a round at the first mismatching chunk.

### dead-site (off by default)

`--dead-site` enables the `dead-site` false-positive subkind: a violation of an execution-claim rule at a site no execution entered. It needs, all at once: a closed branch-free program (`model.mjs closedGrammar`), a quiescent page, a neutral twin (build strip and clean-twin control), no disturbance and no unplaced diagnostic, a nearest enclosing function never entered while an enclosing function ran, no parameter default, and the same in Chrome. See `deadsite.mjs`; each unmet condition is a typed `inconclusive`. Do not enable it until the neutrality evidence has been reviewed.

## Provenance

The shrinker's search loop (execution budget, seen-set of canonical candidates, validity filter before any run, sweep schedule, control hook, complexity ordering), its halving-width chunk removal, and the idea of weighted generation come from Solid's semantic fuzzer, solidjs/solid PR #3446 "test(signals): add experimental semantic fuzzer" (open, MIT), commit 030e77f2b, `packages/signals/tests/semantics/` (`search.ts`, `shrink.ts`, `complexity.ts`, `candidates.ts`, `generate.ts`). `shrink.mjs` carries the per-file note. That fuzzer interprets a scenario DSL against the signals engine and checks written rules; this one emits Solid source and compares the checker with the runtime, so the generator, the candidate families, the oracle and the dedupe differ (`footprints.ts` dedupes by an engine work ledger; a checker finding has none, so the construct path replaces it). The fast path's runner and shims are ported from the Happy DOM experiment (`happy-patched-runner.tsx`, `happy-dom-fixes.ts`), without its corpus-specific doubles.
