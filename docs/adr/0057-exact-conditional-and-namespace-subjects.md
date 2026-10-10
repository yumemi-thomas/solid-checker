# Exact conditional and namespace declaration subjects

Status: implemented and verified in the [full-corpus measurement](../package-contract-v2/phase21/2026-09-07-exact-subject-recovery.md).

The private Type Facts project uses bundler/import resolution without custom
conditions. A singleton artifact case selected under `node`, `browser`, `solid`
or another explicit condition must therefore use the existing exact snapshot
declaration harness. Re-resolving its public package specifier under the host
defaults can select a different declaration. The snapshot replay remains the
authority for the file and owner; ordinary import-only singleton cases retain
their public-specifier harness. Multi-case ambiguity remains independently
handled by the existing resolution-variant census.

A namespace re-export has a module identity, not a named declaration called
`*`. Its declaration path is relative to the independently replayed owning
snapshot, which can be a dependency. Verification requires that exact owner
among the current authenticated plans, its explicit namespace target at that
path, the corresponding private-project package marker, and the compiler's
module display name matching that exact path. Missing ownership, a changed
snapshot, a different dependency path or a named value cannot discharge it.

These corrections consume existing authenticated resolution/export facts.
They add no wire fields, receipt format, trust authority, declaration borrowing,
or runtime behavior model. The ordinary operation proofs and dependency receipt
composition remain mandatory. No TypeScript diagnostic is introduced.

Focused native regressions cover singleton Node/browser/default selection and
namespace-owner/path/name falsifiers built from published test archives.
