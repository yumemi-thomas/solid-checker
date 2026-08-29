# Phase 19 plan — authenticated proof policy 2 and evidence-driven refusal reduction

Date: 2026-08-29

Status: proposed

Review status: adversarially revised

Precondition: merge Phase 18 PR #60 and update from `origin/main`. Deliver the
authenticated policy cut as Phase 19A, then start refusal-reduction work as
Phase 19B from the merged policy-2 baseline.

## Outcome

Make package-contract acceptance independently check the evidence behind each
closed semantic claim, then reduce fail-closed refusals only by producing the
exact missing premises. Stable main `schemaVersion: 1` remains unchanged.

The work is successful when:

- caller-authored completeness flags or matching digest lists cannot create
  accepted closure;
- caller-supplied transcripts cannot impersonate a Type Facts, compiler,
  package-acquisition, dependency-receipt, probe, verifier, or issuer session;
- every accepted claim is reconstructed from exact package artifacts, Type
  Facts, compiler facts, and accepted dependency receipts as applicable, then
  subjected to every verifier-derived mandatory probe veto;
- every analyzer-visible positive fact is independently witnessed even when
  its surrounding domain remains partial; proof policy 2 does not secure
  complete-negative claims while letting fabricated possible behavior through;
- proof demands and inapplicability are derived by the verifier from the exact
  claim and artifact graph; callers cannot discharge irrelevant families with
  fabricated complete-empty censuses;
- certification reads one immutable, content-addressed artifact snapshot, so a
  path cannot change between demand planning, evidence production, and receipt
  issuance;
- receipt trust and proof-policy identity are explicit and replay-safe;
- every active policy-1 receipt document is either reissued under proof policy
  2 or removed from accepted discovery with an exact refusal;
- refusal reduction is measured by verified artifact cases, exports, and local
  claim domains unlocked, not by weakening the refusal percentage; and
- dynamic or unsupported behavior remains open at the smallest exact leaf.

This is not a plan to prove arbitrary JavaScript behavior. It is a plan to
automatically prove the bounded claim language for exact artifact cases and to
refuse everything outside that decidable subset.

## Current baseline

The Phase 16 corpus contains:

| Corpus state | Rows |
| --- | ---: |
| Ecosystem rows | 418 |
| Structurally complete proposals | 40 |
| Partial proposals | 318 |
| Full-row refusals | 60 |
| Generatable proposals | 358 (85.65%) |
| Fully proved live ecosystem proposals | 0 |
| Existing receipt-issued first-party artifact cases | 24 |
| Active receipt documents across bundles, fixtures, and authorities | 73 |

The 318 partial rows retain 1,458 artifact-case-local refusals. The 60 full-row
refusals are currently owned by:

- 20 accepted-dependency composition obligations;
- 14 unresolved export-kind censuses;
- 8 packages with no runtime ESM export surface;
- 2 missing exact package-export identities;
- 1 unresolved parameter-behavior case; and
- 15 unresolved or unsupported artifact shapes.

The existing 24 unique cases are represented by 73 active receipt documents.
They are accepted by the trusted checked-corpus workflow and remain useful
conformance authorities. They are not yet independently reconstructed from
authoritative evidence: `accept_checked_corpus_case` currently turns one corpus
digest into a complete census for every family. Phase 19 must delete that
shortcut and migrate all 73 active documents before scaling receipt issuance.

## Threat model, trusted computing base, and proof standard

Treat all proposal, proof-transcript, sidecar, receipt, package, registry,
lockfile, producer-output, probe-output, issuer-metadata, and catalog bytes as
untrusted input. Assume an attacker can reorder, omit, replay, truncate,
oversize, or replace them; mutate a filesystem path during certification;
substitute a producer executable; and copy a valid receipt through the wrong
provenance channel.

The trusted computing base is deliberately smaller than the evidence corpus:

- the exact Rust verifier build and proof-policy implementation;
- the exact shipped Node orchestration and probe-harness build, plus its pinned
  runtime identity, when the CLI path performs acquisition or launches probes;
- a pinned Type Facts producer and Solid compiler executable selected and
  launched by the verifier after executable/source-manifest and protocol/build
  handshakes;
- the operating-system primitives used to create the immutable artifact
  snapshot and the configured process sandbox;
- compiled built-in receipt bytes reached through the built-in bundle map; and
- issuer public keys and policy constraints loaded from configuration outside
  the analyzed project.

Arbitrary or replaced Node orchestration, package code, project files, proof
JSON, build strings, key IDs, and public keys carried inside a receipt are not
trust roots. The pinned shipped Node adapter is trusted only to acquire bytes,
launch exact processes, and transport transcripts; it has no semantic verdict
interface. Trust may originate only from:

- an acquired published-package archive whose bytes match an independently
  selected registry authority, or a distinctly identified lock-pinned/local
  artifact that makes no published-package claim;
- bytes and identities recomputed from one immutable extracted snapshot of that
  archive;
