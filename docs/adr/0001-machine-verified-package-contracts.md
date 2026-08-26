---
status: accepted
---

# Verify package contracts through proof receipts and one normalized model

The next package-contract format will decode through one deep normalization module, and only an independent proof checker may close a claim domain or issue an acceptance receipt. Generators produce proposals, probes provide positive witnesses and falsification, evidence remains in hash-bound sidecars, and ordinary analysis consumes the normalized contract plus its receipt. This rejects the alternatives of trusting generator omission, treating tests as negative proof, duplicating wire semantics across Rust and JavaScript, or requiring human review for the normal certification path; those alternatives either admit silent false-negative certification or prevent package-scale automation.

The replacement uses temporary wire `schemaVersion: 2` during the all-at-once migration, then is re-emitted atomically as the first stable public `schemaVersion: 1`. The semantic model has its own version and digest so unchanged evidence may be replayed across that renumbering, but every final wire artifact receives a fresh acceptance receipt.
