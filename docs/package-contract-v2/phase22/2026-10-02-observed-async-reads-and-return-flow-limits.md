# Observed async reads and limits of value flow

The two previously silent RxJS and Neverthrow async dependency losses now
receive useful informational feedback. The prototype observes exact native
accessor reads and connects their source locations to callbacks created inside
a native memo. This requires no contract for each external callback API.

There is a credible route to useful feedback across many packages. The evidence
does not establish universal coverage or whole-package correctness. The latest
fresh population finds 5/8 target problems and keeps 16/17 correct-use controls
quiet. Three misses and one unwanted intentional-inspection hint remain.

## Populations and results

This slice reuses six unchanged consumers from the previous callback study and
adds 59 consumers in two stages. Labels and declared behavior enter scoring
only. They do not enter source selection, instrumentation or hint projection.
All research artifacts retain `authority: false` and `certification: false`.

| Population and detector | Targets with matching feedback | Quiet, working controls | Real typing exclusions |
| --- | --- | --- | --- |
| Six original async consumers, V1 | 3/3 | 3/3 | 0 |
| First 32 additional consumers, V1 | 6/11 | 17/18 | 3 |
| Same 32, adapted V2 | 10/11 | 18/18 | 3 |
| Further 27 consumers, V2 sealed before authoring | 5/8 | 16/17 | 2 |

The original three targets use RxJS, Neverthrow and Zod. All render 1 after
their source changes to 2. Their paired controls capture the source during
the memo compute and render 2. The previous native analysis already catches
Zod's visible `await`; RxJS and Neverthrow were silent in both channels.
The new hint makes those two losses visible. RxJS has two observed read sites,
so the six-consumer replay produces four hints for three targets.

The 10/11 result is an adaptation to observed cases. It is not an estimate of
performance on unseen consumers. The further 27-consumer population preserves
its lower result, original labels and all misses. These are authored challenges,
not a random package sample. No executed target counts an unrelated exception
as success. All executed controls have their declared behavior and there are
no execution-harness failures in this slice.

## Exact retained package artifacts

The first challenge covers Lodash 4.18.1, RxJS 7.8.2, Neverthrow 8.2.0,
Zod 4.4.3, Valibot 1.4.2, TanStack Query Core 5.101.4 and Floating UI DOM 1.8.0.
The further population covers six of those packages, omitting Zod. Solid,
signals and web are the retained 2.0.0-rc.9 artifacts. All published declaration
files and package closures are authenticated; there are 389 resolved declaration
files for the original six, 409 for the first challenge and 329 for the further
population. No package installation or package-source patch is involved.

Published TS2588, TS2345 and TS2339 inputs in the first challenge are excluded
before execution. The further population excludes TS2339 and TS2554 inputs.
They receive no feedback. Fixture stubs do not supply these type verdicts.

## What the new hint establishes

`async-read-sites-v1.mjs` and its V2 successor resolve exact published native
`createSignal` and `createMemo` declarations. They enroll zero-argument reads
of local accessors returned by those primitives. The read must occur in a
callback lexically created inside a memo and pass through an admitted returned
expression. Shadowed functions and unresolved callees supply no premise.

The transform adds an observation after the original accessor call. It returns
that call's original value. It observes the native owner and observer without
restoring either, introducing an additional `await`, or scheduling another
promise reaction. TypeScript emission targets ES2022 to retain native async
functions. Composed source maps lead back to the original executed consumer.

The projector recomputes the admitted sites from the current source and checks
the exact source digest, accessor creation, declaration witnesses and spans.
Both the observed owner and observer must be absent. The emitted feedback is:

> This reactive read did not register a dependency for the memo. If the result
> should follow this source, read it during the memo compute and pass the
> captured value into the callback.

Its severity is `info`, its category is `intent-open`, and its static dispatch
status remains `open`. It is not a proven violation. External callback timing,
the path from the callback's result to the memo's result, and user intention
remain open facts. An observed untracked read alone cannot distinguish a stale
result from deliberate inspection.

Native runtime diagnostics, generic exceptions and existing lifetime feedback
remain separate inputs. The evaluation maps the new information to an authored
async-read expectation; it does not assign a production finding kind or reuse
the previous original-source native analysis as new detector evidence.

## Improvements made after the first challenge

V1 misses reads returned inside arrays and objects, an immutable accessor alias,
and a read passed to a promise resolver. It also hints on a callback result
assigned to an unused inspection variable.

V2 follows ordinary object and array return expressions, transparent TypeScript
wrappers, and finite immutable accessor aliases. Exact immutable aliases of
native memo and untrack functions work too. Namespace calls require the actual
namespace import receiver. Deferred object getters, mutable aliases, spread
return structures and unknown argument flow remain unsupported.

V2 suppresses callbacks whose immediate call result is discarded or bound to
an unused constant. This is a conservative selection policy, not proof that a
package discards every callback result. It removes the original unwanted hint
and closes four of the five target misses. The promise-resolver target remains
silent. Original V1 modules, failures and observations are preserved.

## Limits exposed by the further population

The sealed V2 detector handles nested Query results, Floating UI middleware
data, Neverthrow accessor-alias chains, a named RxJS reader, and Valibot results
with transparent TypeScript wrappers. It still misses these three targets:

