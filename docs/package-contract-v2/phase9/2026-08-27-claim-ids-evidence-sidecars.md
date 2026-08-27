# Phase 9 completion report: claim IDs and evidence sidecars

Date: 2026-08-27

Branch: `codex/phase9-claim-ids-evidence-sidecars`

Authority: semantic-model version 1, the Phase 6 temporary main document, the
Phase 7 exact artifact/closure seam, the Phase 8 open proposal plan, and the
frozen rule that evidence integrity alone cannot accept closure

## Result

Phase 9 items 101-107 are implemented. The normalized semantic model now owns
position-independent semantic claim IDs. The backend owns two separate,
strict, deterministic evidence document families and validates both directions
of their binding to the temporary main contract.

Evidence is auditable and content-addressed but remains non-authoritative.
`AcceptedContract` still has no public constructor, no receipt is issued, and
no sidecar outcome closes a claim. Raw evidence terminates at the validation
seam and is absent from the ordinary analysis interface.

## Semantic claim IDs

The canonical spelling is:

```text
claim:v1:sha256:<64 lowercase hexadecimal digits>
```

The version-1 hash is typed and length-delimited. It includes:

- the claim-ID domain and claim-ID version;
- semantic-model version;
- exact package name, version, integrity, and manifest artifact;
- artifact-case ID, entrypoint, ordered resolution trace, runtime artifact,
  declarations, dependency closure, and optional transform;
- exact runtime and declaration export identity;
- one typed semantic subject path.

Subject paths distinguish:

- every immediate call claim domain;
- every recursive value root, child path, and local value domain;
- every operation axis;
- every resource domain;
- the guard partition; and
- positive existence of one exact normalized operation.

Before hashing, `NormalizedContract::claim_id` proves that the artifact case,
export, operation, resource, value root, recursive child, and claim domain
exist. An orphan path cannot receive an ID.

The hash excludes JSON position, summary name, formatting, sidecar layout, and
unrelated semantic values. Summary renaming and wire reformatting preserve the
ID. Package, artifact, closure, transform, export, or subject changes produce a
different ID. Changing an unrelated claim on the same exact export does not.

Phase 8 proposal emission now includes these IDs for closure candidates,
unresolved edges, positive operations, proof-obligation subjects, and probe
subjects. It remains explicitly `unaccepted`.

## Evidence document families

The checked-in structural schema
[`schema/solid-reactivity-evidence-sidecars-v1.schema.json`](../../../schema/solid-reactivity-evidence-sidecars-v1.schema.json)
defines two document kinds at independent `sidecarVersion: 1`:

| Family | `format` | Claim-local material |
| --- | --- | --- |
| Proof/fact | `solid-checker-proof-evidence` | artifact/closure binding, producer, fact-domain transcript digests, Type Facts generation where present, fact producer, proof-rule input digests, proof tool, and coverage limitations |
| Runtime probe | `solid-checker-runtime-probe-evidence` | artifact/closure binding, producer, recipe digest, runtime tool, OS, architecture, conditions, explicit sandbox kind/policy, outcome, and coverage limitations |

Probe outcomes can record planned work, a witness transcript, falsification
transcript, error detail, bounded timeout, or refusal reason. They are evidence
records only. Phase 10 still owns semantic event markers, mode matrices,
isolation, draining, repeatability, and probe authority enforcement.

Every document and claim records tool build identity. Fact transcripts also
record their exact fact domain and producer. Proof inputs record their rule,
input digest, and proof tool. Runtime records distinguish no sandbox from a
process, container, or virtual-machine policy; a claimed sandbox requires a
policy digest and an unsandboxed run cannot carry one.

Emission canonicalizes unordered transcript, proof-input, condition, and
limitation collections and produces pretty JSON with one trailing newline.
Input order therefore cannot change sidecar bytes or hashes.

## Bidirectional binding

The temporary main contract already carries `sidecars.proof.sha256` and
`sidecars.probes.sha256`. Phase 9 now preserves those references long enough
for evidence validation and requires the SHA-256 of the complete supplied
sidecar bytes to match.

Each sidecar binds back to:

- semantic-model version and semantic digest;
- exact package and manifest identity;
- exact artifact case, entrypoint, runtime, declarations, optional transform,
  and dependency closure for every claim;
- the recomputed semantic claim ID and typed subject.

The reverse binding uses normalized contract identity, not the main file hash.
Embedding the main hash in a sidecar while embedding the sidecar hash in the
main file would create an impossible mutual hash cycle. Phase 11 receipts will
bind the final main wire digest and proof root after evidence replay.

## Validation and refusal locality

`EvidenceCatalog` derives the only permitted proof and probe claims from a
normalized contract or Phase 8 `PlannedProposal`. Callers do not supply claim
IDs or artifact bindings during normal emission; the module derives them.

Validation refuses:

