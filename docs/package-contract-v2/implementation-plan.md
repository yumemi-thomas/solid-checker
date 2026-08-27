# Exhaustive implementation plan

This is the execution roadmap for the approved next package-contract design.
Supporting semantic, architectural, proof, producer, conformance, and migration
details are normative within this documentation set.

## Phase 0 — Baseline and authority

1. Inspect the dirty worktree and preserve unrelated changes.
2. Record checker, Type Facts, compiler, and package pins.
3. Verify exact `solid-js@2.0.0-rc.3` and related `@solidjs/*` integrity,
   runtime artifacts, declarations, manifests, and export maps.
4. Run the legacy contract corpus with caches disabled.
5. Save machine-readable and human-readable baseline reports.
6. Classify every refusal by schema, Type Facts, compiler facts, generator,
   resolver, probe, runtime, or TypeScript ownership.
7. Measure legacy main-contract, expanded, evidence, time, and memory baselines.
8. Freeze representative legacy fixtures that expose known false-negative and
   over-refusal paths.

**Exit:** the baseline is reproducible and every current verified/refused row
has a stable reason.

**Status 2026-08-27:** complete. The historical machine and human reports are
immutable evidence and are validated by `make phase0-baseline` and
`make verify`. Validation pins their exact bytes and checks their internal
input, fixture, classification, aggregate, and measurement invariants; it does
not reinterpret the historical compiler and Type Facts revisions through the
current dependency layout. New comparison captures require explicit output
paths and cannot overwrite the frozen baseline by default.

## Phase 0A — Compiler and Type Facts source bootstrap

This phase follows
[the compiler and Type Facts bootstrap](compiler-and-typefacts-bootstrap.md).
It changes source ownership before adding new contract semantics.

### Solid compiler transition

C1. Record the exact current checker compiler pin, semantic trace version,
    generated-output baseline, and findings baseline.
C2. Fetch `solidjs/solid` and record the exact current `upstream/next` commit.
C3. Create a clean `yumemi-thomas/solid` branch from that commit, never from a
    dirty or stale local branch.
C4. Freeze the fork invariant: patches may contain semantic-fact models,
    output-neutral recording hooks, validation/serialization, and fact tests
    only.
C5. Inventory every semantic-trace change on
    `yumemi-thomas/dom-expressions#next` through the selected head.
C6. Classify each old change as semantic-only, already upstream, superseded,
    obsolete, or an upstream compiler issue.
C7. Exclude every lowering, generated-output, diagnostic, runtime, feature,
    performance, unrelated dependency, and unrelated refactor change from the
    fork.
C8. Port semantic trace version 2 to the new compiler's existing decision
    sites without changing their control flow or results.
C9. Port total-census, terminal-decision, discarded-site, callback-role, and
    owner-establishment validation.
C10. Port every still-applicable trace reconciliation regression as a fact test.
C11. Leave a fact open when it cannot be emitted without changing compiler
     behavior; file the behavior issue upstream independently.
C12. Prove trace-disabled and trace-enabled JavaScript, source maps,
     diagnostics, and side effects are identical across the compiler corpus.
C13. Prove generated output matches the exact upstream base when semantic facts
     are ignored.
C14. Adapt the checker to package `solidjs-compiler` and its host-independent
     Rust interface.
C15. Replace the Solid 2 Cargo dependency with an exact revision of the Solid
     fork; retain the separate Solid 1.x compiler.
C16. Update compiler notices, cache identity, version assertions, and
     conformance reports atomically.
C17. Run compiler, adapter, process, coverage, ownership, and full finding-
     parity gates before adding a new fact.
C18. Record a port ledger ruling for every historical trace or compiler change.

**Status 2026-08-27:** C1-C18 are complete at
`yumemi-thomas/solid@1d81e67fd393d12c74b13aa7d3fb492f3d85353b`; see the
[port ledger](compiler-bootstrap/2026-08-27-port-ledger.md) and
[conformance report](compiler-bootstrap/2026-08-27-conformance.md). No upstream
Solid PR was opened. Finding parity had one reviewed precision improvement:
the current compiler supplies a positive tracked fact where the former producer
was silent; the other 557 findings are unchanged.

### Type Facts repatriation

T1. Freeze `solid-ts-facts@92c53392388518d69ef27220729f5c061479deed` as the
    import baseline and require clean source trees.
T2. Import the exact external history under a temporary prefix rather than
    copying a working directory.