| Case | Remaining missing fact |
| --- | --- |
| Read passed to `lodash.identity` | Exact external argument-to-result behavior |
| Read through a `let` accessor alias that is never reassigned | A supported proof that this mutable binding retains its target |
| Read passed to `Promise.resolve` | Exact builtin argument-to-result behavior |

One correct-use control produces an unwanted hint: Lodash returns the read into
an inspection variable, that variable is used for debugging, and the memo
deliberately resolves to 9. The unused-variable policy cannot suppress this
referenced inspection value. The observed read-context fact is true, but it
is not useful feedback for the authored intent. It remains in the noise count.

Further open paths include cross-file accessor creation, accessors returned by
packages, callbacks created outside the memo, reads with a restored owner but
no observer, arbitrary argument flow, and paths never executed. Lexically
visible exact native `untrack` suppresses hints; explicit intent through more
remote callback paths is not comprehensively modeled. Source-epoch unit tests
refuse changed-source observations, but full hot-reload integration is untested.

## Independent audits and behavior preservation

`async-read-audit-v1.mjs` imports neither the selector nor the projector. It
resolves the original read calls, exact native creators, immutable aliases,
memo/callback spans, published declaration digests and mapped consumer frames.
It checks the absent owner/observer context and informational status of
**20 retained hint witnesses**, including the unwanted inspection hint. It
does not prove package value flow or user intent. Five typing exclusions are
also checked independently.

Parity checks compare the original six against their previous uninstrumented
execution and all 32 challenge consumers against a separate plain execution.
Source, labels, real types, package bytes, displayed behavior, callback counts,
native diagnostic deliveries and caught exceptions agree in both comparisons.
This supports behavioral preservation on these consumers, not a universal
claim that instrumentation cannot perturb execution. The further population
has no separate plain parity run.

The original six browser run takes 7.7 seconds, the adapted 32-consumer run
35.3 seconds, and the further 27-consumer run 30.1 seconds. These are browser
experiment batch times, not editor latency, startup benchmarks or overhead
ratios. There is no production performance claim.

## Freeze chronology

The first two import-based seals omit a browser module loaded by a literal path.
Those studies retain `detectorFrozenBeforePopulation: false`. Their browser
profiles are authenticated before and after execution, but the original seal
is incomplete. The adapted replay is identified independently of that weakness.

Before the further population is authored, a third seal captures all 254
existing prototype modules and the oracle support input, totaling 255 files. The further study
reports `detectorFrozenBeforePopulation: true`. Every used profile input is
covered by that earlier seal. The new independent auditor is outside the
detector and was added later. No historical freeze is rewritten.

## Reproduction and verification

Retained input artifacts live under `rust/target/async-read-*`. Evaluators refuse
an existing output path. Reproduce the final fresh score and independent audit
with new output names:

```sh
node benchmarks/reviewed-package-models/async-read-study-v2.mjs \
  rust/target/async-read-transfer-preflight-v1/population.json \
  rust/target/async-read-transfer-browser-v1/browser/results.json \
  rust/target/async-read-transfer-replay.json

node benchmarks/reviewed-package-models/async-read-audit-v1.mjs \
  rust/target/async-read-transfer-preflight-v1/population.json \
  rust/target/async-read-transfer-browser-v1/browser/results.json \
  rust/target/async-read-transfer-replay.json \
  rust/target/async-read-transfer-replay-audit.json
```

`async-read-browser-run-v2.mjs` accepts the case module, feedback bridge, fresh
output directory and Chromium executable. Use `async-read-transfer-cases-v1.mjs`
and `family-matrix-feedback-v2.mjs` to execute the retained further consumers.
Real retained installs and browser tooling are required. Fresh preflight uses
the existing `snapshot-challenge-prepare-v2.mjs` and a sealed detector profile.
No reproduction grants contract or certification authority.

Verification passes:

- **194/194 prototype tests**, including 27 new async-read tests; no skips.
- Syntax checks for **256 prototype modules**.
- Authentication of **263 unique pins across 14 historical detector seals**.
- Three independent hint audits and two unchanged-consumer parity checks.
- `make verify-fast`: producer freshness, Rust formatting and pinned workspace
  Clippy; schema parsing, dialect-manifest validation and `git diff --check`.

Full `make verify`, production coverage, ownership, contract corpus and
certification gates are deferred for this isolated research prototype. No
production Rust source, bundled contract, public manifest, schema or finding
snapshot changes. Generated observations, freezes, audits and logs remain in
`rust/target`; research modules, this report, README and the precision backlog
are the source changes.

## Implication for the package-feedback system

The experiments support a shared feedback core: exact static facts, actual
runtime diagnostics, and optional observed-read information. Small exact
summaries can address external value-flow gaps where a semantic proof is
available. Whole-package certification need not precede every useful message.

The next evidence should come from real applications, with review of message
usefulness, noise, missed paths and development overhead. Further authored
perfect scores would not establish those product properties. Earlier
intentional-snapshot noise, escaped-receiver misses and borrowed-child misses
from other populations remain open; this slice does not resolve them.
