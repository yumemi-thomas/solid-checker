---
status: accepted
---

# Freeze package-contract semantic-model version 1

The first machine-verified package-contract model uses immediate-local
four-state claim knowledge, an acyclic operation graph, explicitly scoped
cardinality, restricted finite guards, recursive leaf-local value knowledge,
relational ownership, and exclusive selection by actual artifact resolution.
Only the independent verifier may close a domain. The compact wire format is
decoded through one deep normalization module; consumers never interpret
omission, `closed` arrays, summary references, or schema versions.

This rejects a flat phase list because scheduling, causality, repetition,
cleanup, and async emissions are independent; parent or call-level closure
because it can certify unknown siblings; general Boolean guards because they
cannot be generated or verified exhaustively; environment labels because they
do not reproduce package-export resolution; and nominal API brands because the
checker certifies observable behavior. These choices trade a richer private
model and verifier for local failure, proof strength, and automatic package
coverage without routine human review.

Wire compression may evolve during the temporary `schemaVersion: 2` migration,
but incompatible normalized meaning requires a new semantic-model version and
digest domain. The atomic stable `schemaVersion: 1` cut cannot reinterpret this
semantic version.
