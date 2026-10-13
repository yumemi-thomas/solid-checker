# Source revisions and live reload feedback

## Result

The prototype now keeps feedback attached to the recorded analysis input
revision across actual Vite edits and automatic full reloads. It rejects old
observations and regenerates the consumer transform so current feedback can
resume. This adds an integration boundary beyond the earlier offline studies'
authenticated, unchanged populations.

The final profile runs **77 revision stages across 11 authored consumers** of
three retained published packages. Stages repeat updates and edits of the same
consumer; they are not 77 independent application defects.

| Population | Stale-result stages with hints | Quiet working stages | Typing exclusions | Plain comparisons |
| --- | --- | --- | --- | --- |
| Earlier 42-stage challenge, adapted final profile | 15/15 | 21/21 | 6 | 42 |
| Fresh 35-stage challenge, final detector frozen before authoring | 10/15 | 15/15 | 5 | 35 |
| Combined | 25/30 | 36/36 | 11 | 77 |

The five fresh misses are five revisions of one async consumer whose native
read happens after an await. The synchronous call scope ends before that read.
Namespace and cross-file re-export consumers retain all ten expected hints.
All working stages update correctly; all target stages remain demonstrably
stale. There are no page errors or harness failures.

All **77** plain/observed comparisons preserve displayed values, helper and
consumer source hashes, published typing results, caught errors and native
diagnostic deliveries. There are **66 automatic full reloads** in the observed
runs. Independent audits check **30 observations**, **five suppressions**, and
the rejection of **90 nonempty old-observation batches**.

## The source boundary that was missing

Direct imported calls already carry the imported declaration's source hash.
A stable consumer alias can instead carry a declaration in the unchanged
consumer file while its imported function body changes. Looking at that
consumer site alone is insufficient to bind the new source model to an old
execution.

The live challenge demonstrates this with a map, set and native controlled
signal. Initially an aliased helper returns the actual reactive value. After
the helper changes to discard the read and return 9, the original consumer
bytes and alias declaration stay identical. Applying the standalone V9
projector to the initial observation with the new program incorrectly
suppresses that old observation as a constant result. The fresh re-export
consumer demonstrates the same boundary through another source file.

This does not invalidate the earlier offline studies: their runners already
refuse changed source inputs. It exposes the extra boundary needed when a
projector is reused in an editor or a dev server.

`project-read-session-v2.mjs` issues an in-memory revision containing a random
session identity, program generation, invalidation count and digest of its
recorded inputs. The record includes source and declaration reads, configuration
and inherited configuration, resolution misses, file/directory existence,
realpaths, included-file listings and directories used for type resolution.
Missing text is represented separately from actual text containing a sentinel.
Inputs are revalidated after program construction and during feedback projection.

Only the currently issued program retains authority in that session. Building
a replacement program or explicit dev-server invalidation retires the earlier
revision. Exposed audit records are copies; old programs are removed from the
issuance registry rather than retained indefinitely. A revision from another
session, an unissued revision or an unversioned event cannot be projected as
current feedback.

Combined projector V10 accepts the session, original consumer path and source,
and events. It checks the issued revision before invoking the unchanged V9
source/read projector and checks it again afterward. The extra revision field
is removed only after this check so the existing exact consumer witness still
needs to match. Rejected events produce explicit open records; they do not
acquire either current hints or current constant-result suppressions.

The V12 transform embeds the issued revision in each original candidate's
metadata. Runtime V4 includes that revision in its observation-coalescing key.
The earlier collector merges two reads from the same source site and native
node across revisions; the focused reproducer observes one event with two
occurrences. V4 retains two events while still coalescing repeated reads within
one revision. Hook V4 uses the same audited rc.9 native facts with this collector.

## Cache integration discovered by execution

The first live profile correctly retires old observations but only catches
**9/15** current target stages. Vite automatically reloads after a helper edit
while reusing the unchanged consumer transform. That transform still embeds the
retired revision, so current reads are correctly refused too. Retiring analysis
authority alone does not renew the served instrumentation.

Transform V13 also invalidates cached consumer modules through the installed
Vite module graph. This restores helper-reload feedback, but the observed run
gives **14/15** targets. Two delayed startup events for `index.html` and the
observer entry script retire a valid set-consumer revision even though neither
changes its analysis inputs.

Transform V14 limits this cache action to an explicitly recorded input path or
a changed recorded input set, including a changed included-file listing.
It then retires the session and invalidates recorded consumer module transforms
before the next serve. Unrelated startup events keep a matching revision active.
The unchanged 42-stage challenge reaches **15/15** targets and **21/21** quiet
working stages. The fresh challenge uses this same frozen profile.

All initial and intermediate profiles, seals, sources and observations are
retained unchanged. The later scores are adapted results for the earlier
challenge; only the namespace, re-export and delayed-read population was
authored after the final detector freeze.

## Challenge scope and actual types

The retained packages are `@solid-primitives/map@1.0.0-next.2`,
`@solid-primitives/set@1.0.0-next.2` and
`@solid-primitives/controlled-signal@1.0.0-next.3`, with the audited Solid rc.9
runtime. The map/set paths exercise the package observer-shortcut channel;
the controlled signal exercises an actual native reader. Helpers and consumer
source are explicitly authored research code.

