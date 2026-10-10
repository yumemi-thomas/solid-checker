# Remaining `creates` census opportunities, 2026-09-07

Current follow-up: the user authorized F's receipt-composition recommendation.
[ADR 0052](../../adr/0052-receipt-bound-dependency-census.md) is implemented;
the [full follow-up measurement](2026-09-07-dependency-census-measurement.md)
confirms **500 → 468** withheld candidates. F's first blockers fall 48 → 10;
32 candidates close and six expose E, raising returned-callable refusals
46 → 52. The policy-decision language and gain estimates below are historical.

Follow-up: [ADR 0051](../../adr/0051-explicit-bottom-type-premise.md) implements
the first bounded slice under protocol 35. Its producer regression and
end-to-end receipt test pass. The subsequent [full 418-probe measurement](2026-09-07-protocol35-census-measurement.md)
confirmed the six-candidate gain: **506 → 500**, with all six `applyBoxDelta`
closures explicitly present in receipt-bound accepted mains. Certified /
refused rows remain 368 / 30. The investigation and baseline below are
retained as recorded.

**Take the explicit `never` case first. Then investigate a local helper's
complete return shape (B) and a returned callable's identity and captures (E).
Do not build the proposed geometry callee-annotation system on the ADR 0049
reconstruction: the diagnostic refutes that reconstruction.**

This is an investigation at HEAD `6b687543`, protocol 34, on
`codex/phase19a-authenticated-proof-policy`. No implementation was retained, no
commit was made, and nothing was pushed. Existing worktree changes to
verification and benchmark tooling were preserved. ADRs 0043–0050, the
precision backlog, accuracy roadmap, and current producer/consumer code were
the starting evidence, not assumed conclusions.

The ranking orders the **next bounded piece of work**, taking account of
likely complete-candidate benefit, cost, and evidence strength. Counts are
measured; gains below are planning estimates, **not measured corpus deltas**.
The one producer prototype and its restoration are documented in
[the reproducible geometry diagnostic](2026-09-07-creates-chain-diagnostic.md).

The corpus basis matters. The checked-in `benchmarks/ecosystem/report.json`
predates the later ADRs: its motion row still withholds 676 candidates. The
retained raw audits reconcile the supplied baseline. There are 404 retained
audit files, including six later utility remeasurements with no withholdings.
Taking the oldest audit per basename gives the coherent 398-row cohort:

| Observation | Count |
| --- | ---: |
| Certified / refused audit rows | 368 / 30 |
| All withheld closure candidates | 506 |
| `creates`, census refused | 449 |
| `creates`, veto could not complete | 37 |
| `returns`, composed from a withheld dependency claim | 20 |
| All withheld **`creates`** candidates | **486** |

[The evidence JSON](2026-09-07-creates-opportunity-evidence.json) preserves all
506 candidate records, normalized reasons, exact artifact/claim/node
identities, hashes and paths of the 18 audits with withholdings, and a digest
of the complete cohort manifest. A record is one candidate occurrence in one
root audit. Shared dependency claims reached from different roots still count
separately, as they do in the supplied baseline. These are diagnostic audits,
not acceptance receipts. No new full-corpus certification run was performed.

