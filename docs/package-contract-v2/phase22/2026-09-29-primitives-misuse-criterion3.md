# `@solid-primitives` checkpoint, criterion 3: what misuse reports today

Measured 2026-09-29 at `c97d7085` (the lead tip `a595bd0d` plus the ADR 0158
soundness pin, which changes no finding), with the release binary. Every number
is **measured** unless it is marked otherwise. The ledger is
[`fixtures/primitives-misuse/cases.json`](../../../fixtures/primitives-misuse/cases.json).
It was run by `bun scripts/primitives-checkpoint.mjs --misuse`, unchanged. The
checkpoint's definition is in
[`2026-09-28-solid-primitives-checkpoint.md`](2026-09-28-solid-primitives-checkpoint.md).

Criterion 3: every export with a misuse path has a ledger case that reports the
expected rule on the misuse and nothing on the correct use, against the real
published package, with `tsc` silent on both, in every host the case names.

Update 2026-09-30, consumer slice `5381c7c8`: gap G's published
`utils.access(count)` case now reports one `SC1001` **uncertifiable** finding
in each host, and its JSX twin stays clean. Both twins pass `tsc` against the
real published typings. The accepted contract states a possible inline
invocation, so it does not justify a proven violation; a fixture-only
guaranteed-invocation control does. The project-helper path and ordinary
callback-body invocation cardinality still need separate review. The counts
below remain the historical measurement; the full ledger is being rerun
after integrated certification and tier regeneration.

## Headline

| | value |
| --- | ---: |
| ledger cases | 121 (was 1), in 60 of the 97 corpus packages |
| app sites those 60 packages cover | 287 of 292 `@solid-primitives` app sites (not `jsx-tokenizer` 3, `drag-drop` 1, `context` 1) |
| cases `tsc` reports on, misuse or twin, dropped | **0**: all 242 files are type-clean against the published typings |
| misuse twin, host free: right finding | 22 (18 %) |
| misuse twin: SC9005 uncertifiable only | 91 (75 %) |
| misuse twin: a different finding | 6 (3 `reactive-dispatch-unresolved`, 3 `reactive-source-uncaptured`) |
| misuse twin: the right rule, only uncertifiable | 1 |
| misuse twin: silent | 1 (`utils` `access`) |
| cases whose correct twin is also clean, so the case reports correctly | **1** (`utils` `createMicrotask`, every host) |
| packages at criterion 3 | **4 of 97**, unchanged: `animation`, `event-dispatcher`, `event-props`, `platform`, which have no misuse path |
| false positives in an existing rule | **1**: SC2001 on a write the callee defers (repro below) |

**The rules are mostly not what fails. The contracts are.** 91 of 121 misuse
twins stop at SC9005, and 119 of 121 correct twins carry it. SC9005 on the
twin fails a case as surely as a missed finding does.

25 cases had every contract they touch admitted. 17 of those report the right
rule on the misuse. The other 8:

- 6 stop at a domain the admitted contract leaves open (gap D);
- `timer` `createTimer` in a leaf owner is gap E;
- `utils` `access` by reference is gap G.

## Method

