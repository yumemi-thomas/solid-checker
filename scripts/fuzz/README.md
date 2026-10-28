# Differential fuzzer

Finds checker violations that the real Solid 2.0.0-rc.17 runtime contradicts (false positives) and defects Solid reports where the checker is silent (false cleans), automatically. For each generated program it compares what the checker says at each site (violation, uncertifiable, none) with what the runtime says (dev diagnostics and thrown errors after driving the program). Only disagreements go to Chrome, the authority. It is operator tooling: it is not part of `make verify`, and the checker and the gates do not read it.

## What a finding means

| verdict | meaning |
| --- | --- |
| `candidate-false-positive` | The checker proves a defect at a site. The runtime ran the site (the page shows its witness), with no diagnostic that claims the rule and no throw. Chrome decides. |
| `candidate-false-clean` | Solid reports a defect (a code the policy table calls a defect) where no checker finding covers it. Grouped by coarse key, confirmed in Chrome, shrunk: see False cleans. |
| `candidate-wrong-rule` | The runtime reports the defect at the finding under a code another rule of the same family claims: the checker named a real defect wrongly. |
| `known-open` | A false clean the ledger (`known-open.json`) accepts, by code and exact coarse key, with a cited reference. Not a failure. |
| `recall-gap` | Solid reports a defect and the checker declines (an uncertifiable finding at the diagnostic). Counted and ranked, never a failure. |
| `candidate-unexecuted` | Shown as `candidate-false-positive: unexecuted`. The checker proves a defect at a payload site the hit ledger shows never ran (see Unexecuted violations). A candidate, never a failure by itself: Chrome confirms it as `confirmed-unexecuted` (exit 3) or not. |
| `inconclusive` | The evidence cannot decide: the site never ran, nothing shows it ran, the rule has no runtime diagnostic, the program threw or halted reactivity, the checker or the runtime failed. Never counted as agreement. |
| `agree` | Both say the same at every site, or both are silent. |

A fast-path candidate is a hypothesis. A **confirmed** finding is a fast-path candidate that Chrome (`scripts/runtime-oracle.mjs`) also classifies `candidate-false-positive`. A candidate Chrome refutes is `refuted-by-chrome`: the fast path misled, which is a fast-path bug to note, not a checker bug. A candidate Chrome cannot decide (an `unattributed` diagnostic the oracle could not place on a line, a page error, a failed execution) is `chrome-inconclusive`: it is neither confirmed nor refuted, and a diagnostic Chrome could not place is never read as silence.

### Stop and report

When a run confirms a false positive (exit status 3), stop the sweep. Do not change the checker from inside a fuzzing session. Report the shrunk repro directory (`repros/<hash>/`, an oracle case: `App.tsx`, `oracle.json`), the rule and finding id, the signature, and the Chrome row. The owner decides whether it is a checker bug, a witness that does not prove what it should, or a runtime behaviour the rule should respect (invariants 1 and 2 of AGENTS.md).

## Profiles

`--profile <name>` picks what the generator draws. Without it a program takes one of the seven base profiles at random. The others are asked for by name and never drawn at random, so adding one does not move the seeds of any other: with no new flag, every profile prints the same programs for the same seed as before it existed.

| profile | what it generates |
| --- | --- |
| `closure-escape`, `default-param`, `helper-closure`, `in-place`, `async`, `mixed`, `body-call` | the base mix: a payload (a write or a read) in layers (closures, defaults, IIFE, `forEach`/`map`, helpers), a host scope, a disposition; legal decoys beside it |
| `owner-cleanup`, `owner-effect`, `owner-settled`, `owner-context`, `owner` | an owner-requiring call (`onCleanup`, `createEffect`, `onSettled`, `useContext`) in every host, owned or not (NO_OWNER_*, SETTLED_CLEANUP_UNOWNED, CLEANUP_IN_FORBIDDEN_SCOPE) |
| `disposal` | a write that runs while an owner is being disposed, behind a cleanup handle called now, later or never (see The disposal profile) |
| `misuse` | one program per claimed diagnostic code, the misuse at one marked site, in a chosen context, layer and binding (see The misuse profile) |
| `packages` | the helper layers are exports of a generated dependency package, `fz-lib` (see The packages profile) |
| `containers` | any base profile (mostly `disposal`) with statements wrapped in containers that run elsewhere, later or never (see Containers); `--containers` does the same on top of any profile |

## Install

The fast path runs in its own install, `scripts/fuzz/install`: the same pinned Solid packages and overrides as `scripts/runtime-oracle-install` (invariant 7, ADR 0453), plus `vitest`, `happy-dom` and `@solidjs/diagnostics`. A run refuses an install whose Solid packages are not 2.0.0-rc.17.

```sh
cd scripts/fuzz/install && npm install        # first time; commit the resulting package-lock.json
make runtime-oracle-install                   # the checker's, the type check's and Chrome's install
```

The type checker is `tsrs`, never `tsc` from node_modules. It is found from `--tsc <path>`, then `SOLID_CHECKER_TSRS_BIN` (`FUZZ_TSC` still works), then a release build in a `tsrs` checkout beside this repository, then beside the main working tree this worktree was made from. A run whose type checker is missing stops with exit 2 before it writes anything (every program would otherwise be a `type-error` and the run would pass with 0 candidates). So does a run in which more than half the programs fail to type-check, or all of them do (`TYPE_ERROR_REFUSAL_SHARE` in `typecheck.mjs`; a healthy sweep sees a few percent). Every entry point checks this: `run.mjs`, `matrix.mjs`, `equivalence.mjs`, `neutrality.mjs`, `disposal-check.mjs`, `packages-check.mjs`. The checker's findings are mapped to the program by realpath on both sides (the checker reports real paths; a work directory reached through a symlink, `/tmp` on macOS, otherwise dropped every finding and the run read as clean); a run in which the checker returned findings and none is under the program's directory stops with exit 2 (`assertFindingsMapped`).

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