T3. Relocate the Go producer to `apps/solid-typefacts`, the Rust client to
    `rust/crates/typefacts`, schemas to `schema`, producer docs to
    `docs/typefacts`, and goldens to `benchmarks/typefacts` with `git mv`.
T4. Preserve local TypeScript-Go shims, licenses, ADRs, lifecycle tests,
    retained-session tests, adversarial tests, and memory benchmarks.
T5. Reconcile Type Facts terminology into `CONTEXT.md` without exposing
    implementation terms as checker concepts.
T6. Add the Rust client to the main workspace and replace its git dependency
    with a path dependency.
T7. Build the Go producer from local source and delete clone, fetch, detached-
    checkout, and revision-extraction behavior.
T8. Replace the external revision stamp with a source-manifest digest covering
    producer, client, shims, schemas, dependency pins, toolchain identity, and
    build id.
T9. Retain the startup protocol, schema, and build handshake. Validate codec
    limits from the language-neutral schema and bind them in the source
    manifest; do not claim a fourth handshake field before a protocol change.
T10. Update CI, release packaging, gate caches, notices, and contributor
     instructions for local ownership.
T11. Replay identical request transcripts through external and imported builds
     and compare byte and decoded semantic results.
T12. Prove lifecycle, cancellation, restart/replay, incremental, memory,
     performance, and checker-finding parity.
T13. Run two clean CI passes and one release build before making the external
     repository read-only or archived.
T14. Require all later Type Facts changes to include their producer, Rust
    client, checker consumer, and proof fixtures in this repository.

**Status 2026-08-27:** T1-T14 are complete. T11-T12 parity evidence is recorded
in the repatriation conformance report; response bytes are compared after
removing only nondeterministic numeric timing values, while timings are gated
separately. T13 completed with successful PR and post-merge CI plus the
post-merge cross-platform release-package matrix. The external repository is
archived and preserved as provenance.

**Exit:** Solid 2 uses a pinned semantic-only fork of the compiler at its new
upstream location; Type Facts builds locally; both transitions preserve the
frozen checker findings and fail-closed behavior.

## Phase 1 — Semantic and policy freeze

9. Adopt the canonical terms added to `CONTEXT.md`.
10. Finalize the four-state claim-domain lattice.
11. Finalize operation cardinality semantics.
12. Finalize the restricted guard language.
13. Finalize operation, resource, value-shape, ownership, tracking, and trigger
    vocabularies.
14. Finalize structural refusal versus local semantic incompleteness.
15. Finalize main-contract versus sidecar versus receipt ownership.
16. Finalize semantic and wire digest rules.
17. Finalize experimental-status locality.
18. Resolve all remaining questions in the semantic-model and wire drafts.

**Exit:** no semantic behavior depends on undocumented omission, inheritance,
or consumer convention.

**Status 2026-08-27:** complete. Items 9-18 and the exit condition are recorded
in the [Phase 1 semantic and policy freeze](phase1/2026-08-27-freeze.md).

## Phase 2 — Deep module interfaces

19. Define private wire types in `solid-facts-backend`.
20. Define normalized contract types in `solid-reactive-ir`.
21. Define `AcceptedContract`, `ContractProposal`, `EvidenceBundle`, and receipt
    types.
22. Define the single contract-loading interface.
23. Define semantic query methods used by Reactive IR.
24. Define the artifact-resolution interface and its two adapters.
25. Define the evidence-store interface and its bundled/local adapters.
26. Define typed failure and refusal outcomes.
27. Add interface-level tests before moving current consumers.

**Exit:** callers can be implemented without learning the wire schema.

**Status 2026-08-27:** complete. Items 19-27 and the exit condition are
recorded in the [Phase 2 interface freeze](phase2/2026-08-27-interfaces.md).
The loader deliberately returns a typed `NormalizationUnavailable` refusal for
otherwise valid development-schema documents until Phases 5 and 7 implement
normalization and the schema. No current producer or consumer moved in this
phase.

## Phase 3 — Type Facts specification and local implementation

28. Specify resolved-invocation demands.
29. Specify selected-signature identity.
30. Specify actual-to-formal binding, optional arguments, rest, and spreads.
31. Specify nested callable paths.
32. Specify finite value-domain facts.
33. Specify complete parameter-use census.
34. Specify return/throw/control-flow census.
35. Specify transcript identity and completeness envelopes.
36. Implement TypeScript-Go adapters in `apps/solid-typefacts`.
37. Implement Go producer and Rust client changes together.
38. Add aliases, overloads, generics, spread, rest, union, and stale-generation
    tests.