**Which cases.** Packages were taken in order of app demand
([baseline](2026-09-28-app-import-metric-baseline.md), sites per package:
`resize-observer` 59, `event-listener` 53, `utils` 42, `raf` 39, …). Then every
other package with a contract-stated misuse path (`misuse[].source` accepted or
proposed in the lead's checkpoint report) was covered where the path's
signature took simple arguments.

Each case is one of four typical misuses, each with a correct-use twin:

- **Outside an owner.** A creator is called at module scope (`missing-owner`),
  or from a `createEffect` apply callback, which runs with no owner. The twin
  runs the same call in a `createRoot`, or in the component body.
- **Untracked read.** A returned accessor, or a reactive object or collection,
  is read once at a component's top level (`strict-read-untracked`). The twin
  reads it in JSX.
- **Reactive argument read.** The export reads its reactive argument
  synchronously. It is called at a component's top level
  (`strict-read-untracked`), and the twin calls it in JSX.
- **Wrong phase.**
  - A creator is called inside `createTrackedEffect`, a leaf owner
    (`leaf-owner-forbidden-call`).
  - A signal is written inside a callback the export runs tracked
    (`reactive-write-in-owned-scope`).

  In both, the twin does the same thing legally.

**Dropping a returned cleanup has no case.** No rule in the catalog reports it:
there is no disposal rule. The ledger schema requires an expected rule, so
these misuses are recorded as a gap and not as cases. The measured exports
that return a cleanup include `makeEventListener` (22 app sites),
`makeEventListenerStack`, `makeTimer`, `makeBodyCursor`, `makeElementCursor`,
`makeConnectivityListener`, `makeNetworkInformation`, `makeMediaQueryListener`,
`makeActiveElementListener`, `makePageLeave`, `makeSpring` and `createAbortable`.

**`tsc`.** The runner compiles both twins with the oracle's options, strict,
against the case's own install. On all 242 files the error diagnostics are
empty. Nothing needed dropping under the absolute rule. That `tsc` was
reporting was checked by hand: a deliberate `number`-to-`string` assignment in
a case directory reports TS2322.

**The install** is the runner's: the pinned package version, with `solid-js`
and `@solidjs/web` at the head probe's runtime, rc.9, for all 97 packages.

## Per package

App sites come from the app-import baseline. "Right", "SC9005", "Other" and
"Silent" count the host-free misuse twin. "Gap" names the letters of the
ranking below.

| package | app sites | cases | right | SC9005 | other | silent | gap |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |
| resize-observer | 59 | 7 | 0 | 6 | 1 | 0 | B, E |
| event-listener | 53 | 5 | 0 | 4 | 1 | 0 | B, E |
| utils | 42 | 3 | 2 | 0 | 0 | 1 | D, G |
| raf | 39 | 4 | 0 | 4 | 0 | 0 | A |
| pointer | 7 | 2 | 0 | 2 | 0 | 0 | A |
| scheduled | 7 | 1 | 1 | 0 | 0 | 0 | D |
| timer | 7 | 7 | 6 | 0 | 1 | 0 | D, E, I |
| marker | 6 | 1 | 1 | 0 | 0 | 0 | D |
| storage | 6 | 2 | 0 | 2 | 0 | 0 | D |
| event-bus | 5 | 2 | 0 | 2 | 0 | 0 | A, D |
| set | 5 | 2 | 0 | 2 | 0 | 0 | A, D |
| clipboard | 4 | 2 | 0 | 2 | 0 | 0 | A |
| pagination | 4 | 2 | 0 | 1 | 1 | 0 | A, F, I |
| scroll | 4 | 3 | 0 | 3 | 0 | 0 | B |
| cursor | 3 | 3 | 1 | 2 | 0 | 0 | A, I |
| keyboard | 3 | 3 | 0 | 3 | 0 | 0 | B |
| props | 3 | 1 | 0 | 1 | 0 | 0 | B |
| deep | 2 | 1 | 0 | 1 | 0 | 0 | A |
| fullscreen | 2 | 2 | 0 | 2 | 0 | 0 | A |
| history | 2 | 1 | 0 | 0 | 1 | 0 | A, H, I |
| i18n | 2 | 1 | 0 | 0 | 1 | 0 | B, F, I |
| keyed | 2 | 1 | 0 | 1 | 0 | 0 | A |
| map | 2 | 1 | 0 | 1 | 0 | 0 | B |
| memo | 2 | 3 | 2 | 1 | 0 | 0 | D |
| mutation-observer | 2 | 1 | 0 | 1 | 0 | 0 | A |
| refs | 2 | 2 | 1 | 1 | 0 | 0 | B, I |
| rootless | 2 | 1 | 0 | 1 | 0 | 0 | D |
| upload | 2 | 2 | 0 | 2 | 0 | 0 | B |
| active-element | 1 | 2 | 0 | 2 | 0 | 0 | A |
| connectivity | 1 | 2 | 0 | 2 | 0 | 0 | A |
| media | 1 | 3 | 0 | 3 | 0 | 0 | B |
| page-utilities | 1 | 2 | 0 | 2 | 0 | 0 | A |
| static-store | 1 | 2 | 1 | 1 | 0 | 0 | B, I |
| transition-group | 1 | 1 | 0 | 1 | 0 | 0 | A |
| trigger | 1 | 1 | 1 | 0 | 0 | 0 | D |
| websocket | 1 | 2 | 0 | 2 | 0 | 0 | A |
| a11y | 0 | 2 | 0 | 2 | 0 | 0 | B |
| async | 0 | 1 | 0 | 0 | 1 | 0 | A, F, I |
| audio | 0 | 1 | 0 | 1 | 0 | 0 | A |
| broadcast-channel | 0 | 1 | 0 | 1 | 0 | 0 | A |
| cookies | 0 | 1 | 0 | 1 | 0 | 0 | A |
| date | 0 | 3 | 0 | 3 | 0 | 0 | A |
| devices | 0 | 2 | 0 | 2 | 0 | 0 | A |
| focus | 0 | 1 | 0 | 1 | 0 | 0 | B |
| intersection-observer | 0 | 2 | 0 | 2 | 0 | 0 | A |
| lifecycle | 0 | 2 | 0 | 2 | 0 | 0 | A |
| notification | 0 | 2 | 0 | 2 | 0 | 0 | A |
| orientation | 0 | 2 | 0 | 2 | 0 | 0 | A |
| permission | 0 | 2 | 2 | 0 | 0 | 0 | D |
| promise | 0 | 1 | 1 | 0 | 0 | 0 | A, I |
| range | 0 | 2 | 0 | 2 | 0 | 0 | A |
| script-loader | 0 | 1 | 0 | 1 | 0 | 0 | A |
| selection | 0 | 2 | 0 | 2 | 0 | 0 | A |
| sensors | 0 | 2 | 0 | 2 | 0 | 0 | A |
| signal-builders | 0 | 2 | 1 | 1 | 0 | 0 | A, I |
| spring | 0 | 2 | 0 | 2 | 0 | 0 | A |
| tween | 0 | 2 | 2 | 0 | 0 | 0 | D |
| vibrate | 0 | 1 | 0 | 1 | 0 | 0 | A |
| video | 0 | 2 | 0 | 2 | 0 | 0 | A |
| workers | 0 | 1 | 0 | 1 | 0 | 0 | A |

`utils` passes one case, `createMicrotask`. `memo` and `trigger` also answer SC9005 under `node` where host free and `browser` report.

**No case yet: 37 packages.** In each entry below, `(n/m)` means n exports with
a misuse path and m of them contract-stated.

- Four have no misuse path and pass criterion 3: `animation`,
  `event-dispatcher`, `event-props`, `platform`.
- The other 33 need arguments this pass did not construct:
  - `analytics` (7/1), `bounds` (1/0), `context` (4/0), `controlled-props` (6/0),
    `controlled-signal` (5/1), `destructure` (1/0), `drag-drop` (5/0),
    `favicon` (6/0), `filesystem` (8/5), `flux-store` (3/1);
  - `form` (1/1), `geolocation` (5/2), `gestures` (9/9), `idle` (1/1),
    `input-mask` (4/0), `interaction` (3/3), `jsx-tokenizer` (3/2), `list` (2/1),
    `list-state` (2/2), `masonry` (1/0);
  - `match` (3/0), `mediastream` (4/4), `mouse` (5/1), `mutable` (1/0),
    `presence` (1/0), `queue` (6/3), `share` (2/2), `sortable` (9/2), `sse` (2/2),
    `state-machine` (1/0), `styles` (2/0), `url` (2/0), `virtual` (2/0).

Four of them, `controlled-props`, `drag-drop`, `favicon` and `virtual`, carry
the published defect the checkpoint names: they import `solid-js/web`, which
rc.9 does not export.

## Checker gaps, ranked by packages

A package counts once per gap it shows on any of its cases.

| | gap | packages | what it is |
| --- | --- | ---: | --- |
| A | no accepted contract: the package is not in the compiled-in tier | 37 of the 60 covered | SC9005 "no receipt-accepted contract matches this exact import". Criterion 1 again, seen from the consumer. |
| B | no accepted contract for the installed rc.9 runtime: the tier's bundles were certified on rc.3 | 13 (`resize-observer`, `event-listener`, `a11y`, `focus`, `i18n`, `keyboard`, `map`, `media`, `props`, `refs`, `scroll`, `static-store`, `upload`) | the tier *has* the package, but every bundle's recorded environment names `solid-js`/`@solidjs/web`/signals **rc.3**. The corpus's head probe, and so the misuse install, is rc.9, so ADR 0123's environment admission rejects them. See the note below. |
| D | the admitted contract leaves a needed domain open | 12 | the misuse may still report (`timer`, `memo`, `tween`), but the twin carries SC9005 for the open `reactiveReads`/`ownerRequirements`/`callbacks`. That is criterion 2's wall. |
| I | the correct twin reports a finding other than SC9005 | 10 | 8 packages: `strict-read-untracked` *uncertifiable* on a read inside a callback handed to an uncontracted export. That is fail-closed and follows from A. `timer`: a proven SC1001 on the documented use (below). `history`: the SC2001 false positive. |
| E | a creator called in a leaf owner (`createTrackedEffect`) is reported `reactive-dispatch-unresolved`, never `leaf-owner-forbidden-call` | 3 (`resize-observer`, `event-listener`, `timer`) | SC3001 knows the dialect's owner-attaching primitives only. A package export whose accepted contract states an owner requirement does not feed it, even where the contract is admitted (`timer`). |
| F | an accessor handed to an uncontracted export is `reactive-source-uncaptured` | 3 | fail-closed and correct, given A. Listed because it names a different rule than the case expects. |
| G | an accessor passed **by reference** to a callee that invokes it on the same stack is not reported | 1 (`utils`) | `access(count)` at a component's top level is silent. `access(() => count())` reports SC1001, twice (below). The same holds for a *project* helper `run(count)`, so this is not contract-specific. A missed true positive. |
| H | the right rule, only uncertifiable | 1 (`history`) | follows from A. |

**The one lever is the tier, per environment.** Gap A (37) and gap B (13)
are disjoint, so together they cover 50 of the 60 covered packages. B is cheaper, because those packages already certify. The
checkpoint's criterion 1 counts a package in the tier when any bundle for the
entrypoint exists. It does not check that a bundle's recorded runtime is the
one the corpus installs. So it reports `resize-observer` and `event-listener`
as met while no rc.9 consumer can admit them. Of the 97 corpus packages, the
compiled-in tier has bundles for these runtimes:

| runtime of the tier's bundles | packages |
| --- | ---: |
| rc.9 in every case | 11 |
| rc.9 in some cases | 1 |
| rc.3 only | 17 |
| none | 68 |

Measured by `pkg/contracts/accepted/index.json`'s environments.

## False positive in an existing rule (separate bug)

**SC2001 `reactive-write-in-owned-scope` reports a proven violation for a write
in a callback the callee does not run during the owned scope.** No package is
needed to reproduce it. `tsc` is clean on it. Release binary at `c97d7085`,
host free:

```tsx
import { createSignal } from "solid-js";

// `later` stores the callback and runs it from a timer, never during App.
let pending: (() => void)[] = [];
export function later(callback: () => void) {
  pending.push(callback);
}
setInterval(() => {
  for (const callback of pending.splice(0)) callback();
}, 100);

export default function App() {
  const [count, setCount] = createSignal(0);
  later(() => setCount(1));
  return <p>{count()}</p>;
}
```

The checker reports two findings. The first is correct:
`package-contract-incomplete` (uncertifiable), "callback parameter 0 … of
`later` reaches a call whose execution timing is unknown; this callback cannot
be certified". The second is the false positive:
`reactive-write-in-owned-scope` **violation**, "accessor setter `setCount` is
called inside owned scope App". `setCount` runs only from the interval, where a
write is legal. The checker itself says it cannot know when the callback runs,
yet SC2001 claims a proven violation. SC1001 answers the matching read case
with *uncertifiable*, which is the right answer.

- **The same bug with a returned closure.** `keep(source)` calls `source()` now
  and keeps the closure it returns. `keep(() => () => setCount(1))` in a
  component body reports the same SC2001 violation, although the setter runs
  only from a click handler.
- **In the ledger.** It is what fails `history-createUndoHistory-top-level-read`'s
  correct twin: `createUndoHistory(() => { const v = count(); return () => setCount(v); })`,
  where the returned setter closure runs only from `undo()`.
- **Affected verdicts.** The same rule answers "right finding" on all five
  `write-in-tracked-callback` misuse cases, though none of the callees has an
  admitted contract (`cursor`, `refs`, `static-store`, `promise`,
  `signal-builders`).
  There the verdict happens to agree with the runtime, but it is not proven.

Precision contract: an unproven write position must be uncertifiable, never a
violation.

## Other observations

- **`timer` `createPolled` and `createIntervalCounter`: the correct twin
  reports a proven SC1001.** The message is "reactive accessor … is read
  through createPolled in App, which does not track". The twin is the
  documented use: called in a component body, read in JSX. The cause is in the
  package. `createPolled` seeds its value with `createSignal(depSignal(), …)`,
  an untracked read of its own derived signal during the call. The runtime's
  strict-read check in `@solidjs/signals@2.0.0-rc.9`
  (`dist/dev-shared.js` `read()`) has no exemption for a signal the call
  created. So this is **believed to be a true positive** against the package
  (a dev `STRICT_READ_UNTRACKED` at every call in a component body). It was
  not probed at runtime. These two cases cannot pass criterion 3 while the
  package reads that way.
- **`access(() => count())` reports SC1001 twice**, for one read: "read
  through access in App" and "read directly in rendering function".
- **Copy the install; do not symlink it.** A repro directory whose
  `node_modules` is a symlink to a case install admits no contract (SC9005 "no
  summary"), while a copy (`cp -cR`) admits it. Worth knowing before building a
  repro from a case.

## What would move criterion 3

In order, by packages moved. None of it was done here:

1. **Tier bundles on the rc.9 environment** for the 17 packages certified only
   on rc.3 (gap B). That is regeneration from certifications that already
   exist. Then criterion 1's tier check should compare the bundle's recorded
   runtime with the corpus's install, or it keeps reporting B-packages as met.
2. **Criterion 1 and 2 for the rest** (A, D). Criterion 3 cannot pass on a
   package whose correct twin carries SC9005.
3. **The SC2001 fix** (bug above). It is required before any
   `write-in-tracked-callback` case can pass for the right reason.
4. **Rule gaps E and G**, and a disposal rule for dropped cleanups (12
   exports), which today have no rule to report them at all.

## Reproducing

```sh
make build-checker-release
SOLID_CHECKER_NATIVE_BIN=$PWD/rust/target/release/solid-checker-rust \
SOLID_TYPEFACTS_BIN=$PWD/bin/solid-typefacts \
  bun scripts/primitives-checkpoint.mjs --misuse --json <out>/misuse.json   # about 5 min, network (bun install per case)
bun scripts/primitives-checkpoint.mjs --report <measure.json>,<measure-browser.json>,<measure-node.json> \
  --misuse-results <out>/misuse.json --json <out>/checkpoint.json
```

The per-case messages behind the SC9005 split come from running the release
checker on each case's `rust/target/primitives-checkpoint/misuse/<id>/tsconfig.{misuse,correct}.json`.
Its `analysisContext` "no receipt-accepted contract matches this exact import"
is gap A or B. "unknown-contract-claims:…" is D.