### Flags

| flag | meaning |
| --- | --- |
| `--out <dir>` | fresh output directory (required) |
| `--seed N`, `--count N` | the seed of the run (program seeds are derived from it and the index) and the number of programs |
| `--profile <name>`, `--containers`, `--components N` | the generator (Profiles above); more than one component per page is for experiments |
| `--code`, `--context`, `--binding`, `--layer`, `--variant` | with `--profile misuse`: pin the template's code, context, binding, layer or variant |
| `--checker-bin`, `--typefacts`, `--install`, `--fast-install`, `--tsc`, `--browser` | the tools and installs (or `SOLID_CHECKER_NATIVE_BIN`, `SOLID_TYPEFACTS_BIN`, `FUZZ_TSC`) |
| `--concurrency N`, `--workers N`, `--all-cores` | checker/type-check parallelism and Vitest workers; default half the cores, `--all-cores` takes all |
| `--confirm-with-chrome` | Chrome decides each candidate (and one representative per new false-clean group) |
| `--shrink`, `--shrink-mode discovery\|focused`, `--budget N`, `--shrink-jobs N`, `--shrink-batch N` | reduce one representative per signature |
| `--control` | neutrality control on the clean twin (always on for `--profile packages`) |
| `--ledger <file>`, `--no-ledger` | the accepted false cleans (default `known-open.json`, currently empty) |
| `--real-clock` | the fast path waits on the wall clock, as before the virtual clock (see Speed); the default is the virtual clock |
| `--shrink-control accepted\|each` | with `--control`, the neutrality control runs on each accepted reduction (default) or on every candidate a shrink tries |
| `--no-instrument`, `--no-daemon`, `--no-cache`, `--keep-work`, `--generate-only`, `--dead-site` | the baseline text with no ledger, one Vitest process per batch, no neutrality cache, keep the checker directories, type-check only, the strict confirmation of unexecuted candidates (see Unexecuted violations) |

Exit status: 0 clean; 3 Chrome confirmed a false positive or a `confirmed-unexecuted` violation (stop and report); 4 Chrome confirmed a false clean the ledger does not know (the sweep ran to the end); 2 a usage or setup error.

### Merge gate

To gate a checker change against false positives only, run fast and wide, not deep:

```sh
for seed in 11 12 13 14; do   # one process per seed; each its own --out and its own seed range
  node scripts/fuzz/run.mjs --out <fresh dir>-$seed --seed $seed --count 200 --profile closure-escape --concurrency 3 &
done; wait
```

Rules: no `--shrink` (a reduction is for a person, after a confirmed finding); distinct `--out` per process (and distinct `--seed` unless you mean to compare: program ids come from the seed and the index alone, so two profiles on one seed share ids, which `compare-runs.mjs` relies on; the checker work directories are per process, `.solid-checker-fuzz/run-<pid>-<tag>/`, removed at exit unless `--keep-work`); keep `--concurrency` times the number of processes within the cores; use the release checker. A false positive needs a fast-path candidate first, so a run whose `summary.json` has `candidates: 0` passes without Chrome. When candidates exist, rerun that seed with `--confirm-with-chrome` and gate on exit 3. Exit 4 and the false-clean and recall-gap counts are recall, not false positives: report them, do not gate on them. Repeat the profile list that matches the change (`owner-cleanup`, `disposal`, `packages`, `containers`), and run `misuse` through the matrix gate below.

## False cleans (phase D)