39. Add differential tests against published TypeScript behavior.
40. Bump the local Type Facts protocol/schema and source-manifest identity
    atomically.
41. Rebuild and handshake the Type Facts producer.
42. Add main-repository integration fixtures.

**Exit:** generator and IR no longer guess variadic, overload, nested-callback,
or finite guard facts.

**Status 2026-08-27:** complete. Items 28-42 are implemented and specified in
[Type Facts invocation transcripts](phase3/invocation-transcripts.md), with the
handoff evidence and exact remaining refusals in the
[Phase 3 completion report](phase3/2026-08-27-typefacts.md). The operation is a
demand-shaped read rather than a retained-table expansion. The existing checker
continues to fail closed where these richer proofs are not yet consumed; the
replacement generator and normalized IR receive them through the backend seam
in Phases 5-8, without a source/name heuristic in the interim.

## Phase 4 — Compiler execution-facts protocol 2

43. Specify source/generated operation identities.
44. Specify terminal execution dispositions.
45. Specify trigger, scheduling, tracking, cardinality, and owner relations.
46. Emit semantic facts from actual lowering decisions.
47. Reconcile every compiler-controlled source site with output.
48. Bind source, configuration, mode, compiler revision, and output digests.
49. Add DOM, SSR, ref, event, control-flow, discarded, and generated-callback
    tests.
50. Add server-function transformation facts.
51. Add direct regression tests for every known semantic-trace divergence; fix
    compiler behavior upstream rather than in the semantic fork.
52. Update both dialect adapters where protocol structure is shared.
53. Update cache identity and invalidation.
54. Move the exact `yumemi-thomas/solid` revision, trace version, checker
    protocol, and notices atomically.

Every change in this phase remains semantic-fact-only within the compiler fork.
If a proposed fact requires a lowering, output, diagnostic, runtime, feature,
performance, or unrelated compiler change, leave it open and split the compiler
work into an independent upstream contribution.

**Exit:** compiler-controlled execution is established by reconciled lowering,
not source-shape prediction.

**Status 2026-08-27:** complete. Items 43-54 and the exit condition are
implemented by Solid 2 semantic trace version 3 and checker compiler-facts
protocol 2. Exact identities, scope proof, verification evidence, and remaining
open domains are recorded in the
[Phase 4 completion report](phase4/2026-08-27-compiler-facts.md); the normative
boundary is [compiler execution facts](phase4/compiler-execution-facts.md).
No upstream Solid pull request was opened.

## Phase 5 — Normalized semantic model

55. Implement local `KnowledgeSet` completeness.
56. Implement recursive value shapes with leaf-local unknown.
57. Implement capability validation.
58. Implement owner relationships and capabilities.
59. Implement resources and lifetimes.
60. Implement operation nodes and causal edges.
61. Implement cardinality and possible/guaranteed distinction.
62. Implement restricted guards and finite partition validation.
63. Implement monotone joins for unresolved guard selection.
64. Implement artifact cases and exact export identity.
65. Implement experimental status at export/case scope.
66. Implement canonical semantic digest.
67. Add property tests for normalization equivalence and leaf locality.

**Exit:** the normalized model can express every row in the Solid 2 conformance
matrix or represent its exact open domain.

**Status 2026-08-27:** complete. Items 55-67 and the exit condition are
implemented in `solid-reactive-ir::contract_semantics`. The wire-independent
model, invariant set, conformance-row mapping, tests, and exact remaining open
domains are recorded in the
[Phase 5 completion report](phase5/2026-08-27-normalized-semantic-model.md).
The backend's development wire decoder remains fail-closed and no generator,
receipt, bundled contract, or analyzer consumer was migrated in this phase.

## Phase 6 — Temporary wire schema version 2

68. Add the required `format` discriminator.
69. Add `semanticModelVersion`.
70. Add exact package integrity and manifest identity.
71. Add unconditional and conditional artifact-case forms.
72. Add runtime artifact, declaration, and closure digests.
73. Add direct export-to-summary references with no overrides.
74. Add local `closed` lists.
75. Add operation, resource, guard, and value-shape definitions.
76. Add hash references to proof and probe sidecars.
77. Exclude `schemaStatus`, inline evidence, generator identity, trust status,
    and compiler-facts protocol.
