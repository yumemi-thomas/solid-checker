# ADR 0052: Receipt-bound dependency calls in the creates census

Status: implemented; full-corpus measurement and verification recorded in the
phase-21 follow-up report.

## Decision

The user authorized the recommended dependency-receipt composition path on
2026-09-07. External behavior remains contract-driven. The census may describe
an external call as a **conditional dependency claim**, and finalization must
discharge that exact obligation using the authenticated child receipt. An
unresolved callee or an absent claim never establishes empty behavior.

## Premise and verifier

For an ordinary call, the Type Facts declaration location must resolve inside
an authenticated dependency snapshot. The snapshot's replayed declaration
export must bind to the same declaration name, source path, and exact span.
Resolving an export reference to its declaration uses the normalized source
facts; spelling alone cannot select an export. The selected child artifact
case must belong to a dependency explicitly demanded by this parent.

The child must propose an empty creates domain as a closure candidate, or
already state it closed. That proposal only permits recording an obligation.
It does not authorize acceptance. The census records the package, artifact
case, accepted-contract digest, export, and semantic claim ID in a
`census-dependency-creates:` witness site, alongside the exact call site.

Dependency composition first authenticates the ordinary exact child receipt,
including its artifact, policy, issuer, epoch, and graph binding. It then
requires both a closed empty creates domain in the certified child main and
that exact creates claim ID in the receipt. Its witness names the child
receipt digest and claim. The composition evidence root binds the census
root; finalization additionally compares the census's dependency-obligation
root with the root the composition token discharged. Mixing an independently
acquired composition token with conditional census evidence therefore fails.

The dependency-first graph transaction retains its existing bounded
withdrawal/replanning behavior. If the child cannot close, the parent
candidate remains withheld. Both child and parent still require their
applicable vetoes. Constructors are excluded: a call-domain creates claim
does not describe construction. This adds no Type Facts wire field or
protocol change and introduces no TypeScript diagnostic.

## Evidence and limits

The packed two-package `dependency-census-composition` fixture proves an
aliased external call closes with a closed child and completed veto, and
remains open with an open child, an uncompleted veto, or a call to a different
export whose creates claim is unknown. The different-export control supplies
both exports to the artifact replay, so it reaches census checking rather
than failing the export census first. Existing exact-receipt tests cover
receipt transplantation, stale epochs, and missing exact closed claim IDs.

The first motion measurement reduced withheld candidates from 258 to 226:
six each for `defaultOffset`, `fillOffset`, `isHTMLElement`, `isSVGElement`,
and `isSVGSVGElement`, plus two for `mergeRefs`. The full follow-up report
records the corpus delta and remaining first blockers. A cleared dependency
call can expose a later blocker; it is not automatically a closed candidate.

Unknown or nonempty child claims, ambiguous export/case bindings, unmatched
declarations, missing receipts, and construction remain fail-closed. The
policy does not inspect dependency implementation bodies in lieu of a
contract. Local result shapes, returned closures and their captures, nested
callable provenance, host operations, and source-condition execution require
their own premises and are not authorized by this disposition.