- a response read directly from the pinned Type Facts process launched for the
  certification session, with domain-specific completeness evidence;
- a semantic trace read directly from the pinned output-neutral Solid compiler
  session and bound to source, configuration, mode, compiler revision, and
  generated-output identity;
- a recursively validated accepted dependency contract for the exact import;
- a runtime-probe result produced from a verifier-derived plan in the selected
  sandbox and bound to its artifact, environment, producer, and repetitions;
  the result can veto closure but never prove absence; or
- an explicitly configured verifier trust root for receipt issuance.

A proof transcript may transport bounded audit witnesses. It may not declare
its own authority, completeness, classification, applicability, inapplicability,
producer session, issuer, or verdict. Serialized producer output is not closure
authority unless it carries an independently trusted producer attestation; the
normal local path reacquires it from a directly launched session. Human review
may select finite entrypoints, conditions, or approved probe recipes, but the
selection only narrows the named artifact case. It may not claim that
unselected entrypoints or conditions do not exist, or mark a semantic domain
closed.

The verifier must distinguish two threat models:

1. Local certification: evidence is recomputed by the locally pinned checker,
   Type Facts producer, and compiler during one snapshot-bound certification
   session. A persistent local receipt is trusted later only when authenticated
   by a configured local issuer key; "produced earlier on this machine" is not
   a trust primitive.
2. Portable receipts: a remote receipt is accepted only when its issuer and
   exact policy/build constraints chain to a configured trust root. A
   package-shipped receipt is not trusted merely because it is present in the
   package, even if it names a known key.

Resource exhaustion is part of the threat model. Archive bytes, expanded
bytes, file count, path depth, symlink/hardlink count, proof demands, witness
bytes, producer rows, process time, and process memory receive explicit policy
limits. Crossing a limit yields a typed resource refusal, never a semantic
negative.

## Deep certification modules and seams

Preserve the repository's ownership split rather than hiding process
orchestration behind a semantic trait. `solid-reactive-ir` owns the pure claim
applicability rules, proof-demand graph, witness verification, local closure,
and receipt payload. `solid-facts-backend` owns archive snapshot validation,
exact resolution, producer/probe session identity and transcript validation,
signing, and atomic publication. The pinned Node adapter owns download and
process/file lifecycle only; it does not classify semantics or construct proof
verdicts.

Keep the Rust-facing certification interface small and stateful enough to
cross that real seam:

```rust
pub fn plan_certification(
    request: CertificationRequest,
    artifact: UntrustedArtifactEnvelope,
) -> Result<CertificationPlan, CertificationRefusals>;

pub fn certify_artifact_case(
    plan: CertificationPlan,
    evidence: AcquiredEvidence,
    issuer: &mut dyn ReceiptIssuer,
) -> Result<CertifiedArtifacts, CertificationRefusals>;
```

`CertificationRequest` contains only the normalized open proposal, selected
artifact case, exact `ResolvedImport`, artifact provenance policy, requested
proof policy. Output formatting is not part of the semantic interface: policy-2
issuance uses one canonical stable-v1 encoding, re-decodes it before signing,
and binds those exact bytes. Human-facing pretty output is a separate audit
rendering and does not carry the canonical receipt.
`plan_certification` validates and snapshots the untrusted archive before
returning an opaque plan bound to the snapshot root, claim set, policy digest,
producer requirements, and resource budget. Callers cannot construct or edit a
`CertificationPlan`.

The open proposal is a set of untrusted candidates, not a lower bound the
certifier must preserve. Planning covers both candidate positives and proposed
closure. A positive operation, edge, cardinality, guard, capability, resource,
ownership/lifetime relation, or recursive value item needs an existence/value
witness even when its collection stays partial. An unproved positive is removed
or downgraded at the smallest structurally valid leaf; if references make that
impossible, the artifact case is refused. Closure additionally requires the
complete applicable census. No analyzer-visible proposal fact rides into an
accepted contract merely because it was not a closure candidate.

Before deriving demands, the certifier walks the normalized artifact case
itself, withdraws every caller-authored complete domain into a closure
candidate, and enumerates every positive fact. It does not trust a supplied
proposal plan to be a complete list. A closed or positive semantic path that
policy 2 cannot address precisely is a typed refusal, not retained knowledge.

The opaque plan is an in-process capability, not a serialized authority. The
CLI keeps one Rust certification session alive while Node satisfies its
demands. If certification must resume in a new process, Rust reopens and
revalidates the snapshot and rederives the complete plan from the original
request; it never trusts a caller-returned plan document or nonce as proof that
the demand set is complete.

`AcquiredEvidence` is not a public bag of family names and booleans. Its typed
entries are created only by backend adapters that bind direct producer-session
responses, compiler runs, authenticated dependency receipts, and probe runs to
opaque demand IDs. The production and in-memory-test adapters justify this
internal seam. Serialized proof documents remain audit artifacts and protocol
inputs to be revalidated, not an adapter with authority.

