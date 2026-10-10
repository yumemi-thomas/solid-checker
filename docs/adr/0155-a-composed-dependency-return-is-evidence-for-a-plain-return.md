# ADR 0155: A composed dependency's closed return is evidence for a plain return

- Status: accepted and implemented (2026-09-28); written with the implementation.
  The lead asked, on 2026-09-28, for non-empty `creates` and dependency-call
  `returns` through cited or graph-composed dependencies, to close
  `createMediaQuery` → `createHydratableSignal` → `createSignal`.
- Date: 2026-09-28
- Owners: the `returns` census and the plain return's positive fact
  (`type_facts.rs`: `primitive_return_sites_in`, `dependency_plain_return`,
  `dependency_export_callees`, `DependencyReturnsClaim`), their discharge at
  composition (`dependencies.rs`: `VerifiedDependencyComposition::authenticate`
  and `from_citations`), and the citation identity (`policy2_receipt.rs`
  `CitedAcceptance`, `accepted_bundles.rs` `citation_withdrawn`,
  `bundle-accepted-contracts.mjs` `withdrawUncarriedCitations`)
- Relation: a fourth evidence kind for ADR 0113's plain return, beside its
  2026-09-28 amendment's `syntax`, `default-library` and `typescript-source`.
  Amends ADR 0151: a citation names the claim by content. Declines non-empty
  `creates` (below). No producer change, no handshake protocol, no
  contract-format change.

## Context

### The chain, domain by domain

Measured on the `make primitives-checkpoint` runs of 2026-09-28 (the
causes each export's measurement names, per host):

| export | `creates` | `returns` |
| --- | --- | --- |
| `solid-js` `createSignal` | the dialect's; its `creates` row is scoped to `browser` (ADR 0124) | the dialect's |
| utils `createHydratableSignal` | open: `solid-js:createSignal` dialect-silent (host free, `node`), a `no function-like declaration` census refusal (`browser`) | proposed as a reactive accessor, never certified: no census arm decides a returned signal tuple |
| media `createMediaQuery` | declined `create-publishing-callee`: its callee's `creates` is **open** | never proposed: `if (isServer) return () => serverFallback;` beside `return state`, element 0 of the dependency's tuple |

Neither limit the lead named is what holds this chain:

- **No `create` operation exists to compose.** Every summary of every tier
  document is `creates` closed-empty or open-empty: 332 closed and 605 open in
  the checked-in tier, 265 and 500 in a local regeneration from the checkpoint
  corpus, and no `kind: create` operation in either. That is the semantic
  model's own position (`semantic-model.md` § creates): `createSignal`
  registers nothing into a runtime outside the call, the only audited `create`
  operations are the dialect's `render` and `createServerReference`, and "the
  generator no longer emits any `create` at all". The `create-publishing-callee`
  decline on `createMediaQuery` fires because `createHydratableSignal`'s
  `creates` is not *closed*, not because it is closed with items
  (`creates_walk.rs`: `contract_creates_closed_empty` returns `false`).
- **`createHydratableSignal` closes no `returns`**, and `createMediaQuery`'s
  value is a member of the dependency's returned tuple on one branch and a
  fresh arrow on the other: partial and conditional forwarding, which stay open.

### Where a dependency call's result is refused today

The `returns` census's plain arm (ADR 0113) needs, beside a primitive type, one
of three facts per return site. A return of a dependency call has none: its
type comes from the dependency's `.d.ts`, which the amendment lists as a type it
cannot trust, and the call is not a reviewed built-in. So a wrapper
`return count(items)` over a dependency whose own `returns` is closed plain is
refused even when the graph certified that claim in the same transaction.
Measured demand in the checkpoint corpus: 115 refused plain returns in the host
free run's audits (19 distinct sites), of which 6 return a call and none a call
of a dependency export.

### A tier regeneration withdrew every citation

ADR 0151 named a cited acceptance by its receipt digest. Regenerating a local
tier twice from the checkpoint showed that no citation survives: all 466
`(artifactAcceptanceRoot, dependencyEnvironmentRoot, semanticDigest)` triples of
the first regeneration reappear in the second, and none of the utils receipts
does, because a receipt binds the certification's own importer path (a
per-run temporary directory) and its resolved import root. So every
re-certification of the same bytes in the same environment issues a new digest
for the same claim, and the bundler dropped all 44 citing bundles.