Each consumer undergoes seven stages: initial execution, a constant helper,
reversion, a helper typing error, recovery, a consumer comment edit and a
configuration edit. Paired working consumers capture their value during memo
computation. The fresh population adds direct namespace dispatch, a cross-file
re-export followed by a local constant alias, and a read delayed until after
an await. The delayed consumer is kept as a miss rather than being relabeled.

Every served stage checks a complete TypeScript program against the actual
retained published declarations. All eleven invalid stages produce `TS2322`
for assigning a number to a boolean in the helper and receive no checker hint
or suppression. TypeScript owns that error. Six explicit `tsc --noEmit` runs
give four clean programs and two expected `TS2322` results. No typing stub is
broadened or substituted, and no network/package installation is needed.

## Independent audit and verification

Audit V3 imports no detector, selector, transform, session or projector. It
authenticates sealed module and package inputs, source variants, complete
recorded input manifests and their digests, issued transform correspondence,
mapped consumer/native/package frames, current event revision identities,
each nonempty retired batch, and plain browser behavior. Original shared
semantic models remain research premises; this is an independent revision and
frame audit, not a new certification of all native semantics.

The initial audit refuses JSON-serialized optional directory-listing arguments:
undefined arguments become null in JSON. Audit V2 restores their optional-argument
meaning. It then refuses typing-error stages because it requires every open
record to be a retired-revision refusal; TypeScript legitimately adds its own
open record. V3 requires a matching refusal for every original event instead.
Both failed attempts remain available and wrote no successful audit report.

Verification passes:

- **397/397** prototype tests, including 19 source-revision checks and three
  cache-boundary checks, with no skips.
- **382** benchmark module syntax checks.
- **21** historical/current seals authenticating **382** distinct pins unchanged.
- `make verify-fast`: matching producer reuse, Rust formatting and
  certification/probe-pinned workspace Clippy.
- Schema parsing, dialect manifest validation and whitespace checks.

The final local observed runs take 29.86 seconds for 42 stages and 23.09 seconds
for 35 stages. Mean recorded program-build time is roughly 140–170 ms per build
for these small projects. Those measurements include this experiment's repeated
projection and old-batch challenges; they do not establish editor latency or
large-project overhead.

Full `make verify`, production coverage/ownership, contract corpus and
certification gates are deferred for this isolated research slice. No production
Rust rule, public contract, manifest, fixture stub or finding snapshot changes.
Generated inputs, observations, comparisons and audits stay under `rust/target/`.

## Limits and next work

Hints remain `info`, `intent-open`, `staticDispatch:open`, `authority:false`
and `certification:false`. This is positive evidence for current development
feedback across package surfaces; every package and every rule remain unproven.

These are automatic full reloads with the Solid plugin's hot mode disabled.
State-preserving HMR, late callback delivery across a retained runtime,
production editor integration, native-store reloads, server execution and
source changes during concurrent module serving remain unverified. The input
revision covers the recorded TypeScript program; executed source outside that
program still requires its own package/native witness. Runtime trace buffers,
large-project invalidation cost, dependency prebundling and ordinary-app
precision also remain open. The raw-source loader still disables dependency
discovery for this observation profile.

The new delayed-read consumer confirms that revision freshness does not solve
asynchronous attribution. That is the next concrete detection task, alongside
applying this revision/cache boundary in real application sessions.

## Primary artifacts and reproduction

Use fresh output paths:

```sh
node benchmarks/reviewed-package-models/feedback-revision-browser-v3.mjs \
  benchmarks/reviewed-package-models/feedback-revision-cases-v2.mjs \
  rust/target/project-feedback-revision-detector-freeze-v3.json \
  rust/target/<fresh-browser-directory> '<browser executable>' reads

node benchmarks/reviewed-package-models/feedback-revision-audit-v3.mjs \
  benchmarks/reviewed-package-models/feedback-revision-cases-v2.mjs \
  rust/target/project-feedback-revision-fresh-browser-reads-v1/results.json \
  rust/target/project-feedback-revision-fresh-browser-plain-v1/results.json \
  rust/target/<fresh-audit>.json
```

The first command can also run with `plain` for the behavior comparison.
Case modification times distinguish adapted populations from cases authored
after a detector freeze; the module seal and before/after file hashes still
authenticate their bytes.

Primary artifacts:

- `rust/target/project-feedback-revision-detector-freeze-v1.json`, `-v2.json`, `-v3.json`
- `rust/target/project-feedback-revision-browser-reads-v1/results.json`, `-v2/results.json`, `-v3/results.json`
- `rust/target/project-feedback-revision-browser-plain-v1/results.json`
- `rust/target/project-feedback-revision-fresh-browser-reads-v1/results.json`
- `rust/target/project-feedback-revision-fresh-browser-plain-v1/results.json`
- `rust/target/project-feedback-revision-replay-audit-v3.json`
- `rust/target/project-feedback-revision-fresh-audit-v3.json`
- `rust/target/project-feedback-revision-combined-summary-v1.json`
- `rust/target/project-feedback-revision-tsc-v1/results.json`
- `rust/target/project-feedback-revision-unit-v2.log`
- `rust/target/project-feedback-revision-cache-tests-v1.log`
- `rust/target/project-feedback-revision-all-tests-v2.log`
- `rust/target/project-feedback-revision-verify-fast-v1.log`
- `rust/target/project-feedback-revision-syntax-v1.json`
- `rust/target/project-feedback-revision-historical-seals-v1.json`
