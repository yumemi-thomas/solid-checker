# Solid RC.7 and rule-driven package contracts: implementation plan

- Date: 2026-09-09.
- Status: implementation authorized; worktree bootstrap and RC.7 audit are the first work package. The complete migration is not yet implemented.
- Target order: Solid 2.0 RC.7, Solid Primitives for Solid 2.0, then a wider ecosystem selected by concrete rule demand.
- Design basis: [rule-demand investigation](2026-09-09-rule-demand-proposal.md), especially its field-level inventory, certainty gaps and §1.5's missing third-party consumers.
- Working branch: `codex/solid-rc7-rule-contracts`.
- Intended worktree: `/Users/thomas/Documents/Github/solid-checker-rc7-contracts`.

## 1. Product and correctness requirements

Keep all 44 current rule identities and their existing configuration surface. Preserve Solid 1.x as a distinct dialect with regression coverage while new development concentrates on Solid 2.0. Changes in the published RC.7 runtime or types can change a rule's applicability; every such change requires evidence and an explicit ledger entry, not a silent snapshot replacement.

Third-party primitive misuse is a central acceptance criterion. Existing rules must recognize the reactive values, callback execution, reads, writes/actions and ownership behavior they need across exact package boundaries and local wrappers. The current consumer's missing capability is not a reason to classify that capability as unnecessary.

The developer-facing flow should automatically select and reuse accepted facts for the exact installed artifact. It should report a concrete misuse with its relevant execution/value/owner explanation. Authoring proof transcripts remains maintainer tooling. Missing evidence is an explicit uncertifiable result; neither fewer diagnostics nor an accepted package binding alone establishes safety.

Non-negotiable requirements:

1. Never duplicate a TypeScript diagnostic for the same claim against the real published typings, including diagnostics present only with strict checking.
2. Preserve exact package version, integrity, runtime/declaration selection, dependency closure, acceptance policy and dialect bindings. Moving tags are discovery aids, never pins.
3. Preserve the subject of a fact: selected parameter/member path, guard alternatives, operation cardinality, tracking/owner context and relevant capability.
4. Missing evidence never proves an operation safe, unsafe, absent or guaranteed. Partial positive knowledge is not domain closure.
5. Keep the semantic owners in [the architecture](../../rust/ARCHITECTURE.md) explicit. Shared code does not infer dialect vocabulary.
6. The compiler fork remains semantic-facts-only. Port observations onto upstream behavior without modifying lowering, generated output, runtime or diagnostics.
7. Preserve existing sound detections; identify and correct existing overclaims explicitly. An unsupported case remains uncertifiable until the required proof exists.
8. No automatic removal of `writes`, `invalidates`, `throws` or `disposals`: first determine whether a missing consumer is necessary for the required third-party extension of an existing rule.

## 2. Worktree and baseline procedure

The source checkout is on `codex/phase19a-authenticated-proof-policy` at `6b687543d3d0ef8109712e2601aec99776dd319b`, with substantial modified and untracked source, fixtures and audit documents. A new checkout of HEAD alone would omit the system that was investigated.

Bootstrap procedure:

1. Record HEAD, branch, tracked changes, untracked nonignored paths and the original index digest. Build a manifest of source bytes and file modes. Include the proposal and this plan.
2. Create the new branch/worktree without changing the original branch, index or source files.
3. Materialize the current tracked source state and nonignored untracked files in the new worktree. Preserve deletions and symlinks exactly. Exclude other worktrees, ignored dependency installations and build outputs from this source transfer.
4. Retain a manifest, the original diff and a recoverable source snapshot in worktree administrative storage. Label it an uncommitted baseline, not a verified release or green commit. Confirm that the source did not move during capture and that the destination matches the manifest.
5. Keep baseline changes distinguishable from new migration changes using the manifest. Do not stage or commit the inherited work wholesale as a migration change.
6. Record ignored prerequisites separately: `bin/solid-typefacts` and its stamp, package dependencies, oracle installs and certification harness inputs. Reuse only verified exact inputs; do not point writable build/output directories into the original checkout.
7. Build through the repository Make targets when an analyzer binary is needed. A source snapshot does not establish that a copied executable matches it.