## Decision

### 1. The dependency return (ADR 0113's fourth evidence kind)

**A live value-carrying return is `plain` when its value fact states a
primitive alone and its whole value is exactly the result of a call of a
composed dependency export whose certified `returns` closes over one
unguarded `plain` return, or over nothing.** The witness names it:
`census-return:…:primitive:dependency:<package>:<export>:<plain|nothing>:<claim>`.

- *Exactly* is the producer's: `ReturnSite::call` states the whole returned
  expression is one call expression after identity-preserving wrappers; the
  implementation records a call (never a construction) at exactly that
  location; its resolved callee declaration is one dependency export, bound
  through the dependency plan's verified export bindings exactly as the
  `creates` census's `dependency-claim` disposition binds one
  (`dependency_export_callees`, now shared by both). Two candidate exports
  refuse.
- *Closed* is the dependency's, checked twice. At the census, the dependency's
  proposal must close the domain or propose it as a closure candidate. At
  composition, the obligation (`census-dependency-returns:<claim>`) is
  discharged against the contract the dependency's receipt certifies: that
  export's `returns` closes the same shape, and the receipt lists the claim
  among its closed claims. Otherwise `MissingClosedClaim`. The graph
  (`authenticate`) and a citation (`from_citations`) discharge it the same
  way, and the obligation is inside `dependency_census_root`, so a finalizer
  cannot drop it. The obligation names the dependent export, so a claim the
  dependency withheld after the dependent's evidence relied on it refuses
  through the dependent's own `returns` closure demand on that dependency,
  which the graph turns into withholding that closure by name
  (`composed_from_withheld_dependency`); the next pass re-decides the return
  without the claim and withdraws the operation by its own refusal. Where the
  dependent has no such closure candidate, the refusal has nothing to withhold
  and fails the graph closed.
- *As strong as the cited claim*: a closed `returns` holds for every
  invocation of the dependency export, so it holds for this call, whatever its
  arguments. `nothing` is `returns: []`: the call hands back `undefined`, a
  primitive. The positive fact of the operation reads the same predicate, so
  the two cannot disagree.
