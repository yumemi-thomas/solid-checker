# Dependency-first veto synthesis

Status: implemented, 2026-09-12.

ADR 0077 can prove a factory-created export's object shape only when the exact
dependency return is exhaustive. ADR 0096 can synthesize that return's veto,
but graph-wide acquisition previously asked the parent for its shape before
synthesis ran. A missing child recipe therefore blocked the whole transaction,
even though the same child certified on its own.

Initial Type Facts acquisition and synthesis now advance dependency first,
keyed by each canonical node identity. Every reachable Type Facts dependency
must finish its first acquisition and synthesis attempt before its parent is
acquired. An attempt that produces no recipe still completes that stage; a
later discarded recipe does not authorize another attempt. With no harness,
acquisition retains its existing unsynthesized behavior.

All plans remain available for authenticated source materialization and exact
re-export owner resolution. Only acquisition is deferred. Shared importer
groups may return evidence for an unrequested variant, so the graph explicitly
discards that evidence until the canonical variant is ready. Initial staging
also means a descendant cannot gain synthesized claims after the parent's
evidence has been cached. Later withdrawals still require exact dependency
receipt discharge.

No conditional proposal becomes acceptance authority. Each child must pass its
implementation census and mandatory veto, and the parent's finalization must
authenticate the exact child claim and receipt. A rejected, incomplete,
missing, wrong-export, wrong-importer or differently selected child cannot
certify the parent. All initial stages must finish before the gate pass. The
loop is bounded by closure candidates plus distinct Type Facts nodes plus two,
and a stage that makes no progress refuses explicitly.

The factory fixture covers an empty hand corpus, a three-node dependency chain,
a refused child census, contradictory and incomplete vetoes, and the existing
return-identity/importer controls. The negative recipes deliberately exercise
gate failure; they are not observations offered as production certification.

The current retained creates-refusal inventory also corrected a proposed next
step: neither the screenshot report nor the current report contains an actual
`exceeds 8 local-recursion hops` refusal. Depth numbers in other refusal text
are not proof-search exhaustion. No depth bound was raised. Floating UI's
module-map coercion and Motion's captured easing factories remain open; this
scheduling change supplies no new semantic premise for either.

## Real package measurement

An offline comparison used the same retained `seroval-plugins@1.5.6`
installation, exact archive integrity, recipe corpus and producer, selecting
`./web` with `--dependency-graph-lane`. The old executable refused at the
`AbortSignalPlugin` factory root. The new executable publishes two root cases,
production and development, each with the same fourteen plugin export names
and a proven plain object shape. These are export-root proofs, not fourteen
new closed call domains.

The accepted graph contains five creates closures and three returns closures
in its Seroval dependency. Both exact `createPlugin` return identities now
support their parents. It still withholds 28 reads and three creates claims;
the plugin export reads are not certified. The successful audit reports receipt
authentication and exact-case selection. A separate consumer finding comparison
has not been established, and these counts are not an ecosystem-wide delta.

The [measurement](../package-contract-v2/phase21/2026-09-12-staged-factory-measurement.json)
retains the executable hashes and verifies the published document/receipt
digests before recording its diagnostic projection. The runner's new optional
`--dependency-graph-lane` switch exercises the production graph path. Without
that switch, both executables refuse this package in the ordinary case-set
lane: staging does not implicitly acquire an absent dependency graph.

The new comparison used only existing authenticated archive-cache entries;
network cache misses still fail. Its successful audit records about 5.45 seconds
across proposal generation and witness acquisition. No full ecosystem run or
installation was performed, and no bundled contract or finding snapshot changed.

## Validation

The focused factory regression passed all thirteen scenarios, including the
three-node graph. Its explicit contradiction assertion caught a malformed
callback event in the new negative fixture (missing `ordinal`); correcting the
fixture made the existing pinned verify-profile test pass in 5.19 seconds.
The final `make verify` passed in 137.78 seconds, including the Rust workspace,
Go race suite, coverage, ownership gate, contract corpus, TypeScript oracle,
performance check and conformance. No assertion was relaxed to obtain that pass.

Parallel read-only review checked the staging/cache boundary and the published
artifact digests. The consumer check confirmed that the publication audit uses
fresh-process receipt authentication and exact-case selection, rather than a
consumer diagnostic comparison. The separate
[creates-refusal inventory](../package-contract-v2/phase21/2026-09-12-creates-refusal-inventory.md)
was reproduced against both retained reports. Its script, the measurement JSON,
this ADR and the two negative recipe modules are new artifacts; public schemas,
bundled contracts and existing finding snapshots were not regenerated.