| Order | Opportunity | Measured current first blockers | Estimated `creates` candidates no longer withheld from the bounded slice | Cost / confidence |
| --- | --- | ---: | --- | --- |
| 1 | **A0: explicit bottom type in the geometry leaf** — newly isolated from A | 6 | **About 6**; producer mechanism confirmed, final veto not rerun | Low / strongest evidence |
| 2 | **B: what a module-local helper returned** | 24 across four six-record families; two require substantially broader return reasoning | **6–12**, starting with the SVG scrape result | Medium / source-supported |
| 3 | **E: which callable a factory returned** | 46 | **8–20**, after binding captured values as well as the returned body | High / no closure delta measured |
| 4 | **A: return-type facts carried back into the caller's twin** | 56 binary-coercion records, including A0; **50 remain after separating A0** | **6–18** for a narrowly selected arithmetic chain; the original 30–50 is unsupported | High / heterogeneous blockers |
| 5 | **J: non-unique signatures** | **27**, not 30; the other 3 lack an implementation | **0 expected in isolation** on the source-condition lane; potentially 9–27 census clearances before execution blocks | Medium / prerequisite work |
| 6 | **D: constructors and TS-source callees** | 52 = 25 constructions + 27 TS function/method calls | **0–12** initially from construction paths; TS paths face the execution blocker too | High / split into separate designs |
| 7 | **G: nested callable parameter provenance** | 44 = 28 corvu + 8 pipe + 8 callback-stack | **0 under the current reviewed policy**; 20–28 is a hypothesis for an explicitly reviewed corvu input-flow premise | Medium plus ownership decision / unprototyped |
| 8 | **I: scoped condition support** | **36 vetoes, zero census refusals** | **0 expected for grammar/observer changes alone**; 36 are exposed to the next execution blocker | Small grammar change, larger executor dependency / mechanism tested |
| 9 | **H: computed read on a library-typed receiver** | 6, all host computed-style reads | **0 from a typing-only change**; at most 6 with a reviewed host premise and executable veto | Host-contract work / not a low-cost library rule |
| 10 | **C: reviewed library member returns a fresh array** | **0 confirmed direct first-refusal sites** for the catalogue's historical match/split/keyframe examples; 18 not reproduced | **0 budgeted** until a current complete blocker set is identified | Unmeasured scope; dispatch/provenance work required |
| Decision lane | **F: dependency terminator** | **48 `creates`**, plus 20 separate `returns` dependency withholdings | **0 until the human decision**; 30–38 is a conditional planning range, with a direct `creates` ceiling of 40 after excluding 8 creation calls | Policy decision, then composition work |

Ranges are deliberately lower than class sizes. They must not be summed:
B/E/A share return-evidence infrastructure and expose each other's next
blockers; D/J/I share a source-execution limitation. A disappearing first
refusal is not a disappearing withheld candidate.

**1. A0: the geometry leaf has an explicit `never` premise.** The current
fixture demands `applyBoxDelta → applyAxisDelta → applyPointDelta → scalePoint`.
The reaching optional slot is `undefined`; inside `if (boxScale !==
undefined)`, the first call to `scalePoint` states argument types
`number, never, number`. The second call states `number, number, number`.
Assignment from the first call **does not remove the second call's slot-0
number premise**. The leaf's first transcript records only
`scale * distanceFromOrigin`; its second records no coercion. Both state a
primitive completion.

`mayBeObjectTypedLocked` already includes `TypeFlagsNever` in its non-object
flags, but rejects an empty `Distributed()` result before checking those
flags. The pinned TypeScript-Go `Type.Distributed()` explicitly returns `nil`
for `never`. A temporary explicit flag check changed the leaf's coercion
count **1 → 0**; restoring the code changed it **0 → 1**. Twelve selected Go
tests passed with the prototype, including existing unknown, object-result,
written-binding, and declaration-mismatch controls. See the diagnostic for
the exact source, commands, and observations.

The closing premise is: **this operand's checked type is explicitly bottom
under the parameter premise bound at this exact call**. It is not that the
producer failed to find a constituent. The current consumer already checks
the call row, demanded slot identity and byte-for-byte helper echo in
`census_call_argument_premises_are_bound` / `census_local_premises`, and names
`census-premise:<helper span>:1:never`; its local-declaration transcript digest
binds the producer's classification. The parent's residual coercion must
still go through `census_coercion_rests_on_primitive_calls` and its
`primitive-coercion` disposition. A production change should explicitly name
the bottom-type premise in the policy/review and audit the other callers of
the shared classifier; this prototype is not approval to generalize an empty
list into proof. If generalized beyond an echoed helper premise, add a
corresponding explicit form/type witness rather than lose that receipt basis.