`ReceiptIssuer` is a real seam with built-in, configured local/portable, and
in-memory test adapters. Its interface accepts only the verifier-derived
canonical receipt payload and returns an authenticated envelope; it cannot
change semantic fields. This is the public façade of
`solid-facts-backend::contract_certification`; the wire-independent verifier
remains inside `solid-reactive-ir`. Analysis consumers never see certification
interfaces or raw evidence. They continue to receive only receipt-validated
normalized semantics. Before publication, the backend re-decodes the returned
envelope, proves that its payload is byte-identical to the submitted canonical
payload, and verifies its signature or built-in entry digest.

The semantic certification module owns:

- analyzer-visible positive-candidate and closure-candidate selection;
- proof-family applicability and demand-DAG planning;
- family-specific witness verification;
- complete-census validation;
- claim-local closure;
- normalized invariant validation;
- canonical receipt payload construction; and
- typed, claim-local refusal reporting.

The backend owns demand scheduling, acquisition sessions, issuer invocation,
and crash-safe publication. The CLI eventually exposes one orchestration
command, but public `verify-contract` input must no longer turn a caller-written
proof JSON document into receipt bytes. It either performs full reacquisition
under policy 2 or only verifies an already authenticated receipt.

## Proof policy 2

Proof policy 2 replaces caller-declared census truth with family-specific,
independently checked witnesses. Keep the stable main document at
`schemaVersion: 1`. Version the neighboring protocols independently:

- set `proofVersion: 2` for the new witness-bearing proof document;
- set verifier `proofPolicy: 2`;
- set `receiptVersion: 2`; authenticated issuer, policy-digest, snapshot-root,
  and evidence-root fields are a deliberate incompatible receipt change; and
- decode old receipts only far enough to report an obsolete-policy refusal.
  Never accept a policy-1 receipt under policy 2.

The numeric policy is only a dispatch version. A canonical policy digest also
binds the exact applicability table, hash domains, algorithms, resource limits,
producer constraints, probe requirements, and receipt-authentication rules.
Two verifier builds may share policy number 2 only when they compute the same
policy digest. The Rust policy implementation is authority; any JSON policy
manifest is a generated audit artifact checked against it, never caller-loaded
runtime policy.

Policy 2 does **not** require eighteen caller-supplied complete censuses for
every claim. That policy-1 shape makes irrelevant complete-empty families a
false-closure primitive and repeats artifact-wide facts for every leaf.
Instead, the verifier derives a typed demand graph over every analyzer-visible
candidate fact and every proposed closure:

- families 1–6 are artifact-case prerequisites proved once for the immutable
  snapshot and shared by claims through exact demand IDs;
- families 7–14 and 17 are emitted only for the positive facts, claim domains,
  and semantic sites to which they apply;
- family 15 is emitted only for compiler-owned sites;
- family 16 is emitted for each relevant external dependency edge; and
- probe consistency is a separate policy veto, not semantic evidence of
  closure.

Every retained positive demand and every closure demand must be discharged.
Positive witnesses establish only the named possible fact; they do not close a
collection or establish a minimum beyond that fact. Family inapplicability is a
verifier result derived from the inspected claim/artifact graph, never an empty
witness or a caller flag. Changing the graph, policy digest, claim, family,
target range, snapshot, producer session, or dependency receipt changes the
demand ID and prevents replay. This changes proof mechanics, not normalized
semantic meaning, so semantic-model version 1 remains valid unless
implementation discovers that a normalized claim itself must be reinterpreted.

### Package-artifact families

For published packages, acquire the exact archive without running lifecycle
scripts and verify its SRI against independently selected registry metadata.
A lock-pinned archive may use the selected lock entry only under distinct
lock-pinned provenance; it does not prove registry publication. Extract either
kind into a controlled immutable snapshot.
Reject absolute paths, traversal, case-fold collisions, device entries,
escaping symlinks/hardlinks, duplicate archive members, decompression bombs,
and mutations after snapshot creation. Registry origin and acquisition policy
are identity. A lock-pinned or workspace/link package must use a distinct
provenance identity and cannot impersonate a published name/version/integrity
tuple. If semantic-model version 1 cannot represent that provenance without
reinterpretation, policy 2 refuses it rather than overloading the registry-
integrity field.

From that snapshot, recompute and verify:

1. package name, version, registry integrity, and manifest bytes;
2. manifest entrypoint selection, exact condition trace, condition set, and
   resolver-semantics identity;
3. runtime and declaration export resolution;
4. runtime artifact, declaration artifact, and transform identities;
5. exact runtime/declaration export identity; and
6. complete local runtime/declaration module closure.

Witnesses must name concrete files, export-map branches, symbols, edges, and
digests. Resolve the host's `ResolvedImport` independently against the snapshot
and refuse a mismatch. All reads occur from the snapshot, not the original
path. Equal caller-provided digest lists, an installed directory hash, or a
self-consistent caller-supplied SRI are not proof of a published artifact.

### Type Facts families

Demand and verify complete evidence for:

7. selected signatures;
8. actual-to-formal argument binding;
9. rest and tuple-spread coverage;
10. callable-path identity;
11. operation reachability;
12. operation cardinality;
13. recursive value shape;
14. finite guard partitions; and
17. domain exhaustiveness.

Each response must carry the exact producer build, table generation, project
identity, demand digest, covered source set, domain-specific completeness, and
unresolved premises. The Rust client must derive the census from the producer
response rather than accepting a proof document's `complete` field. More
importantly, the backend must obtain that response from the exact process the
pinned orchestration launched for this session after executable/source-
manifest identity and handshake verification. A
serialized response with the same build string is only audit data. Responses
from different processes, project generations, snapshot roots, or restart
epochs cannot be spliced into one certification. To close executable
substitution between hashing and launch, copy or open the pinned producer into
a private execution snapshot, verify it against the compiled/configured digest,
and launch that exact image; a platform that cannot establish this identity
returns a producer-provenance refusal.

Any Type Facts addition must update the producer, Rust client, normalized
consumer, protocol/schema when applicable, source-manifest identity, build
stamp, and focused real-typings fixtures atomically.

### Compiler family

For family 15, rerun the pinned semantic trace in the certification session for
the exact snapshot source/configuration/mode/compiler identity and reconcile
every compiler-owned source site with the materialized output identity. A
caller-carried trace is not authority. The Solid fork remains semantic-facts-
only: no lowering, output, diagnostic, runtime, feature, performance, or
unrelated change is permitted.

Published `solid-js@2.0.0-rc.3` and related `@solidjs/*` artifacts remain the
behavioral authority. Experimental server components and unstable protocols
remain explicitly open unless a newer exact published authority supplies the
missing premise.

### Dependency and probe families

For family 16, validate the dependency's receipt, exact resolved import,
artifact identity, closure, proof policy/digest, issuer provenance, and exact
closed claim before composing it into the parent. Bind the dependency receipt
digest and demand ID into the parent. A proposal, package-shipped trust claim,
or stale receipt never satisfies dependency composition.

Runtime probes are a verifier-derived veto channel, not family-18 closure
evidence. When policy requires a recipe, the backend launches it against the
same snapshot in a sandbox that denies writes to snapshot and producer/compiler
inputs. If that write isolation cannot be enforced, the probe is unsupported
and its mandatory gate refuses. A contradiction rejects the exact claim; an
invalid, incomplete, nondeterministic, timed-out, or omitted required run is a
probe-gate refusal. A successful finite run may add a possible-positive
witness, but its lack of contradiction cannot prove a negative, minimum,
maximum, or exhaustive domain. Policy 2 removes `ProbeConsistency` from the
semantic proof-demand set and records the probe-gate result separately in the
receipt payload.

## Receipt trust

The receipt must bind:

- exact stable-v1 main bytes;
- finalized semantic digest and semantic-model version;
- exact artifact provenance and immutable snapshot root;
- package, manifest, artifact, declaration, transform, export, and closure
  roots;
- proof policy number, policy digest, and proof-document version;
- demand-graph root, verified-positive root, per-family witness roots,
  producer-session roots, dependency-receipt and transitive dependency-trust
  roots, probe-gate root, and closed-claim root;
- exact verifier implementation identity as a cryptographic build/source
  digest, not an arbitrary label;
- issuer kind, key ID, and signature algorithm; and
- a domain-separated signature or built-in provenance binding over every field
  above.

Authentication has three explicit provenance modes:

1. Built-in receipts are accepted only when reached through the immutable
   compiled bundle map whose entry digest matches the checker build. Copying the
   same bytes into a project catalog does not preserve built-in provenance.
2. Persistent local receipts use a configured Ed25519 issuer key whose private
   material lives outside the project (an OS credential store or permission-
   checked user configuration). If no signer is configured, certification may
   return transaction-local accepted output but must not publish an offline
   accepted catalog entry.
3. Portable receipts use Ed25519 signatures chaining to an explicitly
   configured public-key trust root outside the package/project. The trust
   entry constrains accepted policy digests and verifier builds; a key ID or
   public key carried in the receipt is never self-authenticating.

Sign a domain-separated canonical binary receipt payload, not mutable JSON
presentation bytes; the signature envelope is excluded from the payload it
authenticates. The loader rejects unknown algorithms, noncanonical signatures,
key confusion, revoked roots, policy rollback, and issuer-kind/provenance
mismatch. Trust-store digest and revocation epoch participate in the accepted-
contract cache identity. The default policy fails closed and requires parent
recertification when a transitive dependency issuer used during certification
is revoked; a parent signature cannot silently launder revoked dependency
authority.

Receipt validation in ordinary analysis remains offline and sidecar-free. It
walks the finalized normalized contract to recompute the complete positive-fact
and closed-claim roots, recomputes every other derivable binding, validates
policy/build constraints, and authenticates the issuer/provenance. Raw proof
evidence is never retained or queried during analysis. Content-addressed
document and receipt blobs are written first; one atomic catalog-pointer rename
commits them, so a crash cannot expose a partially accepted entry.