- Anything else stays open, by the existing refusal: a conditional of calls, a
  member or an operator over the result, a binding holding it, a dependency
  whose `returns` did not close (a withheld plain return, an accessor, a
  parameter pass-through — mapping a dependency's parameter return onto the
  dependent's arguments is not attempted), and every call the binding cannot
  resolve to one dependency export.

It reaches only where a dependency *plan* is in the census: the graph lanes.
On the plain lane a citation (ADR 0151) supplies a receipt and a document but no
plan, so `dependency_export_callees` binds nothing there, exactly as the
`creates` dependency disposition does not.

### 2. A citation names the claim (amends ADR 0151)

`CitedAcceptance` gains `artifactAcceptanceRoot`, `dependencyEnvironmentRoot`
and `semanticDigest`, each as the cited receipt signs it, and the frame is
`cited-acceptances:v2`. A tier carries a citation while some bundle's receipt
states those three for the same package and version; the receipt digest is
still recorded, for audit. Withdrawal is unchanged otherwise: a re-certification
that proves a different contract, in another environment, or for other bytes,
withdraws everything built on the old claim. No receipt issued before this
change cites anything, so none moves.

### 3. Non-empty `creates` is declined, with its evidence

Restating a dependency's closed non-empty `creates` needs a dependency document
with a `create` operation, and the generator to propose `create` operations on
the dependent. Neither exists (context above), and the semantic model defers the
second on purpose. An arm with no instance and no proposal to feed it would be
untestable against any published package, so it is not added. What blocks
`creates` on this chain is an **open** dependency `creates`, a different wall:
`solid-js`' host-free and `node` `createSignal` rows (ADR 0124, 0140) and the
`browser` census refusal inside `createHydratableSignal`.

## Consequences

Tests: the graph lane end to end
(`a_return_of_a_composed_dependency_call_restates_its_closed_plain_return`,
fixture `fixtures/package-contracts/dependency-plain-return`): `forward.js`
(`return count(items)`, the leaf closes plain) and `nothing.js` (`return
reset(items)`, the leaf closes `returns: []`) close one plain return;
`conditional.js`, `bound.js` and `unclosed.js` (the leaf's `widened` is the
amendment's refused reassigned `let`) stay open, and the leaf's own claims are
asserted beside them. The citation identity:
`a_citation_survives_a_re_issued_receipt_of_the_same_claim_only`, the receipt
framing test, and the bundler's "a citation names the claim" test.

Measured 2026-09-28 with `make primitives-checkpoint` (release build, 97
packages, three hosts). The tier was regenerated locally from each run's own
certifications (`bun scripts/bundle-accepted-contracts.mjs --run
run{,-browser,-node}.json`) and never committed; each regeneration feeds the
next run:

| run | code | tier | packages at 1-3 | criterion 1 | clean all hosts | none / `browser` / `node` |
| --- | --- | --- | ---: | ---: | ---: | --- |
| A | `65a2233c` | checked in | 1 | 27 | 92 | 95 / 95 / 92 |
| B | `65a2233c` | from A (466 bundles, all 44 citing ones dropped) | 1 | 40 | 92 | 95 / 95 / 92 |
| C | this ADR | from B (469, all 44 citing ones dropped) | 1 | 43 | 92 | 95 / 95 / 92 |
| D | this ADR | from C (513, **0 dropped**, 44 citing kept) | 1 | 86 | 92 | 95 / 95 / 92 |

- **The dependency return moved nothing**, as the demand census predicted: no
  export's `returns` cause changed between B and C in any host. The checkpoint
  corpus has no wrapper whose plain return is a dependency call.
- **The content identity is what makes the tier converge.** Regenerations 1
  and 2 dropped every citing bundle; regeneration 3, the first from citations
  that name the claim, kept all 44. In D the `node` walls behind uncited
  dependencies are gone: `unaccepted dependency` `@solid-primitives/event-listener`
  80 → 0, `queue` 7 → 0, `static-store` 6 → 0, and the `@solidjs/web` graph
  refusal attribution (93 exports) with them, because the exports it held now
  compose their dependencies by citation. Under `node`, media now cites
  `event-listener`, `static-store`, `rootless` and `utils`.
- **Every export they uncovered is held by a claim-form wall**, the ones the
  host-free run already shows: under `node`, `callbacks never proposed` 227 →
  320, `returns never proposed` 213 → 306, `reads never proposed` 151 → 229,
  `creates never proposed` 74 → 128. `createMediaQuery` under `node` is now
  exactly its host-free self: callbacks, returns and creates never proposed,
  reads declined for a `runtime-accessor-installation` hazard in its case.
- Criterion 1 in C and D reads the local tier, so it measures what a tier
  regenerated from this corpus would deliver (86 packages in the tier for all
  three hosts), not the checked-in tier.

## Still open

- **The chain itself.** `createHydratableSignal`'s `returns` needs an arm for a
  returned signal tuple, and its `creates` the `createSignal` rows for host
  free and `node` (ADR 0124's server-body review); `createMediaQuery` needs
  element-of-a-dependency-result and conditional forwarding, which this ADR
  keeps open by design.
- **The plain lane cites without a plan**, so neither this arm nor the
  `creates` dependency disposition reaches a cited dependency. Planning the
  cited dependency's artifact from its installed files would give both.
- **A dependency's parameter-shaped returns** (`parameter`,
  `invocation-result`, argument arrays: 45 closed tier summaries) would need the
  dependent's arguments mapped onto the dependency's parameters.
- **Non-empty `creates`** waits on `create` operations existing at all.
- **A late dependency withholding with no dependent closure to withhold**
  (the dependent's `returns` closure already withheld, its plain operation
  still relying on the claim) fails the graph closed rather than withdrawing
  the operation, because the graph's withholding pass carries closures only.
  No corpus row reaches it.