The six current `applyBoxDelta` records name
`motion-dom/dist/es/projection/geometry/delta-apply.mjs:299..325`, the matching
multiplication. Its fixture's complete remaining blocker set is the leaf
coercion plus already-reviewed parameter accessor and parent completion
dispositions. This supports a six-candidate estimate. It does not establish
that a fresh corpus veto will complete. The separate `transformAxis` records
at `259..278`, unknown operands, and other geometry bodies remain outside
this slice. Missing types and empty lists without an explicit bottom flag
must still refuse.

**2. B: require a complete return derivation, not an object-looking type.**
Source inspection identifies these four current families, six records each:

| Candidate / first source | Value being accessed | Next difficulty |
| --- | --- | --- |
| `scrapeSVGMotionValuesFromProps`, SVG scrape `719..739` | `newValues[targetKey]`, from the HTML scrape helper | Return a data-only object on every completion; bind the write position too |
| `parseAnimateLayoutArgs`, `LayoutAnimationBuilder.mjs:12653..12689` | `resolveElements(...)[0]` | Helper returns literals, `Array.from`, and `filter` results, and accesses a host selector |
| `getVariantContext`, `get-variant-context.mjs:627..642` | `context.initial` | Recursive result mixed with `{}` and an `undefined` completion |
| `mixColor`, `utils/mix/color.mjs:1430..1441` | `...fromRGBA`, from `asRGBA` | Dynamic `type.parse` selection, a rewritten result, and further calls |

The narrow premise is: every reachable completion of the exact local callee,
under this call's input premises, yields a value with the stated
access/iteration behavior, through enumerated return sites and permitted
aliases/writes. The verifier must bind the subject to the call row, resolve
the stable local callee in authenticated runtime source, demand its census
under the same argument premises, and check a **stated complete return
derivation**. A receipt should name caller form and position, call site,
callee/return spans, premise identity, return derivation and callee transcript
digest. Reuse ADR 0045's two-transcript binding, not its primitive boolean as
an object-shape certificate.

Begin with the SVG scrape helper: its source initializes `newValues = {}`
and returns that binding on both completion paths. Six candidates are a
defensible first target. Twelve assumes also closing the recursive context
family, a separate completeness exercise. The array/host result and dynamic
parser families are not credited to that slice. Unknown/external results,
proxies, constructors, unexplained writes, defaults, incomplete return sets,
and accessor-bearing objects remain refused. The standing own-literal
mutation assumption from ADR 0044 must be stated if reused; a return type
alone cannot silently strengthen it.

**3. E: bind the returned function and its captured environment.** There are
46 exact initialized-value refusals: `backIn` 8; motion-dom's
`supportsLinearEasing` 18, `isCSSVariableName` 6 and `easeCrossfadeIn` 6; and
the utility `reverseChain` callback binding 8. Current
`census_local_declaration_node` accepts a function/arrow initializer and
refuses a call initializer. Returned-callable descent in the producer also
does not trace unknown call results.

The premise must identify the stable binding's initializer call, the exact
factory, every possible returned callable, and the captured values for that
factory invocation. The consumer must check both the factory's execution and
the returned body's census with those captures. A receipt names the binding,
factory call, factory transcript, complete returned-body set, capture-to-input
mapping, and each instantiated body transcript. Merely finding a return arrow
is insufficient: `memoSupports(ownCallback)` must not turn this artifact's
callback into an arbitrary caller exemption.

The 8-record `backIn` family and 6-record CSS-variable predicate are better
first targets than taking all 18 capability-memoizer records at once. An
8–20 estimate allows those smaller paths and some reuse, but does not assume
all factories' bodies clear. Conditional or opaque return identities,
unbound captures, written bindings, external factories without contracts,
and unresolved calls inside the returned body still refuse. This is a
callable-identity premise, distinct from B's object-access premise.

**4. A: there is still return-type work, but the 56 are not one cause.** The
56 are motion's binary-coercion first records: 8 `cubicBezier`, and six each
for `applyBoxDelta`, `applyPxDefaults`, `calcBoxDelta`, `measurePageBox`,
`removeAxisDelta`, `removeAxisTransforms`, `removeBoxTransforms`, and
`transformAxis`. The full corpus binary class is 65. A0 accounts for only six
of the 56. The remaining sources include an unannotated `mixNumber` result
passed as `originPoint`, arithmetic on a rewritten local, array elements,
and nested returned-callable parameters. The current fixture does not prove
these other paths have the same cause.