## Delivery gates

Phase 19 has two separately mergeable milestones so refusal pressure cannot
weaken the trust migration:

- **Phase 19A — authenticated policy cut.** Complete slices 0–10, migrate or
  retire every active policy-1 receipt, remove shortcut issuance, and pass the
  full product gates. This milestone may reduce the accepted first-party count
  if evidence cannot be reconstructed. Do not merge an intermediate Phase 19A
  branch that still exposes the policy-1 loader plus partially trusted policy-2
  artifacts as a finished feature.
- **Phase 19B — refusal reduction.** Begin only from merged Phase 19A. Land
  independently green evidence-producing slices by refusal owner. Zero newly
  accepted ecosystem cases is an honest result if all attempted claims expose
  deeper exact refusals; there is no numerical acceptance exit target.

## Implementation slices

Implementation started on 2026-08-29. The first internal slice now freezes the
policy-1 inventory and refusal envelope, exposes a typed Rust-owned policy-2
manifest with golden digest, and derives proposed closures plus positive
operation candidates directly from normalized meaning. The active loader,
proof documents, and receipts remain policy 1. The producer-field authority
audit is now executable, and Slice 2 adds distinct published, lock-pinned, and
refused-local provenance; bounded archive/SRI and registry-metadata
verification; immutable order-independent snapshots; and Rust-owned replay of
runtime/declaration entrypoint resolution. Slice 3 now inventories normalized operations,
edges, resources, guards, callbacks, and recursive values, then derives opaque
snapshot/policy-bound demand IDs plus a demand-graph root without accepting a
caller-supplied plan. Its closed witness envelope now binds concrete site
identities and evidence roots to exactly one derived demand, rejects unknown
wire variants and fields plus missing, extra, duplicate, empty, oversized, or
family-swapped evidence, and computes an order-independent evidence root. This
is structural coverage, not evidence authority: serialized documents remain
non-authoritative, while the six artifact-family bindings are constructed only
from the opaque snapshot-verified plan. Slice 4 now recomputes the complete
local runtime and declaration module graph from snapshot bytes, including literal dynamic chunks,
resolution-input assets, accepted-edge subjects, and scope-resolved open
frontiers; independently replays exact export bindings through named, star,
namespace, wildcard, and divergent runtime/declaration targets; and rejects a
caller closure or export table on any path, role, edge, hash, condition trace,
or target mismatch. Transformed output remains explicitly uncertifiable until
both output and tool bytes can be materialized inside the snapshot authority.
Authority-bearing Type Facts, compiler, dependency, and probe adapters, receipt
authentication, and the atomic cut are still pending. Run
`cargo +1.97 run --manifest-path rust/Cargo.toml -p solid-reactive-ir --example emit_proof_policy_2`
to render the audit manifest; Rust and Bun drift tests compare the checked-in
artifact and digest with the compiled definition.

### Slice 0 — Freeze authority, inventory, and the executable policy

After Phase 18 merges, regenerate and commit a baseline inventory that binds
the 130 stable-v1 mains, all 73 active policy-1 receipt documents, the 24 unique
first-party artifact cases, the 418-row ecosystem report, exact producer and
compiler identities, and current refusal-owner counts. Add one canonical
Rust-owned policy definition, plus a generated policy-2 manifest and golden
digest covering applicability, hash domains, algorithms, resource budgets,
producer constraints, probe requirements, and receipt trust. The gate must
prove the generated artifact equals the compiled policy.

Audit every proposed demand against an existing authoritative producer field
and completeness guarantee before changing protocols. Record `already exact`,
`producer extension required`, or `unsupported` per demand. An unsupported
demand remains a refusal; the policy manifest must not invent an adapter for it.

Keep policy 1 as the active product loader until the atomic cut. Policy-2 code
may coexist behind a test-only/internal entry point, but no loader may call
itself policy 2 while accepting a policy-1 receipt.

### Slice 1 — Build the adversarial policy-2 harness

Create local red tests, then land each test only with the policy-2 behavior that
makes it green. Do not commit a knowingly vulnerable characterization test or
an ignored red test merely to preserve commit ordering. The attacks include:

- `complete: true` with fabricated complete-empty censuses;
- equal fabricated `enumerated` and `classified` lists;
- fabricated partial-positive operations, edges, resources, guards,
  capabilities, cardinalities, and recursive value items;
- a supplied proposal plan that omits one closed domain or positive fact;
- one checked-corpus digest reused for unrelated proof families;
- a proof document that claims an authority solely through its family name;
- a serialized Type Facts or compiler response with a copied valid build ID;
- a substituted producer executable, Node runtime, or probe harness that copies
  the expected version string;
- a fake but nonempty verifier build;
- a forged proof root in an otherwise self-consistent receipt;
- cross-family and cross-claim witness replay; and
- a policy-1 receipt presented to the policy-2 loader.

