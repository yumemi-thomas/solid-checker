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

The initial proof checker supports:

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

## Distribution

- Bundled contracts ship with compiled receipts.
- Locally generated project contracts cache receipts by semantic, artifact,
  closure, and policy digest.
- A remote registry may distribute checker-signed receipts.
- A package's self-issued receipt is a cache hint unless local policy grants it
  explicit authority.
- Ordinary analysis is offline and reads no raw sidecar.

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