78. Add cross-field Rust validation.
79. Add at least three complete golden contracts and every knowledge state.
80. Add bounded-size and recursion validation.

**Exit:** every golden document round-trips to identical normalized semantics.

**Status 2026-08-27:** complete. Items 68-80 and the exit condition are
implemented by the crate-private `solid-facts-backend::contract_document_v2`
boundary and the temporary structural JSON Schema. Three complete goldens,
all four knowledge states, normalization-equivalence checks, adversarial
cross-field validation, and resource-limit tests are recorded in the
[Phase 6 completion report](phase6/2026-08-27-temporary-wire-schema-v2.md).
No generator, bundled contract, analyzer consumer, proof sidecar, receipt, or
stable public-schema cutover occurred.

## Phase 7 — Artifact resolution and closure

81. Define exact resolution records.
82. Consume host/Type Facts resolved imports during ordinary analysis.
83. Implement standards-compatible standalone package resolution.
84. Preserve ordered export-map branch traces.
85. Bind runtime and declaration targets independently.
86. Build canonical local runtime/declaration closure manifests.
87. Represent external dependency edges through accepted contract digests.
88. Hash materialized compiler/virtual output where behavior depends on it.
89. Open affected domains for nonliteral dynamic loading, `eval`, native code,
    opaque WASM, or mutable unbound globals.
90. Add nested/custom/default/import/require/browser/node/worker/deno/bun and
    symlink tests.
91. Add zero-match, multiple-match, stale-hash, and same-byte/different-closure
    adversarial tests.

**Exit:** selected semantics are identical to actual artifact resolution or the
case is refused.

**Status 2026-08-27:** complete. Items 81-91 and the exit condition are
implemented by `solid-facts-backend::artifact_resolution` and the standalone
package-acquisition resolver. Exact host/Type Facts provenance is retained in
ordinary analysis; conditional export resolution, independent runtime/types
bindings, canonical closure identity, opaque-frontier weakening, and
zero/multiple/stale-identity refusal are covered by focused adversarial tests.
The exact boundary, test results, and remaining fail-closed domains are recorded
in the [Phase 7 completion report](phase7/2026-08-27-artifact-resolution-closure.md).
The proposal generator, proof/receipt authority, bundled contracts, and analyzer
contract consumers remain unchanged for their scheduled later phases.

## Phase 8 — Proposal generator refactor

92. Move semantic proposal construction behind the Rust module interface.
93. Keep Node responsible only for acquisition and process orchestration.
94. Split package discovery, resolution, analysis, proposal, proof planning,
    probe planning, and emission into explicit stages.
95. Emit partial positive operations without claiming closure.
96. Emit local unresolved edges and proof obligations.
97. Preserve unrelated closed candidates when one domain is incomplete.
98. Prevent proposal code from writing accepted `closed` fields.
99. Remove semantic variant collapse and mutable shared summaries from the
    JavaScript path.
100. Add proposal fixed-point and deterministic-output tests.

**Exit:** generation without verification always produces an unaccepted open
proposal.

Completed on 2026-08-27. The replacement path now terminates semantic proposal
construction in `solid-facts-backend::proposal_generation`, requires exact
Phase 7 artifact binding, withdraws proposed closure into local proof
obligations, preserves partial positive operations and unrelated candidates,
and emits only deterministic `unaccepted` proposal plans. Node contains a
seven-stage acquisition/orchestration pipeline with no variant collapse or
mutable semantic summaries. Fixed-point, false-closure, local-uncertainty, and
deterministic-output behavior is covered by focused Rust and Node tests. The
public legacy generator remains unchanged for the Phase 14 atomic producer
migration. See the [Phase 8 completion report](phase8/2026-08-27-proposal-generator-refactor.md).

## Phase 9 — Claim IDs and evidence sidecars

101. Define semantic claim IDs independent of JSON position and summary name.
102. Record artifact, closure, fact, proof, probe, environment, and tool identity
     per claim.
103. Separate proof/fact and runtime-probe sidecars.
104. Bind sidecar hashes in the main contract and contract identity in sidecars.
105. Add explicit document-kind/version discriminators.
106. Reject stale, cross-artifact, cross-package, and orphan sidecars.
107. Prove ordinary analysis works after raw sidecars are removed.

**Exit:** evidence is auditable and hash-bound but absent from the analysis hot
path.