Also pin filesystem mutation between planning and issuance, trust-root/key
substitution, and built-in receipt bytes copied through a project catalog. Do
not weaken or delete the existing false-closure mutation corpus.

### Slice 2 — Add immutable artifact snapshots and provenance

Implement published-archive, lock-pinned, and local-artifact provenance as
distinct types. Verify the selected registry metadata or lock entry and archive
SRI without upgrading lock-pinned bytes into a registry-publication claim;
extract without lifecycle scripts into a bounded snapshot, canonicalize path
topology, and bind every later read to the snapshot root. Resolve the supplied
`ResolvedImport` independently against the snapshot. Add archive traversal,
duplicate member, case collision, symlink/hardlink escape, decompression-bomb,
registry-origin, provenance-confusion, TOCTOU, and mixed-package tests.

### Slice 3 — Introduce typed proof demands and witnesses

Replace generic digest censuses with verifier-derived artifact prerequisites,
positive-fact demands, closure demands, dependency-edge demands, and probe
gates. Use closed Rust enums and opaque proof-demand-v2 IDs bound to the policy
and snapshot; do not overload the existing closure-oriented semantic claim ID
when a positive structural fact needs a more exact subject. Keep authority-
bearing constructors private to the backend session adapters and serialization
non-authoritative. Reject unknown witness variants, duplicate site identities,
unbounded collections, noncanonical paths, fabricated inapplicability, missing
demands, extra/orphan evidence, and family/witness mismatches.

### Slice 4 — Implement package-artifact verification

Move package, manifest, resolution, artifact, export, and module-closure proof
construction behind the certification module. Recompute every file and edge
from the immutable snapshot and prove artifact-wide prerequisites once. Add
same-byte/different-closure, condition-order, symlink, traversal, stale-hash,
wildcard, declaration/runtime divergence, and mixed-package adversarial tests.

### Slice 5 — Implement Type Facts session verification

Add demand-shaped certification facts for the nine Type Facts-owned families.
Require explicit completeness per exact domain and source census. Keep unknown
recursive leaves local. Bind evidence to the launched executable, handshake,
process/restart epoch, project generation, demand, and snapshot. Test copied
build strings, cross-session splicing, crash/restart mixing, unresolved aliases,
generic signatures, unknown-length spreads, escaped callbacks, overload
selection, namespace and member dispatch, conditional guards, and sibling
contamination against real published typings.

### Slice 6 — Implement compiler-session reconciliation

Bind proof witnesses to semantic trace version, compiler identity, source,
configuration, mode, generated output, and the complete compiler-owned site
census obtained from the directly launched pinned session. Re-run output-
neutrality and independent ancestry/identity gates. Reject copied traces and
mixed compiler sessions. If a needed fact would change compiler behavior,
leave that claim open.

### Slice 7 — Implement dependency composition and the probe veto

Build a bottom-up dependency certification queue. Detect dependency cycles and
report the exact first unaccepted edge under canonical `(package, artifact,
export, claim)` ordering. Authenticate each dependency receipt before using its
exact closed claim. Execute verifier-derived mandatory probes against the same
snapshot, consume contradictions, and refuse incomplete probe gates without
allowing successful probes to establish closure.

### Slice 8 — Issue and validate authenticated policy-2 receipts

Implement receipt version 2, the canonical signed payload, built-in provenance,
configured local/portable Ed25519 issuers, trust-store policy constraints, and
atomic publication. Keep the policy-2 loader internal until the cut. Add stale
artifact, changed closure, changed facts, changed verifier, changed policy
digest, policy downgrade, wrong/revoked issuer, algorithm/key confusion,
noncanonical signature, noncanonical accepted-main encoding, provenance
copying, and receipt replay tests in native and WASM loaders.

### Slice 9 — Replace shortcuts, reissue, and cut atomically

Use the Phase 13/14 checked corpus as an oracle and expected-result authority,
not as one generic proof census. Delete `accept_checked_corpus_case` and any
equivalent caller-proof issuance path. Reconstruct demanded witnesses from the
exact pinned Solid 1.x artifacts and Solid 2 RC.3 artifacts, their declarations,
Type Facts, applicable compiler semantic traces, dependency receipts, and probe
gates. Reissue each reconstructable first-party case under proof policy 2. Any
case that cannot be reconstructed becomes an open proposal with an exact owning
refusal; it is not grandfathered.

The cut is one green semantic commit: switch the active loader to policy 2;
reissue or retire every one of the 73 active receipts; update both bundle
locations, fixtures, catalogs, manifests, inventories, caches, and WASM/native
expectations; and make policy-1 discovery report only an obsolete-policy
refusal. No active policy-1 receipt or checked-corpus shortcut remains after
this commit.

### Slice 10 — Add automatic certification orchestration

Add a single CLI workflow that acquires exact artifacts, generates the open
proposal, asks Rust for proof demands, obtains authoritative witnesses, runs
certification, writes accepted bytes and a receipt, and updates a catalog only
after success. Intermediate failures produce deterministic claim-local
refusals and never leave a partially accepted catalog entry. Repurpose or
remove public `verify-contract` proof-file issuance; audit transcripts may be
exported for diagnosis but cannot be replayed as authority.

