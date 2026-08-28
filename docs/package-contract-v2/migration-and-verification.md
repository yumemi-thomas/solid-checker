# Migration and verification strategy

## Migration rule

The repository does not ship a dual public decoder. Development occurs with
the replacement main document at temporary `schemaVersion: 2`. Every producer,
consumer, bundle, fixture, report, sidecar, receipt, cache, and gate converges
on that format before one atomic commit re-emits it as stable
`schemaVersion: 1`.

Because legacy and new formats both eventually use version 1, the replacement
has a required `format` discriminator and the stable cut removes every legacy
decoder and artifact. No build may accept both meanings of version 1.

Phase 14 completed the temporary-v2 producer/consumer cut on 2026-08-28. The
inventory below remains the audit ledger, including retired filenames: the old
JavaScript document/closure/verification helpers and Rust generator were
deleted, their temporary-v2 replacements are live, native and WASM consumers
accept only receipt-issued normalized inputs, and both bundle locations and all
fixtures were regenerated. The stable `2` → `1` renumber remains Phase 17 and
must still be atomic.

Phase 15 completed the adversarial gate on 2026-08-28. All contract JSON
families now share explicit byte, depth, node, and string bounds; file-backed
catalog, document, and receipt inputs are bounded before allocation. The seeded
mutation suite must reject every false-closure mutation, and any fuzz input that
survives decode and normalization must re-encode deterministically to identical
semantics. The stable-version cut remains unchanged.

## Document-version namespaces

The following are independent and must never be changed by a global
`schemaVersion` replacement:

- main package-contract wire version;
- semantic-model version;
- evidence-sidecar version;
- probe-plan version;
- probe-report version;
- proof-transcript version;
- acceptance-receipt version;
- Type Facts schema/protocol;
- compiler execution-facts protocol;
- cache format version.

Every document family receives a format/media-type discriminator.

## Migration order

1. Capture the reproducible legacy baseline.
2. Port the existing semantic trace to a semantic-only fork of
   `solidjs/solid#next` and prove zero compiler-output and checker-finding delta.
3. Switch Solid 2 to the exact Solid fork revision; retain the Solid 1.x fork.
4. Import Type Facts into this repository and prove producer/client/checker
   parity before adding facts.
5. Freeze semantic terminology and invariants.
6. Implement normalized model and semantic digest without changing consumers.
7. Add required Type Facts demands locally with their checker consumers.
8. Add compiler execution-facts protocol 2 as semantic-only instrumentation and
   move the exact Solid fork revision.
9. Implement temporary wire v2 private types and cross-field validation.
10. Implement exact artifact-case resolution and closure identity.
11. Refactor generation to emit open proposals and proof obligations.
12. Implement sidecars, semantic claim IDs, and runtime event probes.
13. Implement proof checker, finalizer, and receipts.
14. Switch ordinary analysis to accepted normalized contracts.
15. Migrate all generators, probes, verifiers, review tools, and WASM adapters.
16. Regenerate bundles and fixtures from exact authorities.
17. Delete legacy normalization and compatibility paths.
18. Complete the Solid 2 RC.3 conformance matrix.
19. Run corpus, mutation, TypeScript-oracle, compactness, and performance gates.
20. Require a repository-wide version-2 convergence audit.
21. Freeze semantic model version 1.
22. Atomically re-emit the wire format as stable version 1.
23. Reissue every acceptance receipt and run the full release authority with
    caches disabled.

## Producer and consumer inventory

The migration audit must include at least:

- `schema/solid-reactivity.schema.json`;
- `packages/cli/scripts/generate-package-contract.mjs`;
- `packages/cli/scripts/contract-document.mjs`;
- `packages/cli/scripts/probe-contract.mjs`;
- `packages/cli/scripts/contract-probe-driver.mjs`;
- `packages/cli/scripts/contract-probe-worker.mjs`;
- `packages/cli/scripts/verify-contract.mjs`;
- `packages/cli/scripts/contract-verification.mjs`;
- `packages/cli/scripts/review-contract.mjs`;
- `packages/cli/scripts/runtime-module-closure.mjs`;
- Rust `solid-contract-gen`;
- Rust document decoder and normalized model;
- contract import resolution and IR consumers;
- CLI validation;
- WASM types and host-provided Type Facts closure;
- bundled contracts in both physical locations;
- dialect manifests and runtime locks;
- backend process fixtures;
- generated fixture contracts;
- contract corpus, closure, pin, differential, review, probe, and obligation
  scripts;
- gate/cache identities;
- local Type Facts producer, Rust client, shims, schemas, build manifest, and
  release packaging;
- exact Solid compiler fork revision, semantic trace version, and upstream-base
  ledger;
- package-contract and RFC documentation.

## Sidecar collision hazard

The existing Type Facts probe-construction sidecar already uses
`schemaVersion: 2`. Add a document-kind discriminator before the main format
migration. Readers must select by document kind and version together.

## Hash and cache policy

Any change to wire bytes invalidates the wire digest. Any semantic change
invalidates the semantic digest. Any artifact, closure, fact transcript, proof
policy, or verifier change invalidates the receipt.

The temporary `2` to stable `1` renumber may reuse expensive raw observations
only when semantic, artifact, closure, tool, and policy identities match. It
always emits fresh main bytes and a fresh receipt.

