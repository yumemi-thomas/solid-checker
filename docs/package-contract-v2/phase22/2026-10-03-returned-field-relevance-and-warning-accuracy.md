# Returned field relevance and warning accuracy

## Outcome

Source facts about returned fields reduce noisy informational hints without
removing any previously detected target in these experiments. The two visible
execution-counter controls now stay quiet. Their delayed reads remain recorded.

| Authored population | Target hints before / after | Noisy controls before / after | Quiet controls after |
| --- | ---: | ---: | ---: |
| Earlier 40 consumers | 14/18 → 14/18 | 10/22 → 6/22 | 16/22 |
| Fresh 18 consumers | 6/6 → 6/6 | 12/12 → 4/12 | 8/12 |
| Combined | 20/24 → 20/24 | 22/34 → 10/34 | 24/34 |

The fraction of emitted hints attached to authored targets improves from
47.6% to 66.7% in the combined population. These are selected serial/concurrent
queue consumers, with correlated variants and supplied expected results. This
is a useful precision improvement, not an estimate for real applications or
all packages. Four targets remain missed and ten controls retain noisy hints.

All feedback remains `severity: info`, intent-open, `authority: false` and
`certification: false`. No production warning, proven violation, behavioral
contract or accepted certification artifact changes.

## What the source facts establish

`constant-returned-data-v1.mjs` examines the exact registered async callback
from an already admitted native-read observation. It requires the current
source hash and exact function span. A bounded control-flow analysis must show
the same primitive own-field initializers on every normal body return.

For example:

```ts
read();
recordExecution();
return { value: 9 };
```

The read is real, but it does not change this returned field initializer.
The model also handles equal branches, catch outcomes and finally replacement:

```ts
try { return { value: read() }; }
finally { return { value: 9 }; }
```

It refuses named allocations, aliases, spreads, accessors, methods, nested
objects, computed keys, differing normal outcomes, fallthrough, unknown control
flow, direct built-in eval, Promise adoption and resource-budget exhaustion.
It rejects `then` and `__proto__` fields. Bounds are 256 statements, 64 distinct
completions and 16 fields. Signed zero remains distinct.

This proves a narrow source fact about normal body return. Fulfilled Promise
contents, settlement, object identity, mutation after return and effects remain
open. It is insufficient to classify an application as correct or a replay as
a safe repair.

## Why consumer use also matters

Constant fields cannot justify filtering feedback when the consumer observes
the result object's identity. `memo-result-field-uses-v1.mjs` therefore requires
an exact local const memo-result declaration and checks every written reference
to that TypeScript symbol in the consumer source. All references must be direct
zero-argument accessor calls followed by static reads of modeled fields.

Transparent TypeScript wrappers are supported. Passing or returning the
accessor, storing the result object, aliases, shorthand escapes, computed or
optional access, writes, unknown fields and top-level declarations remain open.
The fresh identity targets deliberately use `identityOf(result())`: their
constant-field callbacks retain feedback and the stale identity assertion fails.

`native-read-feedback-v23.mjs` combines these two source facts with the earlier
runtime/session admission checks. It moves only the matching field-value hints
to a scoped suppression record, preserving the full observation. `notes` plus
the new suppressions must exactly partition `readObservations`.

V24 additionally preserves an upstream refusal for unavailable or malformed
runtime evidence before requesting a live source/revision API. The normal
selection policy and both source models remain unchanged. Eighteen focused
retention/refusal checks pass, including changed revisions.

No benchmark role, expected value or test assertion is given to this selector.
Raw reads and side-effect uncertainty remain available for debugging. The
filter does not say that the read was intended or that effects are correct.

## Frozen policy and browser comparisons

The first constant-field-only policy V22 was preliminary. The consumer-use
guard and V23 were frozen in `warning-accuracy-detector-freeze-v2.json` before
the 18 fresh consumers were authored. That seal preserves 567 JavaScript inputs
and authenticates the preceding seals. The fresh challenge covers returned reads,
different branches, constant branches/catch/finally, discarded reads, named
objects and identity use in both published queue modes.

Browser V27 runs the frozen policy, plain controls and the previous V21 policy
over the same fresh sources. Browser V28 repeats all 18 after the unavailable-
evidence guard. Every primary value, visible output, consumer counter, native
diagnostic, error and typing result agrees across these policy versions and
with the plain run. The final profile's V3 freeze follows the challenge and is
explicitly classified as an adapted validation; the earlier frozen-policy trial
remains the evidence for the fresh selection test.