### Slice 11 — Reduce refusals by verified-export leverage

Work through refusal owners in this order:

1. certify dependency leaves and compose their accepted receipts, targeting
   the 20 dependency-obligation rows and their transitive parents;
2. improve exact export-kind censuses, targeting the 14 Type Facts-owned rows;
3. supply the one missing parameter-behavior premise;
4. repair the two resolver-owned package-export identities;
5. support finite wildcard entrypoint censuses and other statically enumerable
   artifact shapes from the group of 15; and
6. retain the 8 no-ESM rows and genuinely dynamic/opaque shapes as refusals
   unless a separately reviewed artifact model proves them.

Attempt policy-2 certification for the 40 structurally complete ecosystem
proposals first. No acceptance-count target is allowed to weaken proof. A
newly discovered deeper refusal is a valid result.

## Measurement and reporting

Add a machine-readable Phase 19 report with at least:

- rows by complete proposal, partial proposal, full refusal, and not-applicable
  artifact status;
- policy-2 receipt-issued packages, artifact cases, and active receipt
  documents, with all three denominators stated;
- verified exports, analyzer-visible positive facts, and locally closed claim
  domains;
- open claims by exact fact owner and recursive path;
- verified exports unlocked by each new fact or dependency receipt;
- refusals introduced by the stronger policy;
- all 73 baseline policy-1 receipts classified as reissued, retired/demoted, or
  pending, with pending required to reach zero before the cut;
- artifact-provenance, producer-session, probe-gate, resource-limit, trust,
  authentication, and semantic refusals reported separately;
- demand counts and evidence bytes by applicable family, rather than 18 times
  every closed claim;
- proof-input, verification, receipt, load, and query cost; and
- main, proof, sidecar, and receipt byte distributions.

Do not count a structurally complete proposal, successful probe, reviewed
mapping, or generated proof plan as verified. Keep `not applicable` distinct
from verified, partial, and fail-closed; classification must not hide a
supported import that remains uncertifiable.

## Required tests

### Focused tests

- one positive and one adversarial test for every demand variant, verifier-
  derived inapplicability rule, and probe-gate result;
- artifact archive/SRI/provenance, immutable-snapshot, and TOCTOU attacks;
- copied producer/compiler identities and cross-session/restart splicing;
- substituted producer images, Node runtimes, and probe harnesses;
- complete-positive and complete-negative closure for every recursive domain
  shape;
- partial and unknown leaves beside independently closed siblings;
- exact artifact, export, signature, guard, operation, resource, owner, and
  lifetime identity;
- policy digest, built-in provenance, local/portable issuer, trust rotation,
  revocation, and signature migration;
- accepted dependency composition and cycles;
- runtime probe contradiction, incomplete mandatory probe, and successful
  probe without negative inference; and
- native and WASM accepted-contract loading through the same normalized seam.

### Property and mutation tests

- every seeded false-closure mutation remains detected;
- deleting or substituting any required demand, witness, producer session,
  dependency receipt, or mandatory probe fails;
- adding a fabricated empty family cannot change applicability or closure;
- adding an unwitnessed partial-positive fact cannot change accepted semantics;
- deleting an applicability edge changes the policy digest and is caught by a
  fixture whose exact semantic site demands that edge;
- witness and proof ordering does not change canonical roots;
- source, package, registry origin, snapshot, declaration, closure, compiler,
  Type Facts, policy digest, verifier, trust-store, or issuer drift invalidates
  the receipt;
- archive member ordering that preserves the same canonical snapshot preserves
  semantic identity, while topology changes do not;
- normalization-equivalent documents produce one semantic digest;
- recursive uncertainty stays local under sibling permutations; and
- no policy-1, forged, self-signed, provenance-copied, revoked, or
  noncanonical receipt reaches accepted typestate.

### Product gates

- the real-typings TypeScript oracle proves the checker duplicates no `tsc`
  diagnostic;
- the 16 Solid 2 RC.3 conformance rows remain represented with exact local
  open domains;
- contract corpus, ownership, coverage, compiler parity, false closure,
  ecosystem, compactness, performance, native packaging, and WASM packaging
  remain green;
- every package, producer, compiler, verifier, policy, or trust pin movement
  replays proof issuance rather than reusing stale receipts; and
- the Phase 18 inventory is extended to require proof version 2, receipt version
  2, policy 2, zero active policy-1 receipts, and no caller-proof issuance
  shortcut after the cut.

## Documentation changes

Update together:

- `docs/package-contracts.md` with the policy-2 trust model and certification
  workflow;
- `docs/package-contract-v2/proof-and-evidence.md`, `architecture.md`, and
  `migration-and-verification.md` with demand applicability, immutable
  snapshots, direct producer sessions, receipt authentication, and cutover;
- `docs/package-contract-v2/semantic-model.md` only if normalized meaning
  changes; otherwise keep semantic-model version 1;