All gate caches and registry memos bump their format identity before the new
decoder can run. Bundled `include_bytes!` artifacts require a fresh native
build.

## Focused verification cadence

Run one owning check per semantic slice, then the proportional handoff checks.
Do not run parallel Cargo processes or repeat unchanged commands.

### Type Facts changes

- local Go/Rust model and adapter tests;
- producer/client round trip;
- retained-session and stale-generation tests;
- `facts-lib`;
- focused contract-generation fixtures.

### Compiler execution-fact changes

- compiler fork semantic-trace tests;
- trace-on/trace-off and fork/upstream zero-output-diff gates;
- fork-scope audit rejecting non-semantic compiler patches;
- transform fact reconciliation;
- both dialect adapter tests where protocol structure is shared;
- `facts-lib` and `ir-lib`;
- focused JSX/SSR/server-function process fixtures.

### Contract model and analyzer changes

- `ir-lib`;
- armed contract process tests;
- exact contract generation and verification tests;
- coverage comparison using a fresh debug binary;
- ownership gate when contracts can change findings.

### Wire/document-only changes

- JSON Schema validation;
- encoder/decoder golden tests;
- semantic round trips;
- `git diff --check`;
- contract process tests.

## Universal handoff set

```sh
cargo +1.97 fmt --manifest-path rust/Cargo.toml --all -- --check
git diff --check
jq empty schema/solid-reactivity.schema.json
node scripts/dialect-manifests.mjs validate
cargo +1.97 clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings
```

Fixture-driven process tests must set:

```sh
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts"
```

After Rust or bundled-contract changes, coverage and ownership runs must point
at the fresh debug checker binary. Do not infer success from a stale checked-in
binary.

## Accuracy gates

- Every closed domain maps to an accepted proof record.
- Removing one reachable operation from a closed domain fails verification.
- An unresolved call or escape opens only affected domains.
- A probe never creates complete-negative knowledge.
- A probe plan admits only possible-positive operation witnesses and closure-
  candidate falsifiers from the exact proposal.
- Every mode uses fresh process, realm, and module state and deterministic
  bounded repeat runs; disagreement refuses only that exact mode.
- A timeout bounds execution but elapsed time never classifies behavior.
- Zero or multiple artifact-case matches fail closed.
- Inline and summarized wire forms normalize identically.
- Every contract-driven diagnostic has a `tsc`-silent witness against exact
  published declarations.
- Generator/probe disagreement never selects the friendlier result.
- Experimental absence never means stable.

## Adversarial gates

Run mutation and fuzz suites for:

- omission and false closure;
- cardinality promotion;
- recursive leaf contamination;
- guard overlap and uncovered remainder;
- resolver precedence and custom conditions;
- artifact/declaration/closure mismatch;
- stale or cross-package sidecars;
- stale fact generations;
- unreconciled compiler sites;
- reference cycles and resource exhaustion;
- mixed-framework artifact substitution.

All seeded false-closure mutations must be detected.

## Corpus and automation gates

- Preserve every legacy row whose semantics remain valid under RC.3.
- Verify at least 85% of installable/generatable corpus rows.
- Verify at least 90% of Solid Primitives rows.
- Complete all sixteen Solid 2 conformance categories.
- Provide stable refusal reasons for every open domain.
- Never increase coverage by treating an unproved negative as closed.

## Compactness and performance gates

Measure canonical minified main contract, pretty main contract, normalized
semantic graph, and sidecars separately. Report p50, p95, maximum, bytes per
export, and bytes per operation.

Ordinary analysis must remain linear in selected document size plus normalized
operation edges. It must not execute package code, read sidecars, or access the
network.

## Version-2 convergence audit

Before renumbering, prove:

- every main producer emits temporary version 2;
- every main consumer reads only temporary version 2;
- every bundle and fixture uses temporary version 2;
- all sidecars and receipts bind version-2 wire bytes;
- no legacy decoder, sentinel, condition matcher, or JavaScript semantic
  normalizer remains;
- every cache format has moved;
- documentation consistently describes version 2 as temporary;
- the complete clean-cache authority passes.

## Atomic stable cut

In one change:

1. Change the main schema constant from 2 to 1.
2. Change every main producer and consumer together.
3. Re-emit all main contracts.
4. Recompute wire hashes.
5. Reissue all receipts.
6. Refresh bundles, manifests, locks, fixtures, and caches.
7. Update public documentation.
8. Verify there is no temporary-v2 or legacy-v1 main document.
9. Rebuild binaries containing bundled contracts.
10. Run `SOLID_CHECKER_GATE_CACHE=0 make verify` plus the complete contract and
    ecosystem corpus.

There is no `schemaStatus` before, during, or after the cut.

## Definition of done

- One deep module owns wire interpretation and normalization.
- The generator cannot certify closure.
- Every negative has replayable proof.
- Unknown knowledge stays local.
- Exact artifact resolution selects semantics.
- Normal analysis needs only contract plus receipt.
- Type Facts and compiler execution facts close their intended gaps.
- Solid 2 RC.3 conformance is complete to the documented refusal boundary.
- TypeScript-owned errors do not produce checker diagnostics.
- Coverage and compactness targets pass without weakening accuracy.
- Stable public schema version 1 is emitted atomically from the fully migrated
  temporary version 2 implementation.