The first turn should finish this bootstrap and begin a real RC.7 artifact/rule audit. It should not perform an unreviewed global version substitution or start the entire ecosystem benchmark.

## 3. Work packages and exit criteria

### P0 — Establish the reproducible working baseline

Deliverables:

- This plan and the investigation available in the new worktree.
- A recoverable, hash-recorded snapshot of the inherited source state.
- A bootstrap report naming paths, source identity, ignored prerequisites, checks run and outstanding work.
- A rule identity inventory retaining all 18 Solid 1.x and 26 Solid 2.0 rules.

Exit criteria: original source/index preserved; worktree source transfer verified; no inherited source silently omitted; new migration edits identifiable against the captured baseline.

### P1 — Audit and align Solid RC.7

#### P1a: artifact and change inventory

Acquire the exact published `solid-js`, `@solidjs/signals` and `@solidjs/web` RC.7 artifacts in an isolated audit directory, recording registry metadata, tarball integrity and content identity. Record the matching upstream compiler release/source commit. Inspect the published declarations and exports for client/server/development/production cases that affect rules. Do not run package lifecycle scripts merely to inspect an artifact.

Compare against the existing RC.3 oracle/audited artifacts and the older premises still cited by the built-in runtime model. At minimum inspect:

- effect compute/apply forms, returned cleanup and leaf ownership;
- signal/store/projection/optimistic overloads, returned brands and mutability;
- async reads, Loading ownership and streaming behavior;
- server versus client execution and development export conditions;
- refs/directive application, events and compiler-controlled callbacks;
- refresh/affects/action target requirements;
- module/server-function transformation and transport premises;
- removed, changed and added exports that intersect existing rules.

Classify each difference as: irrelevant to current rules, declaration-only premise change, runtime behavior change, compiler execution fact change, artifact-selection change, or unresolved. Record the owning rule and exact source evidence. Release notes identify audit leads; authenticated artifact bytes and compiler behavior establish the premise.