- `docs/package-contract-v2/implementation-plan.md` with Phase 19 progress;
- `rust/ARCHITECTURE.md` with the certification module and evidence adapters;
- `CONTEXT.md` with canonical spellings for proof demand, artifact snapshot,
  probe gate, issuer provenance, and verified positive fact;
- `docs/precision-backlog.md` with every changed refusal owner;
- the Phase 18 inventory/gate with the new neighboring protocol and receipt
  requirements; and
- a Phase 19 completion report containing exact remaining uncertifiable
  domains and receipt migration status.

## Verification procedure

Use focused local red/green checks for each slice, land only green commits, and
keep one Cargo process active at a time. Test filters must execute at least one
test; a zero-test success is not evidence. While iterating, run only the owning
slice's narrow command, for example:

```sh
cargo +1.97 test --manifest-path rust/Cargo.toml \
  -p solid-reactive-ir --lib contract_semantics::certification

cargo +1.97 test --manifest-path rust/Cargo.toml \
  -p solid-facts-backend --lib contract_certification

SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" \
  cargo +1.97 test --manifest-path rust/Cargo.toml \
  -p solid-facts-backend --test contract_interface --test contracts_process

bun run --cwd packages/cli test
bun run --cwd packages/wasm test
```

This is broad architectural and generated-artifact work, so the final handoff
authority is one uncached full verification. Refresh the expensive ecosystem
authority once before it, because `make verify` checks but does not regenerate
that report:

```sh
SOLID_CHECKER_GATE_CACHE=0 make ecosystem-benchmark
SOLID_CHECKER_GATE_CACHE=0 make verify
```

Do not separately rerun contract corpus, conformance, TypeScript oracle,
ownership, coverage, performance, CLI, or WASM gates immediately before the
full verification when their inputs are unchanged; `make verify` already owns
those checks. If the ecosystem refresh changes tracked inputs, the subsequent
full verification validates those exact bytes.

Run registry/package acquisition with normal network permissions only when a
gate requires exact published artifacts. Record sandbox failures separately
from semantic refusals.

## Commit and handoff strategy

Keep commits individually green and ordered by dependency:

1. baseline inventory, threat model, policy manifest, and golden policy digest;
2. adversarial harness changes paired with their policy-2 fixes;
3. immutable artifact snapshot and provenance;
4. typed demand graph, witnesses, and certification interface;
5. package-artifact verifier;
6. Type Facts producer/client/session slice, if required;
7. compiler-session reconciliation slice, if required;
8. authenticated dependency composition and probe veto;
9. receipt-v2 authentication and internal loader;
10. one atomic active-policy cut containing shortcut deletion, all receipt and
    catalog reissues/retirements, and native/WASM loader changes;
11. automatic CLI orchestration and public proof-file issuance removal;
12. refusal-reduction slices and reports; and
13. architecture, precision, and completion documentation.

The atomic cut is the deliberate exception to ordinary generated-artifact
separation: its loader, all active receipts, catalogs, manifests, indexes, and
tests must move together or no commit is green. Outside that cut, do not mix
generated receipt changes with unrelated semantic implementation.
Do not move the stable main schema, weaken closure, treat absence of an optional
probe as evidence, accept a missing mandatory probe gate, or grandfather a
policy-1 receipt to keep a snapshot green.

## Exact non-goals

- proving arbitrary JavaScript program equivalence;
- accepting dynamic loading, `eval`, native code, opaque WASM, or mutable
  unbound globals without a separately exact model;
- inferring behavior from package or export names;
- treating runtime non-observation as negative proof;
- running package lifecycle scripts during acquisition or treating package code
  execution outside the explicit probe sandbox as evidence;
- trusting package-shipped receipts automatically;
- treating a self-consistent archive SRI, producer build string, issuer key ID,
  or receipt-carried public key as an external trust root;
- changing stable main `schemaVersion: 1` for proof-policy work;
- duplicating TypeScript diagnostics;
- adding Solid compiler lowering, output, diagnostics, runtime behavior,
  features, performance work, or unrelated refactors; and
- claiming experimental Solid server-component behavior as stable.

## Definition of done

Phase 19's proof-policy milestone is complete when an adversarial caller cannot
obtain accepted typestate from fabricated transcripts, copied producer
identities, mutable artifact paths, self-selected trust roots, or forged
receipts; every receipt-issued claim is backed by the complete verifier-derived
demand graph over one immutable snapshot; every retained analyzer-visible
positive fact has an exact witness; and every persistent receipt is
authenticated through its actual provenance channel. The active repository
contains zero policy-1 receipts, no checked-corpus or public proof-file shortcut
can issue accepted typestate, all 73 baseline receipts have a final migration
result, and the policy-2 inventory/gates pass in native and WASM paths.

Refusal reductions come only from exact new evidence. Remaining dynamic,
unsupported, resource-limited, TypeScript-owned, or experimental domains are
reported at the exact local leaf and do not erase unrelated verified facts.