- a main contract whose normalized identity differs from the catalog;
- a referenced sidecar whose bytes are missing;
- supplied sidecar bytes with no main-document reference;
- content-hash mismatch;
- wrong document kind or sidecar version;
- stale or cross-package contract identity;
- cross-artifact, declaration, transform, or closure identity;
- a noncanonical or subject-mismatched claim ID;
- a claim absent from the relevant proof/probe plan;
- duplicate claims;
- nonexistent operations, resources, or recursive value leaves;
- empty proof rows with neither material nor a coverage limitation;
- invalid tool, environment, sandbox, timeout, digest, collection, string, or
  resource-limit data.

None of these failures becomes negative semantic knowledge. They reject only
the evidence document or exact claim material involved; structural main-
contract mismatch refuses the evidence set.

## Raw-sidecar-free ordinary analysis

`ValidatedEvidenceSidecars` retains ordered semantic claim IDs only. It does
not retain raw bytes, fact transcripts, proof inputs, probe recipes,
environments, or outcomes. The normalized contract continues to exclude
sidecar references entirely.

A focused test normalizes a main document containing proof and probe hashes,
deletes both raw sidecars, and obtains the identical semantic digest. Evidence
validation correctly reports the missing raw documents, while ordinary
contract normalization remains independent of them. After Phase 11 issues a
receipt, analyzer consumers therefore need only the main contract and receipt.

## Tests

Reactive IR tests cover:

- stability across unrelated semantic changes;
- distinction among claim paths;
- package and closure binding;
- invalid operation and recursive-leaf subjects;
- canonical claim-ID parsing.

Backend tests cover:

- summary renaming and wire reformatting equivalence;
- deterministic proof/probe bytes under reordered inputs;
- strict separation of proof and runtime material;
- explicit kind/version discriminators and checked-in schema constants;
- main-to-sidecar content hashes and sidecar-to-contract identity;
- successful validation with only claim IDs retained;
- ordinary normalization after raw sidecar removal;
- stale bytes, missing and unreferenced documents;
- cross-package and cross-artifact substitution;
- wrong claim IDs, unplanned claims, and nonexistent subjects;
- every non-authoritative probe outcome family.

## Type Facts, compiler facts, and generated artifacts

No Type Facts producer, Rust client, process/session protocol, build identity,
or proof fixture changed. No Solid compiler fork, compiler pin, semantic trace,
compiler-execution-facts protocol, identity document, notice, or conformance
report changed.

The new evidence-sidecar JSON Schema is a checked-in source authority. No main
contract, bundled contract, package-contract fixture, acceptance receipt,
evidence instance, analyzer snapshot, dialect manifest, binary, runtime lock,
or other generated artifact changed.

## Verification

Focused iteration:

- `cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-reactive-ir --lib`
  — 170 passed;
- `cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-facts-backend --lib`
  — 60 passed, including seven evidence-sidecar tests and proposal-plan
  catalog integration;
- focused Clippy for `solid-reactive-ir` and `solid-facts-backend` with
  `-D warnings` — passed;
- `jq empty schema/solid-reactivity-evidence-sidecars-v1.schema.json` —
  passed.

Final handoff authority:

- `make verify` — passed in 136.01 seconds;
- workspace tests and documentation tests — passed;
- coverage — 94 fixture projects and 557 findings matched;
- ownership gate — 289 cases passed and all 465 ledger rows are resolved;
- performance certification — passed;
- CLI — 7 test files and 60 tests passed;
- TypeScript oracle, obligation audit, compiler identity, dialect manifests,
  package-pin checks, and bundled-contract conformance — passed.

## Exact remaining open or uncertifiable cases

- Phase 10 must implement semantic event transcripts, exact mode matrices,
  isolation, bounded draining, timeout policy, repeat-run consistency, cleanup,
  AsyncIterable, transition, request, and root-lifetime probes.
- Phase 10 must enforce that probes add possible-positive witnesses or falsify
  closure only; Phase 9 records outcomes but does not interpret them.
- Phase 11 must implement proof replay, close authorized domains, compute proof
  and closed-claim roots, issue receipts, and construct `AcceptedContract`.
- Raw transcript digests identify material but do not themselves prove its
  contents. The relevant Phase 10/11 replay engine must consume and validate
  those transcripts.
- Ordinary analyzer consumers remain on the legacy path until Phase 12.
- Public generators, verifiers, workers, bundles, fixtures, WASM adapters, and
  schema migration remain unchanged until the Phase 14 atomic cutover.
- The temporary main schema remains version 2; sidecar version 1 is an
  independent document version and does not advance the stable main renumber.
- Existing open Solid 2 runtime, compiler emission-census, dynamic loading,
  native/WASM, mutable-global, transform, dependency, and experimental server-
  component domains remain open exactly as recorded by Phases 4-8.

## Handoff

- Branch: `codex/phase9-claim-ids-evidence-sidecars`
- Implementation commit: `7e162a62` (`feat(contracts): add claim-bound
  evidence sidecars`)
- Pull request:
  [yumemi-thomas/solid-checker#51](https://github.com/yumemi-thomas/solid-checker/pull/51)

No upstream Solid pull request was created.
