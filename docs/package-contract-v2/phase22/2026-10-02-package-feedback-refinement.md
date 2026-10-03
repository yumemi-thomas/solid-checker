# Refining the package feedback experiment

The revised corpus reaches **45/45**: matching feedback on all **21 target
patterns**, and silence on all **24 correct-use or intent controls**. The
readonly assignment remains a separate TypeScript exclusion with no checker
feedback.

One consumer changed: its existing snapshot intent is now expressed with
`untrack`. The other 45 records, including the typing exclusion, preserve their
source bytes, roles and expected rule/code identities. The unchanged snapshot
still receives a strict-read warning under the new adapter. With every original
consumer left unchanged, the result is therefore **44/45**.

The score retains the earlier meaning of feedback: source candidates,
preferences, loading advisories and declared lifetime checks count. It is not
45 proven defects or coverage of every package and rule. The previous
[combined-system report](2026-10-02-combined-package-feedback-system.md) and its
observations remain intact.

## Closing the async miss

The earlier return extractor established that `createLazyMemo` returns an
accessor. It did not establish where its supplied callback runs, so the source
adapter could not safely project an after-await diagnostic.

The existing generic `CallbackPaths` tracer supplies that missing fact. In the
installed browser implementation, the package's `calc` argument is called at
`memo/dist/index.js`, offset 3487, inside the callback passed to native
`createMemo`. The tracer follows the exact import alias, local bindings and
forwarded argument. This browser path has a mandatory tracked-compute callback
operation, no conflicting callback phase and no reported source gaps. The
server path supplies no such premise.

`family-phase-specializer.mjs` combines that positive callback-phase premise
with the existing accessor-return premise. It admits only the lowerer's
supported single inline callback form. It does not infer a tracked phase from
a callback's type, API name, target label or expected result. Untracked,
delayed, conditional, mixed-context, shadowed and async-helper counterexamples
do not acquire the premise.

Once the analysis copy places the actual async callback inside a native memo,
the unchanged native checker emits `reactive-read-after-await` for the target
and nothing for its correct-use control. The warning points to the original
`count` read at line 4, column 76. The target still renders `1` after its source
changes to `2`; the control captures the dependency before awaiting and renders
`2`. The declared behavior failure remains independent evidence, not input to
the detector.

The same generic phase fact also adds source warnings for the existing owned
write, tracked `resolve` and tracked `until` targets. Their controls remain
quiet. This raises the number of targets with source-assumption warnings from
five to nine; runtime observations remain available alongside them.

## Restoring exact callback locations

The old lowerer generated memo text containing a copied callback, but anchored
the entire generated operation at the original package call. The native async
finding was correct in the copy, while its mapped location landed outside the
original callback. The projection therefore refused it.

`family-phase-projection.mjs` restores callback provenance using three checks:

1. The generated operation is anchored at an admitted source-model call site.
2. The analysis copy's constructor resolves through TypeScript to the actual
   `solid-js.createMemo` export.
3. Its single callback clone exactly matches the original callback text and
   belongs to that generated operation.

Only a unique match supplies a copied source segment. Ambiguous or absent
provenance stays a gap. Tests cover Unicode offsets, identical callbacks at
different sites and text matches without an operation premise. The native
finding is still projected as a **source-assumption warning**, with
`certification: false`. It is not promoted to a proven original-code violation.

## Making snapshot intent explicit

The original intentional control reads `value()` once and deliberately keeps
the initial value after dispatch. That desired behavior is in the authored
test flow, not in the consumer's reactive operation. The strict-read warning
describes the same non-updating read that is a mistake in the target case.

The revised copy uses the real API:

```ts
import { untrack as explicitSnapshot } from 'solid-js';
const frozen = explicitSnapshot(value);
```

Its update/disposal flow and desired output remain unchanged. Both the native
runtime and source channels become quiet. The detector receives neither the
control label nor a suppression derived from the expected outcome. A separate
replay of the unchanged consumer confirms that its warning is still present.

This is an intent edit, not an automatic inference of user intent. An editor
can explain the warning and offer an explicit snapshot form when that is what
the author wants. It cannot know that preference from an otherwise ordinary
reactive read.

## Checks against overfitting

Seven additional real consumers exercise the new mechanism:

| Consumer | Outcome |
| --- | --- |
| Namespace import, source read after await | Matching source warning; output stays stale |
| Renamed import, source read after await | Matching source warning; output stays stale |
| Unexecuted conditional await before a synchronous read | Quiet; output updates |
| Await inside a separate nested function | Quiet; output updates |
| Explicit untracked read after awaiting | Quiet; intended snapshot stays fixed |
| Explicit snapshot captured before awaiting | Quiet; intended snapshot stays fixed |
| Named async callback passed as an identifier | **Open**; output stays stale without source feedback |

All seven pass actual published typing and run without harness errors. The
first six behave as expected without any export-specific detector. The final
case is a demonstrated limit: this adapter supports inline callbacks, and the
existing lowerer cannot yet carry a named function's callback phase into the
analysis copy. This case is retained as a miss outside the original 45.

The comparison validator checks the intended claim, rather than accepting any
unrelated warning as a success. It rejects dropped or relabeled consumers,
checks source hashes and permits exactly the documented snapshot edit. It
also verifies that the corrected snapshot preserves its original desired
output. Expectations are used only after the source and runtime observations
have been produced.

## Evidence and verification

Retained outputs:

- `rust/target/family-refined-study/results.json`: independent combined feedback,
  46 records, 45 executed consumers, one typing exclusion, no automatic misses
  or noisy controls in the revised corpus.
- `rust/target/family-refined-comparison.json`: claim-matching 45/45 score, the
  sole source edit and all seven additional challenges.
- `rust/target/family-phase-original-intent-static/results.json`: the original
  implicit snapshot still warns.
- `rust/target/family-phase-async-static/results.json`: the first callback-phase
  experiment, retaining the native finding that lacked projected provenance.
- `rust/target/family-phase-async-projected/results.json`: the focused async pair
  with the restored source location.

Both browser source closures are frozen before and after execution. The study
rechecks published typing and package/native input hashes. The earlier four
timer precision observations, including their two harness failures, remain
retained separately and do not inflate the revised score.

Verification completed:

- Fresh pinned debug build through `make build-checker-debug`; Type Facts source
  stamp matched the retained producer.
- Complete revised browser run and original/twin native analysis for every
  type-valid consumer, plus focused async, original-intent and challenge runs.
- 92 prototype tests passed, including five new tests for callback phase,
  conflicting contexts, host separation and source provenance.
- `make verify-fast`, module/config syntax checks, schema validation, dialect
  manifest validation and `git diff --check` passed.

No production Rust rule, accepted contract, schema or finding snapshot changed.
Full certification, fixture coverage/ownership and release verification were
deferred for this isolated prototype. Historical shared modules and successful
observations were preserved; new adapters and reports carry experimental,
non-certification status.

Remaining limits include named callbacks, richer callback argument shapes,
opaque branches and helpers, server/SSR and directive contexts, and all rule
families not exercised by this corpus. The result justifies the demonstrated
improvement while keeping those gaps explicit.
