# General package callbacks, caller locations and silent async reads

The experiment now includes **90 additional consumer records across seven
general-purpose packages**. Shared runtime feedback catches owned writes and
cleanup mistakes without a contract for each callback API. It survives errors
caught by Lodash and RxJS. A collector fix preserves two distinct caller paths
through the same callback while coalescing repeated failures on one path.

There is also a clear limit: **two demonstrated async dependency losses remain
silent in both the runtime channel and original-source native analysis**.
They use RxJS and Neverthrow. A third, Zod case is caught statically because
the source contains a visible `await`. Useful package feedback is feasible;
runtime observation alone is insufficient for all reactive dependency rules.

## Populations and results

All counts require a matching diagnostic, with authored expectations used only
after detection. Unrelated exceptions cannot count as a semantic detection.
Correct-use controls require both silence and the declared behavior. Every
artifact has `authority: false` and `certification: false`.

| Population | Targets with matching feedback | Quiet, working controls | Typing exclusions |
| --- | --- | --- | --- |
| First fifty consumers, original browser profile | 18/21 | 22/26 | 3 |
| Same fifty, loader repaired, original collector | 21/21 | 26/26 | 3 |
| Same fifty, revised collector | 21/21 | 26/26 | 3 |
| Thirty-eight fresh consumers, revised runtime collector | 13/16 | 18/20 | 2 |
| Same fresh consumers, plus original-source analysis of six async cases | 14/16 | 18/20 | 2 |
| Two separately authored correction controls | 0 targets | 2/2 | 0 |

The first profile has seven Lodash loading failures. They remain in its report
and score. The fresh population has two incorrect control premises, explained
below; its original score and consumers are preserved. There are no unrelated
feedback successes among executed misuse targets.

These are new consumers, not seven packages held out of every earlier study.
Several packages appeared in earlier callback surveys. The populations are
authored challenges, not a random ecosystem sample or a coverage estimate.

## Installed packages

| Package | Version | Callback surfaces exercised |
| --- | --- | --- |
| Lodash | 4.18.1 | `attempt`, `reduce`, trailing `debounce` |
| RxJS | 7.8.2 | `map`, `filter`, error delivery, `observeOn`, `delay`, `firstValueFrom` |
| Neverthrow | 8.2.0 | `Result.map`, `mapErr`, `ResultAsync.map`, `match` |
| Zod | 4.4.3 | `transform`, `superRefine`, async transforms |
| Valibot | 1.4.2 | `transform`, `check` |
| TanStack Query Core | 5.101.4 | Query-cache and mutation-cache subscriptions |
| Floating UI DOM | 1.8.0 | Initial `autoUpdate` callback, asynchronous middleware |

Solid, signals and web are the retained **2.0.0-rc.9** artifacts. Lodash uses
its retained published declarations. No package was installed, patched or
replaced. Preflight seals **409 resolved declaration files** for each main
population. The correction probes resolve 133 declaration files.

## What the shared runtime channel establishes

The first population tests owned writes and forbidden leaf cleanup in every
package. Four pairs test cleanup delivered after a microtask: the target has
no owner, while the control restores the still-live captured owner. Another
pair tests a disposed owner. Deferred writes through debounce, RxJS scheduling,
Zod async transforms and `ResultAsync` remain quiet and produce the expected
value. An empty stream does not invoke its callback and stays quiet.

Lodash `attempt` catches the thrown write/cleanup error. RxJS delivers it to
the subscription's error handler. In these four single-call targets there is
no escaping exception for the generic exception channel to report, but the
native diagnostic subscription still records the precise failure. Thus a
generic exception catcher is not a substitute for the diagnostic channel.

No source package model or inferred callback contract enters this detector.
These are actual runtime diagnostics about executions that occurred. They do
not prove that every callback path has been executed or that a quiet package
is correct.

## Loader and caller fixes

The first runner disables automatic dependency discovery. Lodash's CommonJS
build then reaches the browser without the default export adapter. The new
version of the browser runner enables Vite's dependency discovery. The
unchanged fifty consumers subsequently execute successfully. This is loader
support; it does not change package semantics or accepted contract authority.

The old collector deduplicates by diagnostic code and nearest application
frame. When `first()` and `second()` invoke the same invalid callback, the
runtime emits two diagnostics but the collector retains one. The new
`extended-runtime-feedback-v2.mjs` includes the complete observed application
frame path in the key. A repeated key increments `occurrences` instead of
creating another message.

On the unchanged fifty consumers, an independent parity evaluator confirms
identical source, labels, typings, installed package bytes, displayed behavior,
callback counts, diagnostic deliveries and caught exceptions. Only the two
multicaller records gain a second feedback item.

The fresh population repeats this through six other API surfaces. Each
two-caller target retains two messages, each with one occurrence. Each target
that invokes the same path four times retains one message with four
occurrences. All twelve paired controls stay quiet. These are synchronous
stack paths; async registration ancestry and arbitrarily deep stacks remain
outside the claim.

An independent TypeScript-based auditor checks **44 runtime diagnostic
witnesses** across the two populations. It resolves each mapped location to
the exact native `onCleanup`/`runWithOwner` call or the setter binding returned
by an exact native `createSignal` call. It also checks mapped consumer frames,
source digests, typings and agreement between occurrence counts and actual
diagnostic deliveries. All twenty-four authored occurrence expectations in
the fresh set pass. The auditor does not import the collector.

## Silent async dependency loss

