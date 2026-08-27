# Phase 11 completion report: proof checker and acceptance receipts

Date: 2026-08-27

Branch: `codex/phase11-proof-checker-receipts`

Authority: semantic-model version 1, Phase 7 exact artifact/closure identity,
Phase 8 unaccepted proposal closure plans, Phase 9 semantic claim/evidence
identity, Phase 10 claim-local probe contradictions, and proof policy 1

## Result

Phase 11 items 117-130 are implemented. The normalized semantic module now
owns one proof checker and the only `AcceptedContract` constructor. The
backend can adapt an exact `PlannedProposal`, proof replays, and runtime-probe
contradictions into that checker, encode its receipt deterministically, read
immutable bundled receipts, and persist project-local receipts by content.

No generator flag, `closed` spelling, evidence claim ID, probe observation,
caller-created digest, or receipt document can construct accepted typestate.
Phase 12 still owns validation of a stored receipt before exposing accepted
semantics to analyzer consumers.

## Proof families and authority

Semantic-model-v1 closure requires all eighteen families for every local
closure claim:

1. package identity;
2. manifest/entrypoint identity;
3. exact export resolution;
4. runtime artifact/declaration identity;
5. runtime/declaration export identity;
6. module/dependency closure;
7. selected signature;
8. actual-to-formal argument binding;
9. rest/tuple-spread coverage;
10. callable-path identity;
11. operation reachability;
12. operation cardinality;
13. recursive value shape;
14. guard partition;
15. compiler lowering/source-site reconciliation;
16. accepted dependency composition;
17. domain exhaustiveness; and
18. probe consistency.

The checker assigns each family to exactly one authority class. Package,
manifest, resolution, artifact, export, and closure rules use package-artifact
authority. Signature, binding, spread, callable path, operation,
recursive-shape, guard, and exhaustiveness rules use Type Facts. Compiler
reconciliation uses compiler execution facts. Dependency composition uses
accepted dependency contracts. Probe consistency uses runtime-probe authority.
A replay with the wrong authority is rejected.

Each replay binds bounded raw transcript bytes, the open semantic digest,
semantic claim ID, package/manifest identity, selected artifact case,
runtime/declaration/transform/closure identity, and exact runtime/declaration
export identity. It requires an explicitly complete finite census, equality of
canonical enumerated and classified site sets, and no unresolved aliases,
spreads, escapes, dependencies, compiler sites, guard branches, dynamic
loaders, or recursive children. Empty censuses are valid only as explicit
complete-empty rule results; omission is never closure.

## Local closure and contradictions

The backend obtains closure subjects only from the Phase 8
`ProposalPlan::closure_candidates` for the selected artifact case. Naturally
unknown claims and positive operation subjects are not closure candidates.
The semantic checker rejects:

- a missing or duplicate proof family;
- stale semantic or artifact scope;
- an orphan proof or wrong artifact case;
- incomplete or differently classified censuses;
- any unresolved premise;
- a fact authority mismatch;
- an operation claim presented as a closed domain;
- a verifier policy downgrade; and
- any Phase 10 contradiction whose claim ID is being closed.

After every replay succeeds, the checker closes only the exact named
`KnowledgeSet` leaf. Complete-positive candidates retain their known items and
become complete. Complete-negative candidates become complete empty. Recursive
tuple, object, choice, capability, resource, owner-production, guard, and call
siblings are untouched. The complete normalized model is validated again, so
contradictions, dangling references, invalid guards, cycles, and impossible
capabilities remain refusals.

## Canonical receipt rules

Acceptance receipt version 1 is computed locally. It contains:

- the SHA-256 of the exact main wire bytes supplied for this issuance;
- semantic-model version and the finalized normalized semantic digest;
- an artifact root over exact package, manifest, selected artifact,
  declarations, transform, and every runtime/declaration export target;
- the selected canonical dependency-closure digest;
- a proof root over sorted family, authority, claim, scope, transcript, and
  census roots;
- a closed-claim root over sorted semantic claim IDs; and
- exact verifier build and proof-policy identity.

