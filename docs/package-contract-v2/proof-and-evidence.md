# Proof, evidence, and acceptance

## Trust model

The package-contract path has four different authorities:

1. Type Facts and Solid compiler execution facts can establish exact static
   premises within their documented completeness envelopes.
2. Runtime probes can witness possible behavior and falsify proposed absence or
   exhaustiveness.
3. The proposal generator is untrusted semantic inference.
4. The proof checker is the only module that may finalize domain closure and
   issue an acceptance receipt.

Human review is an optional, separately identified authority for opaque cases.
It is not required for the normal package path and never silently inherits the
machine-verification tier.

## Proposal lifecycle

```text
open proposal
  + exact fact transcripts
  + artifact closure
  + proof candidates
  + probe plans
        |
        v
runtime observations and falsification
        |
        v
proof replay
        |
        +-- failure: keep exact domain open or refuse artifact case
        |
        `-- success: finalize closed domains and issue receipt
```

The generator may state which domains it believes are exhaustively proved, but
the proposed document does not become an accepted contract until the proof
checker independently replays those obligations.

## Proof rule families

Semantic-model version 1 admits these proof rule families:

- package identity and registry integrity;
- manifest and entrypoint identity;
- exact package-export resolution trace;
- runtime artifact and declaration hashes;
- runtime/declaration export identity;
- module and dependency closure;
- selected TypeScript signature;
- actual-to-formal argument binding;
- rest and tuple-spread coverage;
- callable-path identity;
- finite guard partition;
- operation reachability;
- operation cardinality;
- recursive return/value shape;
- compiler lowering and source-site reconciliation;
- accepted dependency-contract composition;
- domain exhaustiveness;
- probe witness and falsification.

Proof rules are small and local. They cite fact IDs and normalized semantic
subjects rather than generator implementation details.

Phase 11 represents these as eighteen closed-set `ProofFamily` values and
requires every family for each proposed local closure. A rule replay is bound
to the normalized semantic digest, semantic claim ID, exact package,
artifact/declaration/transform/closure identity, and exact runtime/declaration
export identity. It accepts only a complete census whose enumerated and
classified site sets are equal and which has no unresolved premise. Raw
transcript bytes are bounded and hashed by the verifier; callers cannot inject
a transcript digest or construct the opaque replay result directly.

Authorities are independent. Package/artifact rules require package-acquisition
authority; signature, binding, reachability, cardinality, recursive-shape,
guard, and exhaustiveness rules require Type Facts; compiler reconciliation
requires compiler execution facts; dependency composition requires an already
accepted dependency contract; and probe consistency requires runtime-probe
authority. One authority cannot stand in for another.

Policy 2 makes dependency and probe applicability verifier-owned. Every exact
external closure edge is crossed with every proposed parent closure, producing
a canonical `(dependency package, dependency artifact, parent export, parent
claim)` requirement. Multi-package certification is dependency-first and
cycles are refused before receipt use. Probe consistency is not a proof-witness
family: every proposed closure gets a mandatory veto gate, a contradiction
rejects that closure, and successful finite non-observation cannot close it.
Policy-2 receipt authentication is implemented behind the internal native/WASM
seam. These schedules still remain unsatisfied in the active product until the
receipt cut and directly launched probe-harness identity are complete.

Receipt v2 requires the canonical compact stable-v1 main and authenticates a
domain-separated binary payload over the complete artifact, certification,
producer, dependency, probe, policy, and verifier identity. A built-in entry is
trusted only through its independently compiled receipt digest; persistent
local and portable entries require strict Ed25519 verification against a
configured trust-store entry that constrains issuer kind, scope, policy digest,
and verifier build. Revocation epoch and trust-store digest remain part of the
authenticated identity. Publication synchronizes content-addressed main and
receipt objects before atomically replacing one catalog pointer. This loader is
not an active analyzer authority until the Slice 9 cut.

## Closure obligations

Before closing one claim domain, the verifier proves:

1. The exact package, entrypoint, artifact, declarations, and closure match.
2. Every relevant call, return, reference, or compiler site was enumerated.
3. Every enumerated site was classified for this domain.
4. Every alias and statically bounded spread was followed.
5. Unknown-length spreads remain open unless a universal rest proof exists.
6. No value affecting the domain escapes into unknown code.
7. Every external dependency edge has an accepted relevant contract or is
   proved irrelevant.
8. Every conditional branch is disjoint and exhaustive.
9. Every compiler-controlled site reconciles with actual output.
10. Every dynamic loader affecting the domain is bounded.
11. No runtime observation contradicts the proposed set or cardinality.
12. Every recursive value child required for closure is independently proved.

Failure opens this domain unless it invalidates structural identity.

## Probe authority

A probe may establish:

- an operation occurred;
- a callback received an observed argument shape;
- a result exhibited an observable protocol;
- one scheduling order occurred;
- cleanup occurred;
- repeated async emissions occurred;
- a proposed complete-negative domain is false.

A probe may not establish:

- an operation never occurs;
- all branches were exercised;
- a callback always occurs;
- an error is impossible;
- a domain is exhaustive;
- a finite maximum from a finite run.

Probe observations therefore create possible-positive evidence or falsify a
closure candidate. Static proof supplies guaranteed-positive and negative
claims.

Phase 10 enforces this as a typed runtime-plan boundary. A probe plan is built
from an exact Phase 8 `PlannedProposal`, an artifact-case mode matrix, and
claim-addressed recipes. Possible-positive recipes are admitted only for
planned possible operations; closure-falsification recipes are admitted only
for planned closure domains. There is no recipe authority for negative,
minimum, maximum, exhaustive, or accepted claims.

Each recipe uses semantic drains (`flush`, bounded microtask turns, and bounded
macrotask turns), never elapsed-time sleeps as behavioral evidence. Every
session also has a wall-clock timeout as a termination bound. Each exact mode
runs at least twice with fresh process, realm, and module-instance identity.
Only identical semantic event sequences across every repeat are usable.

The event vocabulary is call, render, flush, callback, cleanup, settlement,
emission, transition, request, response, and stream. Scenario validators
require ordered cleanup production/disposal, repeated zero-based
AsyncIterable emissions followed by settlement, active-to-settled/reverted
transitions, request-to-uncommitted-to-committed responses, or root-lifetime
cleanup as appropriate. A finite run that does not contain the expected
positive marker is a refusal, never evidence of absence.

## Evidence sidecars

Sidecars contain detailed, claim-addressed material:

- semantic claim ID;
- artifact and closure identity;
- fact transcript IDs;
- proof rule inputs;
- probe construction recipe;
- runtime and environment identity;
- observed events;
- errors, timeouts, and refusal reasons;
- producer and verifier builds;
- coverage limitations.

Claim IDs derive from normalized semantic subject paths, not summary names or
JSON positions. Reformatting or summary renaming must not orphan evidence.

Phase 9 fixes two independent document families at sidecar version 1:

- `solid-checker-proof-evidence` contains claim-local fact-transcript and proof-
  input identities, coverage limitations, and the producer identities needed
  for later replay;
- `solid-checker-runtime-probe-evidence` contains a claim-local probe recipe
  and a non-empty exact mode matrix. Each mode observation records runtime,
  conditions, operating system, architecture, explicit sandbox kind and
  policy, outcome, limitations, and producer identity.

Phase 10 probe transcripts are deterministic internal documents with their own
kind/version discriminator. They bind the normalized semantic digest, probe
plan, claim, exact runtime/declaration/transform/closure artifacts, export,
recipe, mode/environment, isolated repeat identities, semantic drain counts,
and ordered events. Their SHA-256 becomes the sidecar witness or falsification
identity. The transcript is not the temporary public main schema and does not
advance that schema version.

The checked-in structural authority is
[`solid-reactivity-evidence-sidecars-v1.schema.json`](../../schema/solid-reactivity-evidence-sidecars-v1.schema.json).
The main contract contains the SHA-256 of each present sidecar. Each sidecar
contains semantic-model version, semantic digest, exact package/manifest
identity, and exact artifact/declaration/transform/closure identity for every
claim. It deliberately binds normalized contract identity rather than the main
file hash: putting each file's hash inside the other would create an
unsatisfiable hash cycle. The later acceptance receipt binds the final main
wire digest and proof root.

Semantic claim IDs use the canonical spelling
`claim:v1:sha256:<lowercase-hex>`. The hash domain includes claim-ID version,
semantic-model version, exact package identity, exact artifact-case selection
and closure identity, exact runtime/declaration export identity, and the typed
normalized subject path. It excludes summary IDs, JSON positions, formatting,
sidecar layout, and unrelated claim values.

Evidence validation returns only ordered claim IDs. Raw fact/proof/probe bytes
are inputs to later verification, never fields of the normalized contract or
ordinary analyzer interface. A referenced sidecar must be present during
verification; after receipt issuance it may be deleted without changing
ordinary analysis.

## Acceptance receipt

Illustrative receipt:

```json
{
  "receiptVersion": 1,
  "wireDigest": "sha256:...",
  "semanticModelVersion": 1,
  "semanticDigest": "sha256:...",
  "artifactsDigest": "sha256:...",
  "closureDigest": "sha256:...",
  "proofRoot": "sha256:...",
  "closedClaimsRoot": "sha256:...",
  "verifier": {
    "build": "...",
    "policy": 1
  }
}
```

The receipt binds both wire bytes and normalized meaning. This allows semantic
comparison across the temporary `2` to stable `1` renumber, while ensuring the
final wire document still receives a fresh receipt.

Receipt fields are never caller-selected after proof replay. The verifier
canonicalizes proof order and census order, computes `artifactsDigest` from
exact selected artifacts/exports, takes `closureDigest` from the selected
canonical dependency closure, computes `proofRoot` over family, authority,
claim, scope, transcript, and census roots, computes `closedClaimsRoot` over
sorted semantic claim IDs, and recomputes the finalized semantic digest after
closing only those leaves. A receipt policy below the current local policy is
rejected.

## Distribution

- Bundled contracts ship with compiled receipts.
- Locally generated project contracts cache receipts by semantic, artifact,
  closure, and policy digest.
- A remote registry may distribute checker-signed receipts.
- A package's self-issued receipt is a cache hint unless local policy grants it
  explicit authority.
- Ordinary analysis is offline and reads no raw sidecar.

The Phase 11 bundled adapter is immutable and rehashes every compiled receipt
on read. The project-local adapter writes canonical receipt bytes under their
content hash using an atomic temporary-file rename, verifies the persisted
bytes, and treats repeated writes as idempotent. Phase 12 remains responsible
for validating a stored receipt before analyzer exposure.

## Adversarial requirements

Verification tests must reject:

- a generator-added `closed` name without proof;
- omission of one reachable operation;
- altered operation cardinality;
- a proof transcript from another artifact;
- a stale or mismatched sidecar;
- a reordered or changed dependency closure;
- an incomplete Type Facts census presented as complete;
- an unreconciled compiler site;
- a probe from the wrong artifact case;
- a mutated recursive value leaf;
- a multiple-match artifact selection;
- a proof-policy downgrade.

Mutation coverage for false closure is an accuracy gate, not an optional test.