Three paired consumers return async package pipelines from `createMemo` and
render the resolved result under `Loading`. The target reads the signal only
after the package's async delivery. Its control captures the signal during the
memo's synchronous compute and passes that value into the pipeline.

| Pipeline | After signal changes from 1 to 2 | Runtime feedback | Original-source native feedback |
| --- | --- | --- | --- |
| RxJS delayed stream and promise continuation | Target 1; control 2 | Both quiet | Both quiet |
| Neverthrow `ResultAsync.map` | Target 1; control 2 | Both quiet | Both quiet |
| Zod async transform with visible `await` | Target 1; control 2 | Both quiet | Target `reactive-read-after-await`; control quiet |

All six consumers pass the actual published typings. Native analysis uses the
unchanged, pinned analyzer and Type Facts producer on the original executed
files. No package analog or analysis rewrite is executed. Six analyses take
about **40.7 seconds** in total. These results show why static execution facts
and package callback knowledge still matter alongside runtime diagnostics.

The two remaining cases need a further experiment: observing exact reactive
reads made by deferred callbacks, then associating them with the computation
that created the callback. Any inferred dependency intention should begin as
informational feedback with explicit open facts. Treating an untracked read
as a proven defect would repeat the intentional-snapshot precision problem.

## Two incorrect control premises

I authored two controls expecting `untrack(() => dispatch(callback))` to permit
a setup write. The installed rc.9 preserves the owner in `untrack`. Its
`setSignal` rejects the write based on that owner, so both controls correctly
receive `REACTIVE_WRITE_IN_OWNED_SCOPE`. Mount fails before exposing a dispose
function, which also produces a harness failure. Their original labels and
scores remain unchanged.

Two new probes replace only that wrapper with
`runWithOwner(null, () => dispatch(callback))`. Both execute once, update
successfully and stay quiet. These are additional correction observations,
not relabeled replacements in the original population. Original-source native
analysis also stays quiet on the two invalid external-callback controls; that
does not override the observed runtime failures.

## Freeze chronology and validation limits

The revised collector and browser code are sealed before the fresh population
is authored. The import-only detector seal omits two modules loaded through
literal strings: the guard recorder and origin recorder. The original study
therefore conservatively reports that its full profile is not covered by that
one seal.

`callback-context-seal-v1.mjs` separately authenticates those modules against
the original 162-file seal and an earlier completed caller browser profile.
Both precede the fresh population. Together the earlier seals cover all
nineteen non-consumer profile inputs for the fresh run. The same check leaves
four genuinely adapted files uncovered for the original fifty consumers;
it does not describe that replay as a fresh detector result.

The first parity evaluator rejected absolute paths from different run
directories in otherwise identical TypeScript diagnostics. It produced no
artifact. The preserved v2 evaluator compares paths relative to each run and
handles excluded rows without assuming they executed.

## Verification and remaining work

- **167/167 prototype tests pass**, with no skips. Five new tests cover
  distinct callers, repeated occurrences, unknown locations, excluded type
  diagnostics, and category/unsubscribe behavior.
- Syntax validation passes for the 236 modules then present; the subsequently
  added seal module is checked separately.
- Eleven historical/current detector manifests retain all 411 checked file
  pins. Original source, flow, labels, published typings and package closures
  authenticate in the study and independent audits.
- The five TS2345/TS2339 exclusions are never executed and receive no feedback.
- `make verify-fast` passes producer freshness, Rust formatting and pinned
  workspace Clippy. Schema, dialect manifest and whitespace checks pass.
- Browser batches take 50.2 seconds for the repaired fifty consumers, 37.6
  seconds for the fresh thirty-eight and 2.6 seconds for the correction pair.
  These include isolated browser pages, compilation and type checks. They are
  batch measurements, not editor latency or production performance.
- No production Rust rule, public contract, manifest or finding snapshot
  changes. Full `make verify`, production coverage/ownership gates and contract
  certification gates are deferred for this isolated research addition.

Further work includes the two silent async reads, real application warning
review, cross-file callbacks, HMR invalidation, worker/server contexts and
editor costs. Earlier escaped/borrowed receiver misses, the pagination package
defect and the two intentional-snapshot noisy controls remain unresolved.

## Reproduction and artifacts

The case modules are `callback-context-cases-v1.mjs`,
`callback-context-transfer-cases-v1.mjs` and
`callback-context-detached-controls-v1.mjs` under
`benchmarks/reviewed-package-models/`. Use fresh output paths when rerunning:

```sh
node benchmarks/reviewed-package-models/callback-context-browser-run-v1.mjs \
  benchmarks/reviewed-package-models/callback-context-transfer-cases-v1.mjs \
  benchmarks/reviewed-package-models/family-matrix-feedback-v2.mjs \
  rust/target/callback-context-transfer-new-browser '<retained Chromium path>'

node benchmarks/reviewed-package-models/callback-context-study-v1.mjs \
  rust/target/callback-context-transfer-preflight-v1/population.json \
  rust/target/callback-context-transfer-browser-v1/browser/results.json \
  rust/target/callback-context-transfer-new-study.json
```

Retained evidence is under `rust/target/callback-context-*`. Key outputs are
`callback-context-study-v3.json`, `callback-context-transfer-study-v1.json`,
`callback-context-native-v1/results.json`, `callback-context-detached-study-v1.json`,
the two audit reports, `callback-context-parity-v2.json` and
`callback-context-transfer-seal-v1.json`. Earlier modules and executed
populations are preserved.