The verifiable premise is: this **particular call**, under an already-bound
input premise, has the callee transcript's stated completion type and type
identity. To apply it in a caller twin, the consumer must first bind the
callee and establish that result independently, then bind the caller's
reclassification to that proof. The receipt names both transcripts, call
span, input premise, output type/identity and the twin transformation. A
`primitiveCompletion` boolean cannot justify annotating `number`, and the
callee's exported declaration cannot substitute for its internal caller's
argument premise. Nor can one call's result premise annotate all uses of the
same helper without checking their different inputs. Cycles need an explicit
grounded proof, not mutually assumed return annotations.

Start with one `mixNumber → originPoint → scalePoint` chain and measure every
form on that body. A 6–18 estimate reflects one to three six-record families,
not the entire arithmetic class. Unknown/union element operands, omitted
spelling evidence, written or unresolved callees, unsupported nested inputs,
and ungrounded recursive return dependencies remain refused.

**5. J: distinguish overload completeness from implementation absence.** The
30 incomplete transcripts split into **27 `callSignatureNotUnique`** (`noop`
9; `queryOptions`, `mutationOptions`, `infiniteQueryOptions` 6 each) and **3
`implementationUnavailable`** (`removeOldestQuery`). The early
`len(signatures) != 1` return in `export_value_transcripts.go` is separate
from protocol 21's corrected overload count.

The premise is a complete overload family bound to one executable
implementation, or a complete set of separately checked overload/body
premises when contextual typing matters. The consumer must verify the exact
declarations, ordinals/count, implementation identity and required premise
for every member. A receipt names the full overload set, body span and all
relevant transcripts; picking the first signature is not a proof. Start with
the nine `noop` records, whose empty body is the smallest completeness
question. The other three records require finding an actual body and are not
J's gain.

These TanStack source-condition paths also need an executable veto. Thus
9–27 possible census clearances are an enabling estimate, with **zero net
withholding reduction budgeted in isolation** until the source lane runs.
Incomplete sets, unresolved implementation aliases, absent bodies and an
open body census keep refusing.

**6. D: split construction, methods, and overload-to-body resolution.** The
52 are 24 motion constructions (`MotionValue` at two sites, `JSAnimation`,
`ViewTransitionBuilder`, six each), one i18n `new Traps`, and 27 TanStack
TS-source calls. The latter split into `partialMatchKey` 18 and `getAll` 9.
The consumer currently searches function nodes and cannot treat a class name
as the whole execution of `new`.

A constructor premise must enumerate constructor execution, instance
initializers, superclass construction, and any relevant computed/decorator
or lowered initialization behavior at the correct phase. The receipt names
the exact class and construction site, ordered executable regions, heritage
targets, arguments, and transcripts for all reachable calls. A class name or
the `own-class` **`instanceof`** premise proves none of this. An exact method
dispatch additionally needs the receiver's runtime identity; a TS overload
needs the executable implementation bound to its declaration family, not
the smallest contained function.

The initial 0–12 estimate credits at most one or two motion construction
families after inspecting their complete initialization graph. No
end-to-end constructor clearance was measured. The 27 TS paths overlap J's
implementation-resolution work and I's executor limitation; do not count
them as immediate certified gain. Unknown heritage, substituted methods,
unmodeled initializers, unauthenticated dependency bodies and every new
callee's own blockers remain refused.

**7. G: provenance must be proved at the nested invocation.** The 44 are
corvu's `some`-style `signal()` calls 28, `motion-utils.pipe` 8, and
`createCallbackStack` 8. Current code explicitly refuses a nested parameter,
and the roadmap assigns the unresolved behavior to `callbacks`. Do not make
the declaration kind `Parameter` sufficient for a creates exemption.