Initial sources: [Solid RC.7 release](https://github.com/solidjs/solid/releases/tag/solid-js%402.0.0-rc.7), `fixtures/tsc-oracle/packages.json`, `rust/crates/solid-dialect/src/solid_2.rs`, the v2 export tables, `rust/Cargo.toml`, and [compiler bootstrap constraints](compiler-and-typefacts-bootstrap.md).

Exit criteria: exact artifacts recorded, rule-relevant delta ledger written, and no RC.7 compatibility claim based solely on a version number or release notes.

#### P1b: rule fixtures against real RC.7 types and behavior

Retain existing fixtures and add focused version-differential cases for changed premises. Each case records exact package/artifact/condition, TypeScript strict and loose results, expected rule identity/finding kind, and runtime/compiler evidence when relevant. Do not weaken stubs to manufacture a checker finding.

Use a separate RC.7 audit/provisioning location until promotion is justified; changing the existing oracle pin before auditing it would erase the comparison baseline. A positive rule fixture must not merely exercise an obsolete accepted-contract rejection path.

Exit criteria: every existing v2 rule has an RC.7 disposition—verified applicable behavior, evidenced applicability change, or named unresolved premise. All v1 identities and their differential behavior remain intact. Retaining a rule does not require reporting a case TypeScript now rejects.

#### P1c: compiler facts and coordinated pin promotion

Record the exact current fork and upstream base. Port only the required semantic trace hooks to the RC.7 upstream compiler if the existing fork cannot establish its execution facts. Compare generated output/source maps to the exact upstream base and verify trace completeness. Missing facts stay open; compiler behavior changes must be upstream-owned.

Promote runtime/type pins, audited byte slices, dialect export tables, compiler revision, protocol assertions and relevant fixture expectations together once justified. Regenerate only artifacts owned by that change. Do not reuse old receipts as authority for new artifact bytes.

Exit criteria: RC.7 rule results are based on matching runtime, declarations and compiler facts; focused dialect/contract process tests, conformance, oracle and ownership/coverage checks pass; the change has a full `make verify` handoff. Record every precision change.

### P2 — Establish the new package-consumer path early

Begin one small P2 tracer while P1 is underway. Core uses the built-in runtime model, so a green core audit alone cannot validate external accepted-contract consumption.

First tracer: an exact external package returns a reactive value or selected member, application code carries it through a local helper, and an existing rule detects a type-valid misuse. Include a valid counterpart and an unresolved or mismatched-artifact counterpart. Choose an available exact artifact with evidence sufficient for the selected relationship; do not promise a currently open composite return can be certified merely because one member is known.

Implement a narrow query over the normalized contract model. Prefer adapting the existing accepted-use instantiation machinery. Keep exact call/export binding, returned/member subject, all relevant alternatives and claim knowledge. The verifier derives proof demands for the selected semantic candidate; the caller never chooses which necessary evidence to omit.

Introduce consumer demand records sufficient to explain: rule/site, exact artifact/export, requested predicate/subject, evidence used, and discharged/uncertifiable/inapplicable outcome. This is measurement and attribution, not caller-created proof authority.

Subsequent slices:

| Slice | Required predicate | Principal current gap | Acceptance example |
| --- | --- | --- | --- |
| Returned reactive value/member | Exact reactive/store identity and relevant capability on the selected return path | Open composites rejected; paths and capabilities lost | Existing accessor/store rule detects external misuse through a wrapper |
| Parameter/direct read | Read of the exact actual value/member in the relevant execution | Parameter paths lost downstream | External read participates in an existing untracked-read proof |
| Callback execution | Exact actual callback, possible/guaranteed execution, exhaustive timing when required, tracking/owner context | Partial rows can be treated as exhaustive; rich instantiation unused | Existing execution rule distinguishes inline, deferred and unresolved cases |
| Caller owner requirement | Operation requires an owner the callee does not supply | Partial requirements exist, but context/operation certainty must survive | `missing-owner` detects an ownerless external call and accepts owned usage |
| Returned setter/action or helper effect | Exact write/action capability and relevant execution context | Current return projection recognizes mainly accessors/stores | Applicable write/action rule recognizes a third-party primitive |
| External helper in a leaf scope | Required owner/cleanup effect under the leaf's restrictions | Unknown helper or accessor identity is insufficient effect proof | Leaf rule diagnoses a proven forbidden operation; unknown effect stays uncertifiable |

Exit criteria per slice: actual accepted artifact, type-valid positive finding, clean negative, explicit uncertainty control, exact identity mismatch control, and no certainty lost at the consumer boundary. Explain any changed finding against the baseline. Do not call SC9011 changing into SC9005 a capability gain.

### P3 — Solid Primitives for Solid 2.0

Use the project's [2.0 branch](https://github.com/solidjs-community/solid-primitives/tree/next) to discover candidates, then resolve exact published versions and verify their compatibility with RC.7. Do not trust the namespace or moving dist-tag. Every package retains its own artifact and dependency-closure identity.

Select an initial set by behavioral diversity and actual rule demand: returned accessors/stores; callback scheduling; owner/cleanup requirements; ref factories; and async sources. Start with available artifacts that exercise the P2 predicates, then expand across the package family. This is sequencing, not a permanent package whitelist.

For every selected primitive:

1. Identify the exact export/case/signature and applicable existing rules.
2. Add misuse, correct-use, unresolved and TypeScript-owned cases against real declarations.
3. Acquire only the required behavior and its proof dependencies. A dependency's negative creates premise becomes demanded when composition needs it.
4. Exercise the automatic developer-facing analysis path with the accepted publication.
5. Record enabled rule conclusions, unresolved predicates and acquisition/analysis cost separately.

Exit criteria: representative primitives across these behavioral families produce correct existing diagnostics without manual contract configuration; another package using an already-supported behavior can reuse the same machinery; no name-based trust or false clean result is introduced.

### P4 — Expand and remove demonstrably unnecessary work

Add packages from actual consumer use and the retained ecosystem corpus. Rank by rule conclusions enabled and repeated implementation subjects, not total export count or namespace popularity. Keep exact artifact/case bindings even when implementation evidence can be reused under identical premises.

After P2/P3 establish target demands, decide domain by domain what acquisition, representations or tests are truly unnecessary. Stable schema-1 decoding, canonical hashing and receipt policy remain compatible; physical model/wire removal requires its own versioned decision. Remove redundant old consumer paths only after their required behavior has a tested replacement.

Keep acquisition/certification fixtures, application consumer fixtures and a broader corpus as complementary layers. Consolidate obsolete-policy rejection fixtures only after restoring their intended positive consumer coverage. No blanket corpus or rule deletion is authorized by a U cell in the original inventory.

Exit criteria: increased supported misuse detection with preserved precision, explicit unsupported cases, and less irrelevant acquisition/maintenance work demonstrated on recorded examples.

## 4. Verification and commit discipline

Use `.claude/skills/add-fixture/SKILL.md` and `.claude/skills/verify-handoff/SKILL.md`. Inspect dirty state and owning diffs before each slice. Keep source and changed snapshots together. Each implementation commit must be independently green; no broad staging or cleanup of inherited changes.

During iteration:

- Type/AST facts: the owning facts/Type Facts tests and the exact strict/loose published-type case.
- Contract model/query/IR: focused `solid-reactive-ir` tests plus the selected accepted consumer fixture.
- Artifact/diagnostic boundary: armed focused backend process tests with `SOLID_TYPEFACTS_BIN` set.
- Certification/probes: use Make targets carrying certification pins; an unarmed probe test is not evidence.
- Fixture changes: compare before updating the exact snapshot; run coverage with the fresh debug checker and the required dialect-stub checks.
- Run one Cargo process at a time. Avoid rebuilding unchanged binaries or repeating unchanged checks.

Build analyzer binaries using `make build-checker-debug` or `make build-checker-release`; do not overwrite the packaged binary merely to test source. Preserve the distinction between `SOLID_CHECKER_BIN` for coverage and `SOLID_CHECKER_NATIVE_BIN` for the CLI launcher.

At handoff, run the universal format/diff/schema/manifests/workspace-Clippy checks and the proportional semantic checks. A pin promotion/shared dialect/architectural implementation requires `make verify`. A documentation/artifact-inventory slice does not establish analyzer correctness; label its narrower checks accurately.

Do not rerun the full ecosystem benchmark for an answer already present in retained audits. Use selected real-package runs during implementation, and a full ecosystem measurement only at an explicit expansion/performance checkpoint with recorded binaries, versions, cache state and scope. Keep findings, uncertifiable results, authentication, closure and timing denominators distinct.

## 5. Progress and next actions

- [x] Design investigation and 44-rule demand inventory completed.
- [x] Product requirements clarified: retain rules, third-party misuse, precision, RC.7-first rollout.
- [x] Detailed implementation plan written.
- [ ] P0: worktree created, inherited source copied and baseline verified.
- [ ] P1a: RC.7 exact artifacts acquired and initial rule-relevant delta recorded.
- [ ] P1b: focused real-type/runtime/compiler cases added and exercised.
- [ ] P1c: compiler alignment and coordinated pin promotion verified.
- [ ] P2: first accepted external-package consumer path implemented and verified.
- [ ] P3: representative Solid Primitives 2.0 coverage established.
- [ ] P4: wider ecosystem expansion and justified deletions measured.

Immediate execution: complete P0, then P1a. Choose the first P1b/P2 fixture from the observed artifacts and missing consumer predicates. Record findings and blockers in the worktree rather than silently substituting newer packages or an older compiler model.
