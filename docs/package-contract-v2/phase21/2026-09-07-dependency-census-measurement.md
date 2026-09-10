# Creates census: receipt composition and remaining opportunities, 2026-09-07

Follow-up: [ADR 0053's full measurement](2026-09-07-literal-result-census-measurement.md)
closes six SVG helper-result candidates, taking 468 → 462. The inventory
below records the preceding dependency-composition run.

The current full 418-probe measurement has **468 withheld closure candidates:
448 creates and 20 returns**. Of creates, **411 are census refusals and 37
are uncompleted vetoes**. Certification still has **368 certified / 30
refused rows**, with 20 probes not advanced to certification. These units
must not be conflated: a certified row can contain withheld candidates.

Compared with the preceding protocol-35 run, receipt composition closes
**32 candidates**, taking 500 to 468. Including ADR 0051's six earlier
closures, the supplied 506 baseline has fallen by **38**. No candidates were
added. All 418 probe identities, installedVersions fields, and certification
statuses match the preceding full run. Every comparison preserves duplicate
candidate occurrences across graph nodes and roots.

[The evidence JSON](2026-09-07-dependency-census-measurement.json) contains every
remaining occurrence, every refused and unattempted row, the complete audit
manifest and hashes, all removals and changed reasons, and the 32 accepted
mains with their exact SHA-256-bound receipt payloads. The raw report and
binary digests are recorded there. Historical analysis and precise source
locations remain in [the original ranking](2026-09-07-creates-opportunity-ranking.md).

## Implemented decision and measured shortfall

[ADR 0052](../../adr/0052-receipt-bound-dependency-census.md) implements the
user-authorized recommendation: compose authenticated dependency receipts
during census finalization. External behavior remains contract-driven.
There is no remaining human policy decision for F.

The first dependency-refusal class fell **48 → 10**, but candidate gain was
**32**, not 38. Six stagger candidates now refuse on the returned
`easingFunction` binding at
`motion-dom/dist/es/utils/stagger.mjs:635..649`, called at `706..738`.
Their former first blocker was `easingDefinitionToFunction`. This raises E's
measured returned-callable class **46 → 52**. All other remaining first
refusals are unchanged after normalizing temporary source roots.

| Explicitly closed package/export/domain | Candidate occurrences |
| --- | ---: |
| motion-dom/defaultOffset/creates | 6 |
| motion-dom/fillOffset/creates | 6 |
| motion-dom/isHTMLElement/creates | 6 |
| motion-dom/isSVGElement/creates | 6 |
| motion-dom/isSVGSVGElement/creates | 6 |
| @solid-primitives/refs/mergeRefs/creates | 2 |

For each closure the accepted main explicitly states closed creates with an
empty list, and its receipt's mainDigest equals that main's SHA-256. The
ordinary-analysis stage also authenticated and selected the accepted catalog.
A missing refusal was not used as evidence of acceptance.

The remaining ten F cases are six createSignal calls, two getOwner calls,
and two createMemo calls. The selected solid-js accepted mains expose
callable shape but an empty call summary for these exports: **they do not
state closed empty creates**. That absence is why this slice cannot close
them. The earlier estimate that eight were intrinsically honest creation
calls was not established by the census measurement and should not be used
as a semantic conclusion based on API names.

## Ranked remaining work

Counts are measured first blockers, except C's explicit unconfirmed scope.
Estimated gains are planning ranges for a bounded next slice, not measured
closures, and must not be added together.

| Rank | Opportunity and measured count | Precisely required premise and receipt binding | What still refuses | Estimated candidate gain |
| --- | --- | --- | --- | --- |
| 1 | B: local helper result, 24 | Every completion of the exact helper, under the caller's bound argument premises, returns an enumerated data-only literal derivation. Name the caller form and position, call, helper and return spans, aliases/writes, premise identity and helper transcript digest. | Unexplained writes, accessors/proxies, mixed or incomplete return sets, recursion without a complete derivation, external and host results. | 6–12. The SVG helper's two returns use the same locally allocated object; six is the first target. The other six require a separate recursive-context proof. |
| 2 | E: returned callable, 52 (was 46) | The initializer's exact factory and every returned callable are identified, together with each capture's source for that invocation. Bind the initializer, factory/body transcripts, complete return set, capture-to-input mapping and all resulting execution censuses. | Unknown factories, mutable bindings, unexplained captures, hidden callback execution, multiple return targets without complete coverage. The six newly exposed stagger cases belong here. | 8–20 initially. The larger count does not establish more complete-body closures; factories and captured environments differ. |
| 3 | A: remaining arithmetic-chain slice, 50 within 59 binary-coercion first refusals | Carry a stated result-type premise from the exact helper transcript into the exact downstream argument/form; consumer binds both calls, helper completion, reaching value and premise echoes. | Unknown operands, written joins, nonprimitive completions and uncovered coercions. The original poisoning reconstruction was refuted; A0's explicit never slice is already complete. | 6–18 for a measured homogeneous chain, not the original 30–50 claim. |
| 4 | J: 27 non-unique signatures, plus 3 missing-body cases in the 30 incomplete transcripts | Enumerate the complete callable signature/implementation set and prove every relevant body and veto under its exact entry premises. Receipt names overload set, declaration/body identities and coverage. | Missing implementations, ambiguous bodies and TS-source execution. Merely choosing one signature is insufficient. | 0 in isolation on the source-condition lane; 9–27 possible census clearances before the execution blocker. |
| 5 | D: 52 missing function-like nodes = 25 construction + 27 TS function/method calls | Separate a construction execution model covering constructor, initializers, superclass and invoked members from TS-source body execution with an authenticated output-neutral transform. Name exact class/member spans, dispatch, source/output identities and all body witnesses. | Unknown classes, dynamic dispatch, inherited effects without evidence, unsupported transforms and veto execution. A call creates claim cannot cover new. | 0–12 initially from a bounded construction slice; TS source shares J/I's execution limitation. |
| 6 | G: nested callable parameters, 44 | State the value-flow proof connecting each nested invocation parameter to a particular external caller input, and distinguish inputs from this artifact's callbacks. Receipt binds invocation, parameter, originating input and execution timing/ownership. | Module-owned or unexplained callbacks, rewritten parameters, hidden invocations and unproven forwarding. | 0 under the current reviewed premise; 20–28 remains an unprototyped corvu input-flow hypothesis. |
| 7 | I: 36 scoped-condition veto failures, zero census refusals | The exact export condition is accepted as an interpreter argument, resolution selects the same authenticated artifact, and the pinned executor runs that TS source with a bound transform. Receipt names condition, resolution, runtime/source identity and completed veto. | TS under node_modules without a reviewed executor, resolution changes, missing transforms. | 0 for grammar support alone; up to 36 require the executor too. |
| 8 | H: six host computed-style accesses | A reviewed host contract states the actual receiver and computed-read behavior; bind host operation, receiver provenance, property evaluation and completed host-capable veto. | Type-only receiver claims, unknown keys/receivers and absent host execution. | 0 from a typing-only patch; at most six with both host premise and execution support. |
| 9 | C: zero confirmed current direct first blockers | Identify an actual recorded form whose exact reviewed member returns a fresh array; bind library declaration/version, dispatch, call result and receiver provenance at the later use. | Historical match/split/keyframe examples do not establish a current whole-body opportunity. | 0 budgeted until a recorded complete blocker set is found; the historical 18 is unconfirmed. |
| Residual | F: ten external calls remain; 32 closures implemented | The selected dependency export's receipt must explicitly close the exact empty creates claim. Exact declaration source/span, artifact case, package, claim ID and child receipt digest are bound; finalizer compares the obligation root with its discharge. | The current createSignal/createMemo/getOwner summaries lack that affirmative closure. Unknown/nonempty claims, missing/mismatched receipts and constructors remain refused. | 0 from further composition relaxation. A new reviewed child claim is a separate prerequisite. |

For B, source inspection confirms that the HTML scrape helper initializes
`newValues = {}` and returns that binding on both completion paths; its SVG
caller later writes `newValues[targetKey]`. This supports the six-candidate
prototype target, but is not yet a complete return-shape proof. The other B
families include host selection and array-library results, recursive mixed
completions, and dynamically selected parsers. No closure gain is credited
to source inspection alone.

The eight written-callee-binding refusals and the additional written
parameter joins are not fixed by A's result typing. They need an explicit
complete reaching-definition set, including entry values and every write.
The residual form classes below also remain independently binding; clearing
one does not authorize ignoring a body's next form.

## Exhaustive first-refusal census

The following disjoint categories cover all 468 remaining occurrences. The
opportunity rows above intentionally overlap these categories and are not
another additive partition.

| First refusal | Occurrences |
| --- | ---: |
| creates/census/form/coercion/BinaryExpression | 59 |
| creates/census/returned-callable | 52 |
| creates/census/missing-function-node | 52 |
| creates/census/form/property-access-unknown-accessor/PropertyAccessExpression | 46 |
| creates/census/nested-callable-parameter | 44 |
| creates/veto/scoped-condition | 36 |
| creates/census/form/property-access-unknown-accessor/ElementAccessExpression | 34 |
| creates/census/incomplete-transcript | 30 |
| creates/census/form/property-access-unknown-accessor/SpreadAssignment | 29 |
| returns/dependency | 20 |
| creates/census/form/iteration-protocol/SpreadElement | 10 |
| creates/census/dependency-terminator | 10 |
| creates/census/form/instanceof/BinaryExpression | 9 |
| creates/census/written-callee-binding | 8 |
| creates/census/form/coercion/TemplateExpression | 7 |
| creates/census/form/coercion/PrefixUnaryExpression | 6 |
| creates/census/form/property-access-unknown-accessor/BindingElement | 6 |
| creates/census/form/iteration-protocol/ArrayBindingPattern | 4 |
| creates/census/form/await-then/AwaitExpression | 3 |
| creates/census/library-reference-invoker | 2 |
| creates/veto/TS-under-node_modules | 1 |

## Verification and scope

The new packed graph regression passed all four scenarios: exact aliased
child closure; open child; missing completed veto; different export with an
unknown claim. Existing exact-receipt composition tests passed, including
transplantation, stale epoch and missing-claim controls. Workspace all-target
Clippy passed after the internal evidence structures changed.

Full `make verify` passed with exit 0, **TOTAL 197.93 seconds**, and no
`FAILED during step` marker. It included the armed Rust/process tests,
Go race tests, coverage, ownership, contract corpus and TypeScript oracle.
The complete live ecosystem run covered all 418 probes. No additional
fixture snapshots or public contract artifacts changed for ADR 0052.
Protocol remains 35. No commit was made and nothing was pushed.

This completes the remaining-opportunity investigation and the approved F
implementation. B–E and G–J are ranked future semantic work, not claimed as
implemented or silently treated as accepted. Their required facts and
execution prerequisites are stated above.