A **false clean** is a runtime diagnostic of a defect code with no checker finding that covers it. Which codes are defects is the policy table in `codes.mjs`: every rc.17 dev code has `{ rules, disposition, severity }`, and `codes.test.mjs` greps `code: "X"` across the pinned dev dists of the fast install and fails on a code the table does not classify (so a Solid bump cannot add noise silently) or a severity that differs from the dist. Dispositions: `defect-claimed` (a rule claims it), `defect-unclaimed` (a recall gap by "never quieter than Solid"), `consequence` (the engine's reaction to an earlier failure), `advisory`, `out-of-scope` (SSR, host attribute checks, codes the published typings already reject).

Per runtime diagnostic of a defect code: a checker violation there is `agree`; an uncertifiable finding is a `recall-gap`; nothing that could claim it is a `candidate-false-clean`. "There" is any line of the diagnostic's stack, and the line of the call that registered a callback the stack runs through: a callback handed to `onSettled(cb1)` runs later, so its stack holds the lines inside `cb1` and never the registering line, where the checker answers (`registration.mjs` reads the link from the program tree: a stack line inside `const N = ...` belongs to the primitives that take exactly `N` as their callback, `N` declared once). A diagnostic no line could be given for is a false clean only when the checker has no claiming finding anywhere in the file. `ASYNC_OUTSIDE_LOADING_BOUNDARY` is reported by the scheduler, so it never has an authored line (`placelessCodes` in `codes.mjs`): `run.mjs` judges it by file, as the matrix does. A claiming violation anywhere in the file agrees with it, only declining claiming findings make it a `recall-gap`, and no claiming finding makes it a false clean.

The pipeline (`falseclean.mjs`, `falseclean-run.mjs`):

1. **Coarse key** = the code, the chain of primitives above the owner call (`root>untrack`, `memo`), and the dynamic stack shape (the roles of the diagnostic's frames: a primitive, a handler, JSX; wrapper functions and calls are left out). Closure layers, helpers and decoys do not change it; the fine signature (`dedupe.mjs`) keeps them apart. The frames are read through `print.mjs`'s `lineTable` (line to node path), not the nearest site by line. On the 179 Chrome-confirmed owner diagnostics of phase C the key gives 55 groups (15 to 18 per code).
2. **Ledger** `known-open.json`: `{ code, coarseKey, ref, reason }`, one exact key each, no wildcards. `ref` is `open-work.md#<anchor>` of a heading that exists, or `ADR nnnn` of a decision that exists; a bad ledger stops the run before it starts. A false clean matching an entry is `known-open`. A run reports the entries that did not fire (`stale-open` when the run covered the profile the entry came from). `--ledger <path>` or `--no-ledger`.
3. **Confirm**: with `--confirm-with-chrome`, one representative per new group (the smallest program) is run in Chrome. Confirmed means Chrome reports the same code at the same innermost App line with no covering finding. A diagnostic Chrome reports but could not place counts too (`confirmed-unplaced`): for a false clean, Solid spoke. Other outcomes: `line-differs`, `covered-in-chrome`, `refuted-by-chrome`, `chrome-inconclusive`.
4. **Shrink** (`--shrink`): the predicate is the same code, the same coarse key and no covering finding; the control is the legal twin, the program with its payload replaced by a literal, which must report no diagnostic of the code. The minimum is run in Chrome again and its status is reported (`minimum confirmed`); the fast path alone can drift.
5. **Exit status 4**: a confirmed false clean with a coarse key the ledger does not know. The sweep still runs to the end (exit 3, a confirmed false positive, stays the stop).

`findings.json` has a `falseClean` section (groups worst severity first, `knownOpen`, `staleOpen`, ranked `recallGaps`); `summary.json` carries the counts.

## The misuse profile and the recall matrix

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

**Runtime cache.** Solid's answer on a program does not depend on the checker. `matrix.mjs` keeps each program's type checks (clean and twin) and fast-path record under `--cache-dir` (default `$TMPDIR/solid-checker-fuzz-cache`, beside the neutrality baseline), keyed by the case's files and by an environment key: the generator version, the hash of this directory's judging sources, `scripts/runtime-oracle.mjs` (the type check compiles its entry), the clock, the `tsrs` binary and both installs' lockfiles. The checker is not in the key, so a run after a checker fix reruns only the checker. A record the machine produced (a runner that wrote nothing, a type check killed by its timeout) is never stored. `matrix.json` does not change; the hits and misses go to `cache.json` beside it. `--no-cache` turns it off, and so does `--real-clock`, whose records depend on the machine.

`--confirm-with-chrome` runs, per cell, up to `--chrome-sample` (3) agree programs, up to `--chrome-false-clean` (4) false-clean programs and every wrong-rule program in Chrome and records `same`, `differs`, `not-fired-in-chrome` or `chrome-inconclusive` beside the fast verdict. `--shrink` reduces the smallest Chrome-confirmed false-clean program of each cell (the control is the legal twin) and reconfirms the minimum in Chrome. Output: `matrix.json` (per cell counts, per binding and layer counts, per-seed verdicts, Chrome summary, minimum), `matrix.md`, `programs.jsonl`, `repros/`.

**Gate.** A cell's program seeds depend on the run seed, the code, the context and the index only (`cellSeed`), so a baseline and a later run with the same `--seed` and `--per-cell` judge the same programs. The gate fails (exit 1) when a judged program leaves `agree` for another judged outcome or enters `false-clean`, or a baseline cell is missing; it names the seeds that moved. `typed-away` and `harness` are not verdicts, they are what a loaded machine produces: a program that moves out of one of them is reported as "baseline unjudged" and never fails the gate, and a program that moves into one is an "infrastructure warning" (the run is suspect), also not a failure. A current run in which more than half the programs are typed-away or harness is refused (exit 2). Different seeds, per-cell counts or generator versions are refused (exit 2). A cell where the checker improved is reported, not failed.

## The packages profile

`--profile packages` gives a program one or two exports of a generated dependency package, `fz-lib`, so the fuzzer tests how the checker reads dependency source (ADRs 0291, 0297, 0300). The same tree, host and disposition machinery builds the program; the helper layer is an export of the package. `package.mjs` is the single place that says what each behaviour does at run time (call the callback inline or N times; in `setTimeout`, `queueMicrotask` or `Promise.then`; under `untrack`, `runWithOwner(null | getOwner())` or `createRoot`; pass it to `createEffect`, `createMemo`, `onCleanup` or `onSettled`; return it, a wrapper of it, or `{ cb }`; store it in a module variable, array or object, with or without its owner, for a second export, `fire`, to call later; never call it) and in which export form (named, arrow, default, an index re-exporting `impl.js` by name, renamed or `export *`, a class method, a static method, an object method, a namespace import). The generator knows each behaviour, so each shape's runtime truth is determined; `packages-seeds.mjs` pins sixteen of them and `node scripts/fuzz/packages-check.mjs --out <fresh>` runs them through the fast path and exits 1 when the runtime contradicts one.

* **Layout.** The package is real ESM with `.d.ts` typings and a `package.json` of `type: module`, `peerDependencies: { "solid-js": ... }` and an `exports` map (`types`, `default`), the shape the checker selects (ADR 0291). It imports `solid-js` from the program's install, never its own copy. A case holds it as `node_modules/fz-lib/` and only the exports the program refers to; the checker and the type check see it below the case's work directory, the fast path stages it beside `App.tsx` (the Vitest config inlines `fz-*` so the dev build of `solid-js` is the one instance), and `oracle.json` names it in `packages`, which `runtime-oracle.mjs` copies into the run directory's own `node_modules`. Nothing is written into the shared install's `node_modules`.
* **The checker runs as a user's run does.** A case with a package is checked without `--feedback-facts`: under a declared runtime target and build an ordinary analysis reads the package from source (ADR 0297), and the feedback mode `feedback run` uses does not. Chrome is the runtime only for such a case; its checker verdict is this run's, on the clean text, with the checker's run on the twin as the neutrality control (always on for this profile).
* **Reads.** A read payload is drawn only when every package export on its chain hands the callback's value back, so its witness can land.
* **Triage.** Uncertifiable results at the package call (SC9005 or SC9012) count as uncertifiable; with a runtime diagnostic they are recall gaps, not false cleans.

## Containers

`--profile containers` (or `--containers` on top of any profile, `disposal` and `owner-cleanup` among them) wraps expression statements of a generated component in a *container*, a place where code is written but runs elsewhere, later or not at all: an uppercase inner function never rendered, rendered as `<Child />`, handed to a slot (`props.slot?.(Child)`, a local object's method) or returned; a class instance field, static field, static block, method or constructor (constructed or not); dead code (`if (false)`, after `return`, a function never called, `false && f()`, with `true && f()` as the live twin). It also passes the callback of every primitive by name (`const`, `let` or a function declaration), rewrites calls as `f?.()`, `(f as typeof f)()`, `(void 0, f)()`, `(true && f)()` or `(f)()`, and puts an `await` before or after the payload of an async container. Every program type-checks with tsrs. A function-like container has an entry counter and a direct form (`f = h();`, `false && h();`) the glue counter of its call, so the ledger tells ran from not-ran (`containers` in `results.jsonl`: kind, use, `entered`); a violation at a payload site in a container that never ran is a `candidate-false-positive: unexecuted` (counted on every profile, containers included; see Unexecuted violations). The profile draws its base profile (mostly `disposal` with the cleanup handle called now or later) from a stream of its own, so every other profile's seeds are unchanged. Not built: a callback imported from another generated file (a case is one `App.tsx`), event handlers passed by name.

## The disposal profile

`--profile disposal` (`disposal.mjs`) builds a write that runs while an owner is disposed, the case rc.17's owned-scope write guard exempts (`disposalDepth > 0`) and an action call or a later run of the `onCleanup` handle does not. The host is an owner the driver can dispose (a `Show` a button toggles, a memo or effect a button reruns, a `createRoot` whose disposer a button calls); the cleanup keeps its handle discarded, kept, called at once or called later; a chain of wrappers (`runWithOwner(captured | createOwner() | null)`, `createRoot`, `untrack`, an IIFE, `forEach`, a helper, a timer, a microtask, a promise callback, an async function, `try`/`finally`, `flush()`) sits around the write, which is a signal setter, a store setter, `refresh(memo)` or an action call. Controls put the same chain in a component body (the write throws) or an event handler (legal). Cleanup evidence (did the driver really dispose the host, did the handle run) is in `results.jsonl` (`cleanup`), so a write that never ran under disposal proves nothing. `disposal-seeds.mjs` pins the known shapes with the runtime's answer for each; `node scripts/fuzz/disposal-check.mjs --out <fresh>` runs them through the fast path and exits 1 when the runtime contradicts one.

## How the evidence works

* **Sites.** Every payload read or write is a site. Its line is where its call is written.
* **Witnesses.** A site carries the page text that shows it ran and landed: a write to its own probe signal shows its unique value; a read's value reaches a sink signal or a rendered child. A site without a witness is never evidence of silence. Decoy constructs (a tracked memo read, a write in an apply, a write in a handler) are legal by construction and have witnesses too, so a violation on one is a candidate.
* **One component per page.** A thrown dev error halts reactivity for every root on the page, so a second component would hide or fake the first one's behaviour. The default is one component; `--components` is for experiments.
* **Disturbance.** Any throw, `REACTIVITY_HALTED`, or runner failure makes the other findings of that program inconclusive.
* **Same driver both sides.** The fast runner repeats the oracle's two passes of DOM events on every element, then clicks the plan's buttons and reads the witnesses; the oracle runs the same events and the same plan as scenario steps (`oracle.json`).
* **Signature.** The identity of a finding is its rule plus the construct path from the component to the site (host, closure forms, defaults, helper form, IIFE/forEach/map) and what the program does with each closure on that chain (called, returned, stored, passed to a helper of some form, called from a handler). Names, values and the rest of the file are ignored.

## The hit ledger

Every place the runtime can execute is counted. `print.mjs` emits two texts from one tree: `text` (clean, what the checker reads and what an owner reads in a repro) and `runText` (the twin: type check, Happy DOM, Chrome). The twin has `(__h(N), expr)` on each payload site and each *glue* call (every other emitted call: `m1()()`, sinks, memo accessors, primitives; only the top of a call chain is counted), `__h(N); ` before a statement that is itself a counted call, `{ __h(N);` at every function entry, and the counter's source after the last line, so no line moves. The counter touches no Solid state: it increments `globalThis.__scHits` and mirrors the counts as JSON into a plain-DOM `<output id="sc-hits">` outside `#root` (one pixel, transparent: the oracle's `wait-for-selector` needs it visible). The fast runner reads the global and polls until it is still (quiescence); Chrome reads the last two `#sc-hits` checkpoints, which the twin's `oracle.run.json` adds as steps.

A site is judged by two separate facts: `ran` (its counter is above zero) and `landed` (its witness text is on the page). A payload with a witness needs `landed`; a site with none (a glue call, a void payload) is judged by `ran`; a site that ran and did not land is inconclusive. A finding on a plumbing line (no payload site on its lines) is judged by the glue calls on them, replacing "the finding spans no site of interest". A payload site that never ran, placed in the ledger, on a quiet page whose host ran, is a `candidate-unexecuted`; the rest of the never-ran rows stay `inconclusive`. Every inconclusive item carries a `why` (`never-ran`, `ran-not-landed`, `no-witness`, `disturbed`, `perturbed-run`, ...). `results.jsonl` rows carry a `hits` summary per program.

### Neutrality

The counters must not move anything the fuzzer judges. Four checks, none needing Chrome except the last:

1. `strip(runText) == text` (`hits.mjs stripInstrumentation`, checked when the case is built; a failing program is inconclusive `not-neutral`).
2. `--control`: the clean text runs in the same engine and the (code, line) diagnostics, throws, witness texts and failure phase must match; the checker also runs on the twin's text and its (rule, kind, line) findings must equal the clean ones. A difference makes the program inconclusive `perturbed-run`.
3. `node scripts/fuzz/neutrality.mjs --seed 1 --count 300 --profile <p> --control --out <fresh>`: the same seed range with and without instrumentation (`--no-instrument` is the baseline mode of `run.mjs`). Only an `inconclusive` verdict may move, and only to a resolved one; any other change exits 1. It also reports how many findings that used to span no site now map to one. `--baseline <results.jsonl>` reuses a recorded baseline from the same checker; `--baseline a --candidate b` compares two recorded runs.
4. Chrome: the oracle runs the checker on the twin; its findings must equal the fast pipeline's clean ones, else `instrumentation-perturbed-checker` (inconclusive, `chrome-inconclusive`).

Observation: the checker reports nothing on `(f(), m1()())` as a statement in a component body where it reports `m1()()` written plainly (the comma expression hides the read). The counters avoid the form for that reason (statement counters); the gap is the checker's, and is recorded in the report that added the ledger.

### Speed

Nothing here changes a verdict (the acceptance run compares them): the fast path runs in one long-lived Vitest host, whose per-file environment is the isolation (`fast-daemon.test.mjs` has a program leave a global, a document attribute, a prototype property and a halted signal runtime, and the next must find none); `--concurrency` (checker and type check) defaults to half the cores and `--workers` (Vitest) to twice the cores, since a program mostly sleeps on timers (on the virtual clock, the default: one a core); the neutrality baseline is cached under `--cache-dir` (default `$TMPDIR/solid-checker-fuzz-cache`) keyed by generator version, program, checker and type-facts sha256, the fast install's lockfiles and the hash of this directory's judging sources, so a later round re-runs only the instrumented pass (`--no-cache` to disable); `--order failing-first` puts what mismatched last time first and `--stop-on-mismatch` ends a round at the first mismatching chunk.

A shrink with `--control` asks for the neutrality control of each reduction it accepts, not of every candidate it tries (the control can only turn a verdict into `inconclusive`, so the answers are the same; `--shrink-control each` restores the old rule), shrinks its false-clean groups in a pool (`--shrink-jobs`) and reconfirms their minima in Chrome in one call. Chrome runs a case's clean control beside its twin in one oracle call, and runs a case that ended in `browser close timed out after N ms` once more. The fast path runs on a **virtual clock** (`fast/virtual-clock.mjs`; `--real-clock` restores the wall clock, the behaviour before it existed). The program's `setTimeout`, `setInterval` and their clears run on a clock the runner advances, so the runner's waits (60 ms after the render, after each pass of events and each click, 150 ms twice, 50 ms ticks of the hit-ledger settle) keep their lengths in virtual time and cost no wall time. It keeps Node's delay rules (a delay under 1 ms is 1 ms), the order of timers (expiry, then creation), a chain of 0 ms timers advancing a millisecond a link, the real microtask queue drained between two timer callbacks, and a throwing callback reported as an uncaught exception with the timers after it still run. It does not model: anything that waits on real time or I/O (`setImmediate`, `process.nextTick`, `Date`, `performance.now` stay real), a program slow enough that a real timer would have fired late (the real clock's verdicts depend on the machine there, the virtual clock's do not), and host timers that do not go through `setTimeout`/`setInterval` (`requestAnimationFrame`). A timer chain runs for the virtual length of the wait, as a real chain runs for its wall length, so a chain of 0 ms links runs one link a virtual millisecond. The generator emits `setTimeout(f, 0)`, `queueMicrotask`, `Promise.then` and `await` only. Equality with the real clock was measured, not argued: 15 profiles x 300 seeds, every row (verdict, items, hit counts, control) identical (`compare-runs.mjs`; `neutrality.mjs` over six of them exits 0 under both). **Chrome is never on it**: the oracle runs the real browser on its real clock, for confirmations and for the rerun of a shrunk minimum; the flag changes only the Happy DOM fast path, and a verdict Chrome cannot confirm stays unconfirmed.

`node scripts/fuzz/compare-runs.mjs <run A> <run B>` is the equality check for any of the above (every row but the wall times, the findings, the summary counts and the shrunk minima byte for byte). `--unexecuted-moves` compares two runs of the same seeds from before and after `candidate-unexecuted` existed: a row may differ only by an `inconclusive: never-ran` item that became a `candidate-unexecuted` one (listed as a move), and any other difference fails.

### Unexecuted violations

ADR 0458: a violation at a site no execution reaches is uncertifiable, so the checker reporting one is a false positive. Five of them were found by a person reading `inconclusive: never-ran` rows. Now the run counts them (`summary.json` and `findings.json` `unexecuted`: candidates, programs, confirmed, by status, by basis, groups).

* **Candidate** (`candidate-false-positive: unexecuted`, verdict and item kind `candidate-unexecuted`, `unexecuted.mjs`), on every profile and with no flag. A violation of a rule a runtime diagnostic can contradict, whose payload sites the ledger shows at counter zero, each placed in the ledger, with the page quiescent, a function enclosing the site entered, and no disturbance. Its `bases` say why: `dead-code` (below `if (false)`, after `return`, in `false &&`), `container-not-entered`, `function-not-entered`, `in-default`, `skipped-in-entered-function`. A site that ran, a finding on a plumbing line (judged by glue calls), a site the ledger cannot place stay `inconclusive`. The verdict ranks under `recall-gap` and above `inconclusive`, so only a row that was `inconclusive: never-ran` moves. A candidate never exits 3.
* **Confirmation** (`--confirm-with-chrome`). Each candidate's program runs in Chrome with the counters, and the fast path runs its clean-twin control. `confirmed-unexecuted` (exit 3, shrinkable) needs every condition of `deadsite.mjs` on Chrome's records, plus no diagnostic of the rule's codes anywhere. Anything else is typed: `refuted-by-chrome` (`chrome-ran`, `chrome-diagnostic`) or `chrome-inconclusive` (`chrome-failed`, `chrome-checker-differs`, `rule-code-reported`, `instrumentation-perturbed-checker`, a disturbance, or `conditions-unmet` with the conditions). The conditions are the old dead-site ones with two narrowings: the closed grammar admits the containers of `container.mjs` (`closedGrammar(program, { containers: true })`, `comp-prop` excepted since the caller supplies the prop), and a site the text itself skips (`lexicallyDead`) needs neither the grammar nor the no-throw reading, since no driver and no throw is behind a skip the text makes. Every other site keeps every condition.
* **`--dead-site`** now means the strict confirmation: the strict grammar (no container) and no waiver, the conditions as they stood before. It is a mode, not a second path. Folded, not kept as its own subkind, on this evidence: the strict conditions are a subset of the default ones, so it cannot confirm anything the default does not; and it confirms none of the five known false positives (m1-m5, all containers; each ends `closed-grammar`), so as a separate verdict it would stay silent where this was built to speak. It stays only as the way to ask for the smaller claim.
* **Not covered.** A finding on a plumbing line stays `inconclusive`, so two of the five original programs (a createEffect call after an `await` in a component never rendered) are not flagged in that shape; their minimal forms with a payload site are. A shrunk repro keeps its witness probes (Chrome waits on them) but is not rerun in Chrome.

## Equivalence mode

`node scripts/fuzz/equivalence.mjs` is metamorphic testing. For each generated program it runs the checker on the program and on `--equivalence N` variants (default 3), each the program under **one** rewrite that does not change what it does at run time (`rewrite.mjs`). The verdict at every site of interest is compared across the two. The rewrites are equivalent by construction, so a different verdict is a checker placement path that handles one spelling and not the other; no runtime is needed to find it, and Chrome only says which side is right. It takes the generator flags of `run.mjs` (`--profile`, `--seed`, `--count`, the tool flags), works with every profile, and leaves `run.mjs` and its verdicts untouched when it is not used.

Rewrites (`rewrite.mjs` states each one's soundness argument beside it; none moves code into or out of an effect, `untrack`, a component body or an async boundary, so none depends on timing, owner or tracking): `rename` (alpha-renaming of same-file names), `extract` (`const t = e;`, `const t = e; return t;`), `block`, `if-true`, `try-finally`, `iife` and `outline` (a statement in a synchronous IIFE, or a helper declared and called at once), `inline` (the reverse of `iife`), `callform` and `accessor-form` (`(f)()`, `f?.()`, `(void 0, f)()`, `(f as typeof f)()`, `(true && f)()` on a function name and on a signal accessor or setter), `declform` (function declaration against `const` arrow, only where hoisting is not observable), `reorder` (a closure declaration swapped with an independent neighbour) and `dead-decl` (an unread declaration). A wrapper prefixes the first line of the statement and suffixes its last, so a one-line statement keeps its line. A variant is type-checked with tsrs and dropped (counted as `type-error`) when it does not pass. A subset of the places a rewrite fits is drawn from the variant's seed; `--rewrites a,b` and `--exclude-rewrites a,b` pick the kinds.

The **site map** is a tag: every site node carries its original id (`eqSite`) through the rewrite, and the variant's printed sites map back to the original's by that tag.

| bucket | meaning |
| --- | --- |
| `high` | a violation on one side and clean on the other, or two different rules. A user-visible difference. |
| `certainty-divergence` | uncertifiable on one side and clean on the other. The forms are equivalent, so the clean side claims a proof the checker cannot make on the other: a possible silent clean (fail-closed). `cleanSide` says which. |
| `low` | a violation on one side and uncertifiable on the other, or uncertifiable against another rule. Internal (ADR 0328). |

Each divergence is a typed `equivalence-divergence` with both programs (`divergences/<variant id>/original.App.tsx`, `variant.App.tsx`, `divergence.json`; `*.program.json` are the trees). Groups are keyed by rewrite kind plus the fine signature of the original site (`equivalence.json`, `divergences.jsonl`, `results.jsonl`). With `--confirm-with-chrome` one representative per high and certainty group (`--chrome-max N`, default 40) runs in Chrome, original and variant together: `violation-confirmed` (Chrome reports a defect the violating rule claims, so the other side misses it), `violation-contradicted` (the site ran undisturbed with no diagnostic: the violation side is a false positive, **exit 3, stop and report**), `defect-at-site` / `silent-at-site` (certainty divergences), `inconclusive`, and `unsound-rewrite` (the two Chrome runs show different diagnostics or witnesses: a fuzzer bug, **exit 5**; drop the rewrite with `--exclude-rewrites`). A run the driver could not finish (a throw halts reactivity and the next step finds no element) is judged one way only: a diagnostic it reached before the failure can confirm a violation or show a defect, but a missing diagnostic never yields `violation-contradicted`, `unsound-rewrite` or `silent-at-site`; those stay `inconclusive`, because the part that never ran could hold the diagnostic. A run in which no program was checked is refused (exit 2). `--shrink` (`--budget`, `--shrink-max`) reduces one program per high and certainty group while the same divergence holds, the rewrite applied at every place it fits (`repros/<hash>/`).

Equivalence mode does not make a rewrite a license to compare unlike code: a rewrite that is only equivalent under a condition (a name used before its declaration, a `return` in an IIFE) checks the condition and declines. Calibration: on a checker from before PR #128 (a violation at a site no execution reaches was reported), containers programs show `low` divergences between `if (false) { createRoot(f1) }` and the same statement outlined into a called closure; on main they agree.

## Replay

`node scripts/fuzz/replay.mjs --out <fresh dir> <dir|list> [...] [--confirm-with-chrome]` re-judges recorded programs with the current checker without regenerating their seeds: the recheck after a checker fix. An argument is a directory or a text file that lists directories (one per line, an optional label after the path, `#` comments). A directory is an item when it holds:

| item | files | what is judged |
| --- | --- | --- |
| pair | `original.program.json`, `variant.program.json` (an equivalence `divergences/<variant id>/`) | both trees rebuilt as cases, compared at the recorded sites by the site map; Chrome judges them as equivalence mode does |
| text pair | `original.App.tsx`, `variant.App.tsx`, `divergence.json` (a shrunk equivalence repro) | both texts, compared at the recorded lines; Chrome runs each text with the oracle's own driver |
| plain | `App.tsx` (a repro) | the checker's findings against the defect diagnostics Chrome reports in the file |

Any other directory is searched for items. Per item (`replay.jsonl`, `replay.json`, a line on stderr): the bucket now (`agree` when both sides agree at every recorded site), the bucket and Chrome status recorded with it (a pair reads them from its run's `divergence.json` and `equivalence.json`), Chrome's status now, `chromeCache`, and `fixed`. An item is `fixed` only when both sides agree on a violation and Chrome still reports a code its rule claims there; two clean sides agree but are not fixed. A plain repro is fixed when every defect diagnostic Chrome reports is covered by a claiming violation. Exit 3 when Chrome contradicts a violation (stop and report).

**Chrome cache.** `replay.mjs` and `equivalence.mjs --confirm-with-chrome` keep each Chrome record under `--cache-dir`, keyed by the staged case's files, the call's mode, the judging sources, `scripts/runtime-oracle.mjs` with the product scripts it drives Chrome with (`packages/cli/scripts`), the browser build and the oracle install's lockfile, so a recheck runs Chrome only on new programs. It is for confirmation only. The checker is not in the key, so a cached record's checker half is the run's that stored it: callers that judge the checker by Chrome's findings (`run.mjs`, `matrix.mjs`) do not use it. A cached answer is named on its row (`chromeCache: "hit"`, `[chrome cache hit]`), so it never turns a fresh disagreement into agreement unseen. Harness errors and pages that never loaded are not stored. `--no-chrome-cache` turns it off.

**Halted runs.** A generated case asks the oracle for its halt marker (`"haltMarker": true` in `oracle.json`): a second page module, outside the case's tsconfig so the checker never reads it, that appends `<output id="sc-halted">` when Solid reports `REACTIVITY_HALTED`. Every click and witness step names it as its alternative (`X, body:not(:has(X)) > #sc-halted`, `haltable` in `hits.mjs`), so a wait for an element a halt keeps from appearing ends at once instead of after the scenario's 12 s timeout; with no halt the selector is `X`. `chromeRuntime` reads such a checkpoint as absent and the run as cut short (`failure.phase: "halted"`), so the partial-run rule stands: a diagnostic reached may confirm, a missing one never contradicts. The fast path needs nothing: on the virtual clock a wait costs no wall time.

## Known limits

* The fast path is Happy DOM, not a browser, and Chrome judges only its candidates: a false positive the fast path does not produce is not looked for in Chrome. A candidate the fast path invents is refuted by Chrome and noted, not reported as a checker bug.
* One component and one `App.tsx` per case: no import from another generated file (a callback "from another module" is not generated; the packages profile is the dependency-source stand-in), and no event handler passed by name (containers).
* A program `tsrs` rejects is dropped (invariant 1), so a defect the published typings catch is out of reach by design: the matrix records such codes as `typed-away` (`MISSING_EFFECT_FN` has no template).
* A site that never ran, or ran without landing its witness, is `inconclusive` or an unexecuted candidate, never agreement. Sites in containers that never ran, programs that throw or halt reactivity, and diagnostics Chrome cannot place on a line limit what a run can say.
* `ASYNC_OUTSIDE_LOADING_BOUNDARY` is reported from the render root, so the matrix judges it wherever the program reports it (`placeless`).
* The instrumentation hides the read in `(f(), m1()())` from the checker; the counters avoid the form, and the gap is the checker's (see Neutrality).
* Packages: only the export forms and behaviours `package.mjs` lists; a read payload only through exports that hand the callback's value back; the checker's run is the clean text's, not Chrome's own.
* The matrix gate compares the same seeds and per-cell count against a recorded baseline of a given generator version; it says nothing about programs it did not draw.
* The fuzzer is operator tooling: it is not part of `make verify`, and the checker and its gates do not read it. Do not change the checker from inside a fuzzing session.

## Layout

| file | role |
| --- | --- |
| `model.mjs` | the construct tree and its validator (a small type checker for the node language) |
| `generate.mjs` | seed -> program: a payload (write or read) in layers (closures, defaults, IIFE, forEach/map, helpers), a host scope, a disposition |
| `print.mjs`, `emit.mjs` | tree -> the clean `App.tsx` and its instrumented twin `App.run.txt`, with the site table, glue (auto-sites) and hit ledger; an oracle case directory (`App.tsx`, `oracle.json` steps, `program.json`, `sites.json`, and the twin's files) |
| `container.mjs` | the containers dimension: wraps statements in places that run elsewhere or not at all (components, classes, dead code), passes callbacks by name, rewrites call forms; the ran/not-ran evidence per container |
| `hits.mjs` | the hit ledger: the counter's source, the strip, the readout parser, the per-site record, and the pure comparisons of the neutrality gate |
| `neutrality.mjs` | the regression: a seed range with and without instrumentation, verdict by verdict (`--profiles`, `--chunk`, `--order failing-first`, `--stop-on-mismatch`) |
| `fast-daemon.mjs`, `fast/serve.mjs` | the long-lived fast host: one Vitest process (Vite server, Solid plugin, transform cache) serving every batch; `--no-daemon` gives one process per batch |
| `cache.mjs` | the baseline cache of the regression and its mismatch history; the runtime and Chrome caches |
| `unexecuted.mjs`, `deadsite.mjs` | the unexecuted candidate (the ledger's proof), Chrome's judgement of it, and the confirmation conditions as code |
| `typecheck.mjs` | `tsrs -p ... --noEmit` against the published typings; a program that does not type-check is dropped |
| `checker.mjs` | materializes a case as the oracle does and runs the native checker; per-site verdicts |
| `fast.mjs`, `fast/` | the Happy DOM + `@solidjs/diagnostics` runtime path (Vitest) |
| `classify.mjs`, `codes.mjs` | the comparator; the policy table per rc.17 code and the oracle's code/rule tables (tests fail when they drift) |
| `falseclean.mjs`, `falseclean-run.mjs`, `known-open.json` | the false-clean stream: coarse key, ledger, Chrome confirmation, focused shrink with a control |
| `shrink.mjs`, `dedupe.mjs` | delta debugging over the tree; signatures by construct path |
| `chrome.mjs` | Chrome confirmation through `scripts/runtime-oracle.mjs` |
| `misuse.mjs` | the misuse profile: a template per claimed code, the contexts, layers and binding forms |
| `matrix.mjs` | the recall matrix (code x context), its Chrome sample, the false-clean shrink and the baseline gate |
| `package.mjs`, `packages.mjs`, `packages-seeds.mjs`, `packages-check.mjs` | the `packages` profile: generated dependency packages (below) |
| `rewrite.mjs`, `equivalence.mjs` | equivalence mode: runtime-identical rewrites of a program, the per-site comparison across them, the buckets and the Chrome judgement |
| `replay.mjs` | re-judges recorded pairs and repros with the current checker, optionally in Chrome (Replay) |
| `compare-runs.mjs` | equality of two runs (rows, findings, summary, minima), the evidence that a speed change moved nothing |
| `run.mjs` | the command line |

## Phases

1. Generator, type-check, classifier, shrinker, dedupe, CLI. Unit tests: `packages/cli/node_modules/.bin/vitest run --config packages/cli/vitest.config.mjs scripts/fuzz`.
2. Rediscover this week's false positives on the pre-fix checker (ADR 0452 families), then confirm the fix clears them.
3. Sweeps on the current checker; confirmed findings go to the owner.
4. The hit ledger (phase A): the twin, ran/landed, glue sites, the neutrality gate. Unexecuted violations are counted, then confirmed in Chrome.
5. Speed: one long-lived fast host, a baseline cache, batched shrinking.
6. Owner and disposal profiles (phase C), then false cleans (phase D): the policy table and its drift test, the verdict tiers, the ledger, coarse dedupe, Chrome confirmation, focused shrink, exit status 4.
7. The misuse profile with the recall matrix and its gate; the packages profile; the containers dimension.

## Provenance

The shrinker's search loop (execution budget, seen-set of canonical candidates, validity filter before any run, sweep schedule, control hook, complexity ordering), its halving-width chunk removal, and the idea of weighted generation come from Solid's semantic fuzzer, solidjs/solid PR #3446 "test(signals): add experimental semantic fuzzer" (open, MIT), commit 030e77f2b, `packages/signals/tests/semantics/` (`search.ts`, `shrink.ts`, `complexity.ts`, `candidates.ts`, `generate.ts`). `shrink.mjs` carries the per-file note. That fuzzer interprets a scenario DSL against the signals engine and checks written rules; this one emits Solid source and compares the checker with the runtime, so the generator, the candidate families, the oracle and the dedupe differ (`footprints.ts` dedupes by an engine work ledger; a checker finding has none, so the construct path replaces it). The fast path's runner and shims are ported from the Happy DOM experiment (`happy-patched-runner.tsx`, `happy-dom-fixes.ts`), without its corpus-specific doubles.