Completed on 2026-08-27. The normalized model now assigns versioned claim IDs
from exact package, artifact-case, export, and validated semantic-path identity
without using summary names or JSON positions. The backend emits and strictly
validates separate proof/fact and runtime-probe sidecars, binds their hashes
from the temporary main document and its normalized identity back from each
sidecar, rejects stale/cross-package/cross-artifact/orphan material, and
returns no raw evidence bytes to ordinary analysis. See the
[Phase 9 completion report](phase9/2026-08-27-claim-ids-evidence-sidecars.md).

## Phase 10 — Probe redesign

108. Replace timing sleeps with semantic event markers.
109. Record call, render, flush, callback, cleanup, settlement, emission,
     transition, request, response, and stream events.
110. Build exact artifact-case mode matrices.
111. Isolate process/realm and module state.
112. Bound timeouts and microtask/macrotask draining.
113. Add deterministic repeat-run consistency.
114. Allow probes to add possible-positive witnesses and falsify closure only.
115. Prohibit probe promotion of negative, minimum, maximum, or exhaustive facts.
116. Add cleanup, repeated AsyncIterable, transition, request, and root-lifetime
     probes.

**Exit:** probes detect contradictions without becoming a negative-proof engine.

## Phase 11 — Proof checker and acceptance receipts

117. Implement package, manifest, artifact, declaration, and closure proofs.
118. Implement export and selected-signature proofs.
119. Implement argument/rest/spread/callable-path proofs.
120. Implement operation reachability and cardinality proofs.
121. Implement recursive value-shape proofs.
122. Implement guard disjointness/exhaustiveness proofs.
123. Implement compiler lowering reconciliation proofs.
124. Implement accepted dependency composition.
125. Consume probe contradiction records.
126. Finalize only verified local closure.
127. Compute closed-claim proof root.
128. Issue acceptance receipt.
129. Implement bundled and project-local receipt storage.
130. Add false-closure mutation tests for every proof family.

**Exit:** no unaccepted closed claim can discharge an analyzer obligation.

## Phase 12 — Analyzer integration

131. Replace legacy Rust decoding with private v2 wire decoding.
132. Validate receipt before exposing accepted semantics.
133. Select artifact case through actual resolved import.
134. Resolve exports through exact runtime identity.
135. Instantiate guards from exact call-site Type Facts.
136. Make every consumer demand-sensitive by claim domain.
137. Distinguish possible behavior, guaranteed behavior, complete absence, and
     unknown behavior.
138. Define native dialect precedence and conflict refusal.
139. Update cache identity to include semantic, artifact, and receipt policy.
140. Update diagnostics for precise local open-domain reasons.
141. Add `tsc`-silent witnesses for every contract-driven finding.

**Exit:** one open domain does not refuse or weaken unrelated known behavior.

## Phase 13 — Solid 2 RC.3 conformance

142. Encode split `createEffect`.
143. Encode `createTrackedEffect` and `onSettled`.
144. Encode batching and `flush()`.
145. Encode `For`, `Repeat`, `Show`, and `Match` callback variants.
146. Encode Promise and AsyncIterable computations.
147. Encode Loading, pending, latest, refresh, and affects.
148. Encode actions and optimistic state.
149. Encode stores, drafts, projections, snapshots, shallow/deep tracking, and
     reconciliation.
150. Encode two-phase refs/directives.
151. Encode root-owned event delegation.
152. Encode render, hydrate, render-to-string, and render-to-stream cases.
153. Encode request-scoped status/header and response commitment.
154. Compose server-function compiler and runtime behavior.
155. Mark server components experimental and leave unstable domains open.
156. Encode conditional adapters through finite guards.
157. Verify mixed-framework artifact selection and refusal.
158. Add positive, negative, partial, refusal, consumer, and TypeScript-oracle
     fixtures for every row.

**Exit:** the entire conformance matrix passes against exact RC.3 artifacts.

## Phase 14 — Producer and consumer migration

159. Switch package generators.
160. Switch missing-contract and bundled-contract generators.
161. Switch probe plan, worker, driver, and harness.
162. Switch verifier and review tooling.
163. Switch closure, pin, differential, review, and obligation scripts.
164. Switch Rust contract generator and CLI validation.
165. Switch WASM host and types.
166. Regenerate bundled contracts in both physical locations.
167. Regenerate runtime locks and dialect manifests.
168. Migrate backend process fixtures and all fixture package contracts.
169. Compare normalized semantics before accepting snapshot changes.
170. Delete legacy unknown sentinels, variants, conditions, inline evidence, and
     duplicate JavaScript semantic normalization.