A possible narrower extension would state that a specific nested slot
receives an element of a particular caller-supplied iterable through an
exact reviewed invoker. The consumer would bind the invoker/call row, outer
receiver provenance, callback argument/body, delivered slot and nested call.
The receipt would name that complete flow, including captured roots; a full
callback-domain contract is an alternative when the invoker is external.
This would need an explicit review of the ownership boundary, building on
ADRs 0042/0048 rather than overriding the current refusal text.

The corvu leaf family is the best place to test such a proposal: 28 records
share the small `signals.some(signal => signal())` shape. A 20–28 gain is
only a hypothesis under that **new reviewed premise**, not current-policy
gain. Pipe's reduction inputs and callback-stack's mutation/element sources
need their own mappings. A callback receiving a locally created function,
an arbitrary helper's output, an unknown invoker, or an unbound nested slot
must still refuse. Nothing here closes the `callbacks` domain itself.

**8. I: the grammar fix exposes an executor refusal.** All 36 affected
records are `creates` vetoes on `@tanstack/custom-condition`: three query
rows with four records each and three persist-client rows with eight each.
The value 18 is not the current candidate-occurrence count.

The investigation's temporary resolver/Node experiments confirmed that the
scoped condition can select an ESM target. However, the harness both rejects
`@`/`/` in `plain_condition_name` and concatenates the condition into an
observer package name. Admitting the grammar without encoding that observer
name creates a package/subpath lookup, so there are two changes, not one.
Use an exact, collision-free observer token while retaining the original
condition as the export key and `--conditions` argument.

The receipt still must bind requested conditions, observed ESM/require
conditions, exact runtime targets, every dependency edge, and closure-wide
condition neutrality, including any admitted reproduction condition. Unknown
or unobserved conditions and target divergence still refuse.

The selected TanStack target is TypeScript under `node_modules`. A temporary
Node 24.11.1 reproduction selected that target and then failed with
`ERR_UNSUPPORTED_NODE_MODULES_TYPE_STRIPPING`, including with
`--experimental-strip-types`. This is a separate authenticated execution
profile problem. **Budget zero candidate gain for grammar and observer
encoding alone.** Do not count a differently named veto refusal as a closure,
or move the import outside the authenticated layout as an undocumented fix.

**9. H: all six receivers are host computed-style results.** The source is
`const computedStyle = window.getComputedStyle(element)` followed by
`computedStyle[name]` at `style-computed.mjs:232..251`. It is not a fresh
engine array or a primitive receiver. The library declaration is not a
runtime guarantee about a host object or a replacement host function.

The required premise would identify the host operation and realm, exact
receiver/result provenance, and the computed property's lookup behavior in
the supported runtime. A consumer must bind that to a reviewed host contract
or execution-model authority; a receipt names the host operation, contract
identity, result derivation, key/read site and environment. Arbitrary
library-typed objects, proxies, replacement host methods, unsupported realms
and unknown property behavior still refuse. No gain is justified from
typing alone; six is the ceiling after the host premise and veto environment
are supplied.

**10. C: remeasure a body before designing the member table.** None of the
449 current first refusals names the historical `match[2]`,
`value.split(...)[0]` or `resolvedKeyframes[index]` examples. Their presence in
source is not proof of a live first-refusal opportunity. There are only 34
element-access first refusals in total, including H's six, B's twelve, eight
written-parameter `handleDiffArray` cases, four global-registry accesses,
three input-mask accesses and one virtual range-array access. The catalogue's
18 direct fresh-library-result refusals could not be reproduced.

A future premise must identify the exact reviewed operation and prove the
dispatch/receiver/argument conditions under which its output has the claimed
lookup behavior. A receipt would name those conditions, output derivation,
call site and consumed read/iteration. The consumer checks them against an
explicit operation table and authenticated transcript facts, not the printed
array return type. `filter`/`slice` species construction and match/split
hooks are not covered by a name-only rule. A local runtime counterexample
used an Array subclass whose species was a Proxy of the Array constructor:
`filter` returned a real array (`Array.isArray === true`) whose indexed read
still triggered a Proxy trap. The array type alone therefore proves too
little. Unknown dispatch, custom species/hook paths without a separately
bound caller-provenance premise, and unexplained result mutation remain
refused. Budget zero until a current complete body is measured.

