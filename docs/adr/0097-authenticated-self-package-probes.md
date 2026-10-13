# Authenticated self-package imports in probe workspaces

Status: implemented, 2026-09-12.

The probe workspace excluded its subject from the dependency snapshot map.
Consequently a same-package import such as a Solid web entrypoint importing
`solid-js` could not run its veto, even when both entrypoints had independently
planned targets in the same authenticated archive. ADR 0096 kept these
candidates open rather than losing the containing package transaction.

The subject snapshot now participates in dependency resolution. Its existing
private copy is reused; no installed files or unauthenticated bytes are admitted.
Every planned specifier retains its own exact runtime target. Deduplicating
package snapshots no longer discards sibling targets, and two different targets
for one specifier refuse. Different snapshot roots under the same package name
remain ambiguous and refuse.

A self-package edge additionally requires an exact planned target. The ordinary
condition replay still checks every accepted edge, and the worker's independent
resolution answer must match both the authenticated directory and the exact
specifier target. Same-archive containment alone is insufficient. The existing
whole-private-tree census watches the reused copy; it is not copied or hashed
again as a separate dependency tree.

The authenticated self-package graph tracer now certifies with both generated
and handwritten recipes and requires a nonempty gate root. It also refuses
when the sibling plan is absent. Existing forged-target, snapshot-version,
condition-reproduction, and wrong-file-inside-the-copy controls remain.

## Measured gain

An offline before/after run on the same retained installation of
`@tanstack/solid-store@0.11.1` certified four then five return entries in its
dependency graph. The additional closure is `solid-js@1.9.14`'s
`dynamicProperty`; its missing-snapshot withholding disappeared. Both catalogs
passed ordinary receipt authentication and exact-case selection. Both runs used
authenticated cached archives and refused network cache misses.

The [measurement](../package-contract-v2/phase21/2026-09-12-self-import-and-reads-pilot-measurement.json)
records the two executable hashes and public audit results. The previous corpus
contains 158 `dynamicProperty` entries with this blocker, but this slice did not
remeasure that corpus and claims only the one observed package-graph gain.

The separate [reads pilot](../package-contract-v2/phase21/2026-09-12-reads-development-pilot.md)
does not change production recipe policy or the self-package implementation.

## Validation

`make test-focused TEST=authenticated_self_dependency_veto` passed the generated
and handwritten graph tracer, including the missing sibling-plan refusal.
`make verify` then passed in 148.39 seconds, including workspace checks, coverage,
the 289-case ownership gate, contract corpus, TypeScript oracle, and conformance.
No fixture snapshots or bundled contracts changed for this slice. The retained
measurement JSON is a new audit projection, not a certification authority.

The expensive full ecosystem measurement was intentionally deferred to keep
this slice bounded to one affected package and one reads pilot. The existing
corpus totals therefore remain unchanged; the 158-entry opportunity is still
unmeasured against this implementation.