The earlier 40 consumers are an adapted replay. Their before-policy output is
retained verbatim as `readObservations` by the preceding V21 projector. Four
field-value hints are filtered: two constant-object controls and two visible
execution-counter controls. All 24 raw observations remain available.

Published inputs are unchanged: `@solid-primitives/queue@1.0.0-next.3`,
Solid/signals/web `2.0.0-rc.9`, and TypeScript `5.9.3`. Package closures, real
typing programs, served source spans, exact registration identities, read/entry
frames and session revisions are authenticated by the browser reports.
Only generated isolated consumer copies execute. Remote requests are blocked.

## Independent audits and typing boundary

Browser audit V17 imports no detector or projector. It reconstructs normal
outcomes with `constant-returned-data-audit-v2.mjs` and reconstructs exact
consumer references with `memo-result-field-uses-audit-v1.mjs`. It checks the raw
observation partition and compares application behavior with plain controls.

Audit V1 initially refused a standalone function whose span coincided with the
source-file node. V2 selects the exact requested node kind at the same span.
It does not choose a contained node or a name match. The regression and altered
evidence checks pass. A nested transparent-wrapper consumer also exposed an
ascent error, fixed before the selection-policy freeze.

Focused checks total **92 passing, zero skipped**:

- 36 constant-field model and refusal tests;
- 27 consumer-use and independent-audit tests;
- 11 independent normal-outcome and altered-evidence tests;
- 18 retention, malformed-evidence and revision tests.

A combined real `tsc --noEmit` invocation passes all 116 source files belonging
to the 58 valid consumers. Two additional adapted typing-boundary controls
produce the expected CLI diagnostics, TS2345 and TS2769, with zero projected
notes and zero suppressions. These errors remain TypeScript's responsibility.
The paired typing-boundary browser audit passes. The final adapted/fresh/typing
audits cover 60 plain comparisons; prior-policy and first-policy audits are
additional comparisons over the same authored populations.

The Type Facts stamp, Rust format check, pinned workspace Clippy, schema parse,
dialect manifests and diff whitespace check pass. Full production verification,
coverage, ownership, contract corpus, release and CLI suites are deferred for
this research-only slice. No native analyzer source, fixture snapshot, bundled
contract, compiler pin or production diagnostic changes.

## Remaining limits

The four missed targets are adopted-child and Promise-reaction reads in both
queue modes. Their observations are absent; this filter does not recover them.
Six earlier noisy controls involve caught rejected adoption, caught throwing
`then` getters or never-settling Promises. Four fresh noisy controls use named
objects or object identity. Those paths retain informational hints because the
required value/settlement/identity facts remain open.

The next useful extensions are source facts for exact local returned objects
and explicit consumer catch outcomes, with mutation and Promise behavior kept
separate. Developer intent and effect relevance still need application evidence.
Ordinary warnings should require stronger defect evidence than these retained
untracked-read observations. Warning accuracy across other packages and real
application populations remains unmeasured.

## Artifacts and reproduction

All generated reports live under `rust/target/`:

- `warning-accuracy-combined-summary-v1.json` contains paired counts and pins;
- `warning-accuracy-replay-{reads,plain}-v2/results.json` and replay audit V2;
- `warning-accuracy-fresh-reads-v1/results.json` records the first frozen trial;
- `warning-accuracy-fresh-{reads-v2,plain-v1,baseline-v1}/results.json` and audits;
- `warning-accuracy-typing-{reads,plain}-v1/results.json` and typing audit V1;
- `warning-accuracy-{published,rejected}-typing-v1/` contains actual CLI evidence;
- `warning-accuracy-detector-freeze-v{1,2,3}.json` preserves profile history;
- `warning-accuracy-handoff-freeze-v1.json` preserves final inputs and earlier
  compiler/source work.

The initial sandboxed browser attempt failed before executing a case. Its
incomplete report and log remain preserved and are excluded from the counts.
Successful browser runs use permission for local headless Chromium and local
fixture servers. Browser V28 takes case module, seal, fresh output directory,
installed Chromium executable and `reads` or `plain`. Audit V17 takes the same
case module, reads report, plain report and fresh JSON path. The summary command
takes a fresh output path and records the saved comparison input hashes.