171. Delete the legacy public decoder rather than maintaining compatibility.

**Exit:** every producer and consumer speaks temporary main schema version 2.

## Phase 15 — Adversarial hardening

172. Attack misplaced and sibling closure.
173. Attack empty open domains and dangling references.
174. Attack operation/resource/summary cycles.
175. Attack contradictory capability combinations.
176. Attack guard overlap and uncovered remainder.
177. Attack resolver precedence and custom conditions.
178. Attack same-byte/different-dependency cases.
179. Attack stale sidecars and receipts.
180. Attack Type Facts completeness and generation identity.
181. Attack compiler-fact reconciliation.
182. Attack path traversal, excessive depth, and document bombs.
183. Attack mixed-framework artifact substitution.
184. Fuzz decode, normalize, encode, and semantic round-trip.
185. Require detection of every seeded false-closure mutation.

**Exit:** malformed inputs remain bounded and all false closure is rejected.

## Phase 16 — Corpus, compactness, and performance

186. Run official Solid RC.3 packages.
187. Run Solid Primitives.
188. Run bundled and synthetic contract corpora.
189. Run mixed-framework and ecosystem corpora.
190. Produce per-domain refusal reports.
191. Preserve current valid verified rows.
192. Reach the 85% installable/generatable and 90% Solid Primitives milestones
     without weakening proof.
193. Measure p50/p95/max main bytes and evidence bytes.
194. Measure generation, probing, verification, load, query, and memory cost.
195. Improve compression only where proof-equivalent semantics remain.
196. Confirm ordinary analysis reads no sidecars and performs no package code or
     network access.

**Exit:** accuracy, automation, compactness, and performance gates pass.

## Phase 17 — Temporary version-2 convergence

197. Audit every main document producer and consumer.
198. Audit every bundle, fixture, sidecar, receipt, cache, and manifest.
199. Prove no legacy-v1 main document or decoder remains.
200. Prove no unrelated document-version namespace was globally replaced.
201. Run the complete clean-cache verification authority.
202. Freeze semantic-model version 1 and canonical digest algorithm.

**Exit:** the repository is internally complete on temporary schema version 2.

## Phase 18 — Atomic stable version-1 cut

203. Change main schema version 2 to stable version 1.
204. Change all main producers and consumers in the same change.
205. Re-emit every main contract.
206. Recompute every wire hash.
207. Reissue every acceptance receipt.
208. Refresh bundles, manifests, locks, fixtures, and cache versions.
209. Rebuild binaries containing bundled contracts.
210. Update public documentation and remove temporary-version language.
211. Assert no temporary-v2 or legacy-v1 main document remains.
212. Run the full contract, TypeScript oracle, ownership, ecosystem, performance,
     and `make verify` authority with caches disabled.

**Exit:** stable schema version 1 is the only public package-contract format.

## Phase 19 — Stable maintenance

213. Permit only backward-compatible optional wire additions within stable
     version 1.
214. Version incompatible normalized meaning independently.
215. Version proof policy without silently reinterpreting old receipts.
216. Track every remaining open claim by exact owning fact domain.
217. Prioritize upstream facts by verified exports unlocked.
218. Re-run false-closure and compiler parity suites on every upstream pin move.
219. Reissue receipts whenever artifacts, closure, facts, proof policy, or
     verifier identity changes.
220. Keep experimental surfaces explicit and revisit only against newer exact
     published authority.

## Final definition of done

- One deep module owns compact document semantics.
- Type Facts and compiler execution facts expose exact, complete premises.
- Type Facts source, client, tests, and checker consumers live in one repository
  behind the retained process/session interface.
- The Solid 2 fork follows `solidjs/solid/packages/compiler` and contains only
  output-neutral semantic-fact instrumentation.
- Generators emit proposals, never accepted closure.
- Proof receipts authorize every closed claim.
- Probes witness and falsify but do not prove negatives.
- Recursive uncertainty and incomplete domains remain local.
- Package, entrypoint, environment, artifact, declaration, dependency, and
  export identity are exact.
- Ordinary analysis uses only accepted normalized contracts and receipts.
- Required Solid 2 RC.3 behavior is represented or explicitly left open.
- TypeScript-owned defects remain TypeScript's responsibility.
- Corpus automation and compactness targets pass under adversarial gates.
- Temporary schema version 2 has been atomically re-emitted as the first stable
  public schema version 1 with no `schemaStatus` field.