**F, held for the human decision.** The 48 `creates` records are `isObject`
18, `progress` 12, `easingDefinitionToFunction` 6, `createSignal` 6, `chain`
2, `createMemo` 2 and `getOwner` 2. The additional 20 are `returns`
withholdings composed from dependency claims; they are not 20 missing census
records. Eight visible creation calls (`createSignal`/`createMemo`) already
rule out a direct 48-candidate gain. An authenticated empty claim for the
others has not been established merely by counting them.

The concrete contract-preserving option is a new accepted-dependency census
disposition: bind the resolved callee to an exact authenticated graph edge,
dependency artifact case/export, and acceptance receipt whose `creates`
claim is **closed and empty**. The verifier must authenticate that receipt,
issuer/policy, artifact and semantic identities, applicability and graph
composition order; cyclic claims cannot certify each other without a
grounded authority. The witness names parent call/case, dependency node and
edge, export/domain, receipt and proof-root digests. Existing
`AcceptedDependencyComposition` machinery provides a place to bind this;
`census_call_disposition` has no such terminator today.

The other choice is to retain today's boundary and keep these withheld.
Crossing into external implementations instead is a different policy decision
that changes the standing contract-driven boundary; it was not selected here.
No recommendation in this report authorizes either implementation. After an
explicit composition decision, 30–38 is a planning range for the mainly
`isObject`/`progress` families plus a few smaller paths, **conditional on
existing or independently established empty dependency claims**. The
direct creates ceiling is 40 after the eight known creation calls, not
40–60. Open/nonempty dependency claims, unmatched cases, missing authority,
cycles and newly exposed body/veto blockers remain refused. Any downstream
`returns` recovery is counted separately.

There is one additional measured lead worth keeping ahead of an unmeasured
array table: **eight `handleDiffArray` first refusals are written parameters**.
Both versions do `prev = prev.slice(i)` and `current = current.slice(i)`.
Their first `current[i]` therefore fails the file-wide root test even before
the assignment executes. ADR 0050 extended written *locals*, not parameters.
A parameter-join premise would have to include its entry argument and every
assignment, and compose the existing caller-result derivation for the method
call. Its receipt must name the entry slot and every source, not whichever
assignment happens to be convenient. A 0–8 gain is an unprototyped lead;
array provenance alone does not close its whole blocker set. Similarly,
`transformBox`'s six `resolveBox.x` refusals need a multi-slot root for
`sourceBox ?? box` and still reach A's arithmetic; do not credit six immediate
closures to the root join.

Verification and scope: the temporary Go diagnostic/prototype was removed;
production source returned to its original state. The producer experiments
ran offline, passed as described above, and demonstrated a recorded form
disappearing and reappearing. Temporary condition-resolution/Node and array
species experiments ran; no packages were installed. No analyzer rule,
finding, fixture snapshot, bundled contract, protocol, binary or acceptance
receipt was changed. No `tsc` diagnostic was added or duplicated. The
published-typing oracle was not rerun because this investigation introduces
no diagnostic rule; any eventual new semantic fixture must still satisfy
the repository's published-typing boundary.

Only this report, its evidence JSON, the reproduction note and a precision
backlog correction were retained. JSON parsing, candidate-count
reconciliation and `git diff --check` passed. Cargo/Clippy, process tests,
coverage, ownership gates and full `make verify` were intentionally deferred:
there is no retained implementation and no commit to certify. In particular,
the 368/30 and 506 counts are the reconciled prior cohort, not a new green
verification claim. Before shipping any slice, add nonvacuous producer and
consumer positive/negative controls, rebuild through the pinned Make targets,
measure both the first-refusal change and the withheld-candidate shortfall,
and run full `make verify` for each commit.