All hash domains are typed and length-delimited. Input proof order, census
order, and duplicate census entries do not change a receipt. Changing raw
transcript bytes, a semantic value, an artifact/export identity, closure,
family, authority, claim, verifier policy, or wire bytes does.

## Receipt storage

`BundledEvidenceStore` remains an immutable compiled map and rehashes receipt
bytes on every read. `LocalEvidenceStore` now implements `ReceiptStore`: it
computes the content key, treats a matching repeated write as idempotent,
writes through a unique temporary file, atomically renames it into the
`receipts/` namespace, and rehashes persisted bytes before returning. Receipt
JSON encoding is deterministic compact JSON plus one trailing newline.

## Focused and adversarial tests

The new focused tests cover:

- deletion of each of all eighteen proof families;
- incomplete, unresolved, differently classified, and wrong-authority rule
  inputs;
- stale semantic/artifact replay;
- exact probe contradiction consumption;
- attempted operation-as-closure promotion;
- verifier-policy downgrade;
- recursive leaf closure without sibling contamination;
- proof/census input-order normalization and deterministic proof,
  closed-claim, and semantic roots;
- the complete Phase 8 plan to Phase 11 acceptance adapter;
- local receipt persistence, content rehashing, canonical keys, and idempotent
  writes; and
- deterministic receipt encoding with every required field.

Existing Phase 6-10 adversarial suites continue to cover generator-added false
closure, omitted graph operations, invalid cardinality and guards, stale and
cross-artifact sidecars, changed dependency closure, wrong probe artifact/mode,
multiple artifact matches, invalid compiler/probe lifecycle records, and
recursive semantic contradictions.

## Type Facts, compiler facts, and generated artifacts

No Type Facts producer, Rust client, schema/protocol, build identity, fixture,
or normalized consumer changed. No Solid compiler fork, compiler semantic
trace, compiler-facts protocol, compiler pin, identity document, notice, or
conformance report changed.

No public wire schema, evidence-sidecar schema, temporary main document,
bundled contract, fixture snapshot, dialect manifest, runtime lock, binary, or
other generated artifact changed. Receipt bytes exist only in focused tests;
Phase 14 owns bundled-contract regeneration.

## Verification

Focused iteration:

- `cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-reactive-ir --lib contract_semantics::proof`
  — 10 passed;
- `cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-facts-backend --lib proposal_generation::tests::phase11_adapter`
  — 1 passed;
- armed `solid-facts-backend --test contract_interface` — 8 passed;
- focused IR/backend Clippy with `-D warnings` — passed;
- `git diff --check` — passed.

Final `make verify` results are recorded before PR handoff.

## Exact remaining open or uncertifiable cases

- Phase 12 must validate stored proof-issued receipts and migrate analyzer
  consumers to accepted normalized semantics. The current loader still refuses
  analyzer exposure after decoding/normalization.
- Phase 13 must execute the exact published Solid 2 RC.3 proof/probe corpus and
  attach conformance evidence for every matrix row. This phase implements the
  proof authority but does not claim that absent corpus transcripts exist.
- Phase 14 must switch public generators/verifiers/probes, regenerate bundled
  contracts and receipts, and perform the temporary-v2 to stable-v1 atomic cut.
- Incomplete Type Facts censuses, unresolved aliases or unknown-length spreads,
  unknown escapes, incomplete callable paths, unreconciled compiler sites,
  overlapping/non-exhaustive guards, unknown recursive children, and runtime
  probe contradictions remain open at the exact claim.
- Nonliteral dynamic loading, `eval`, native code, opaque WASM, mutable unbound
  globals, unmaterialized transforms, and unaccepted dependency edges remain
  artifact-local fail-closed frontiers.
- Experimental server-component behavior and runtime modes without an exact
  RC.3 transcript remain uncertifiable; unrelated proved facts remain usable.

No upstream Solid pull request is required or authorized for this phase.

## Handoff

- Branch: `codex/phase11-proof-checker-receipts`
- Commits: recorded after green verification
- Pull request: recorded after branch push

No upstream Solid pull request was created.
