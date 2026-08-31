# Phase 21 plan — resolve checker-addressable fully refused rows

Date: 2026-08-30

Status: complete

Baseline commit: `870dcceb83d4aa72cb3671e4c41e503c5df018e4`

The baseline is the dirty Phase 20 implementation, identified by its generated
artifacts rather than by the commit alone:

- corpus report: `benchmarks/ecosystem/report.json`
- report SHA-256:
  `dfd72fa5d7e8108abdf840d0edfbb9d89cfd83df06ad03cbde2163c9e23894f2`
- Phase 20 ledger: `benchmarks/package-contract-v2/phase20/row-ledger.json`
- ledger SHA-256:
  `1cf22736be8a71205f59cd5cd1ec02f6be0dd1c977552a89cc645b0ce8b72107`
- uncached end-to-end corpus time: 118,810 ms

## Implementation result (2026-08-31)

The bottom-up published graph transaction is implemented without turning
proposal material into receipt authority. Graph preparation discovers
canonical nodes by full artifact and resolution identity, emits each node in a
dependency-first frontier, and submits one deduplicated case set to one native
certification transaction. Reuse is transaction-local: archive snapshots are
keyed by full published identity, dependency closures are keyed by the exact
resolved package root and are byte-revalidated on every hit, and Type Facts are
shared only by independently canonicalized equal source programs inside one
native emission request. Package roots, manifests, lock selections, and package
identity are reread at their authority boundaries. Failed dependency-closure
acquisitions are never memoized.

The native transaction independently replays the archive, lockfile, graph
root, proposal, source closure, and policy inputs, batches its Type Facts export
demands in one pinned producer session, finalizes each root bottom-up, and only
then issues and publishes receipts. A retained proposal-refusal audit is merely
an optimization: the certifier validates the complete current artifact-case
census and independently replays each exact refusal before using the same bytes.
Fresh-process ordinary analysis remains the acceptance authority.

The final focused uncached Corvu probe completed in 8,352 ms and certified in
7,997 ms. Proposal generation took 5,762.42 ms and native witness acquisition
2,014.89 ms. Its census records 18 root cases, 42 canonical nodes, 25 acquired
published artifacts, 179 compiler-closure sources, one native certification
transaction, and one Type Facts case-set batch; the fresh ordinary process
authenticated the receipt and selected the exact accepted case. The final
focused Kobalte probe completed in 21,144 ms, including 19,564 ms of proposal
generation for 629 artifact-case candidates. Exact-source-program reuse formed
621 programs and avoided only eight fact builds; unequal source programs never
share facts.

An earlier 19,907 ms Corvu result was rejected during handoff verification:
generic proposal batches had treated equal export conditions as sufficient
compatibility even when their exact source closures differed. The final design
does not use condition-only proposal batching or recursive eager child
certification. The contract corpus pins byte-identical singleton semantics.

The final-code certified-release uncached 418-row authority is
`benchmarks/ecosystem/report.json`, SHA-256
`c4fdede40e69dcccada59749d9fec277db4a5c0a06956d4b5ed1649f41d33478`.
It completed in 112,595 ms, 7,405 ms beneath the hard 120,000 ms budget. The
report contains 52 complete-contract generator successes, 324 partials, and 42
failures. It attempted 102
certifications: 41 certified and 61 refused. Aggregate worker time was 54,403
ms installing, 615,921 ms generating, 79,727 ms in other harness work, and
751,492 ms certifying; certification-stage totals were 56,384.71 ms artifact
acquisition, 596,721.74 ms proposal generation, 9,438.59 ms demand planning,
54,421.49 ms witness acquisition, 3.02 ms certification, 0.49 ms receipt
issuance, and 0.32 ms catalog publication.

The generated Phase 21 ledger preserves the frozen Phase 20 cohort and records
all 30 before/after dispositions. Of the 18 checker-addressable rows, Corvu is
newly verified through ordinary receipt loading, 15 retain exact dependency
proof refusals, Geolocation is honestly partial with its remaining absent
published target, and Context is a confirmed upstream declaration defect.
The 15 exact refusals are one missing installed `@solidjs/router`, four missing
installed `@solidjs/web` layouts, three unresolved TanStack package-import
targets, five value-export/function-effect normalization contradictions, and two
unsupported non-export Type Facts transcript demands. Their exact stage, owner,
and reason remain in each row rather than being collapsed into a claimed fix.
The other 12 controls remain fail-closed: five authenticated archives lack
required published bytes and seven packages have no supported runtime ESM
surface. The ledger is
`benchmarks/package-contract-v2/phase21/row-ledger.json`, SHA-256
`b4b2aee8545fa6a59732b932f2afaa97130382dbd00d8f92a5e153dfcce5b465`.

The corpus report preserves the benchmark's raw observed class, including six
legacy `unclassified` values. The Phase 21 ledger retains that field as
`observedClass` and separately supplies the authoritative exact
`terminalClass`: five `published-target-missing`, one
`published-declaration-closure-missing`, and Context's independent
`authenticated-dependency-layout-required` disposition.

One diagnostic-only limitation remains: the JavaScript dependency plan digest
contains temporary absolute paths and does not bind `rootIdentity`. It is not
trusted by native certification, receipt issuance, catalog publication, or
fresh-process selection; the native graph-root and authenticated inputs remain
the authority.

## Objective

Remove every current full-row refusal that is caused solely by a bounded,
proof-preserving checker limitation. Do not make a row green by dropping an
artifact case, guessing CommonJS exports, trusting an unauthenticated
dependency, treating missing bytes as runtime-inapplicable, or weakening proof
policy 2.

The maximum honest yield is 18 of the 30 currently fully refused rows. That is
a forecast, not the acceptance criterion. A row counts as fixed only after all
of its applicable artifact cases complete proposal generation and a fresh
ordinary analyzer process discovers, authenticates, selects, and queries its
policy-2 receipt. Advancing a row from fully refused to partial is progress but
is not a fix.

Five rows are upstream-only controls and must remain refused for their exact
published versions unless the benchmark deliberately changes version:

- `@kobalte/themes@0.0.1-next.0|solid1|only` — absent `dist/index.jsx`;
- `@solid-primitives/animation@1.0.0-next.1|solid2|floor` — absent dist and
  source targets;
- `@solid-primitives/animation@1.0.0-next.1|solid2|head` — the same exact
  archive under the other Solid 2 target;
- `@solid-primitives/composites@1.1.1|solid1|only` — absent dist payload; and
- `@solid-primitives/workers@0.4.3|solid1|only` — declarations import absent
  `./types.js`.

The checker may improve their classification, but it cannot certify semantics
for bytes the authenticated archive does not contain.

Seven CJS/no-ESM rows are also intentional retained refusals. Phase 21 will not
add a CJS export, interop, or module-initialization model. Keeping that scope out
of the artifact interface avoids a second execution model whose only purpose is
improving the benchmark count.

## Baseline disposition

| Cohort | Fully refused rows | Phase 21 disposition |
| --- | ---: | --- |
| recursive dependency-contract obligation | 16 | checker-owned, subject to exact terminal leaves |
| CJS/no-ESM export surface | 7 | intentional non-goal; retained fail-closed |
| geolocation export-kind conflict | 1 | checker-owned entity/proof reconciliation defect |
| archive-relative dependency declaration | 1 | conditional on authenticated installed dependency layout |
| missing published bytes | 5 | upstream-only, never checker-unblocked |
| **Total** | **30** | **up to 18 checker-addressable** |

The 16 dependency rows contain 203 terminal-leaf occurrences in the current
planning reports: 90 unavailable authenticated receipts, 45 unresolved
dependency identities, 24 artifact-resolution failures, 15 unsupported
external specifiers, 27 semantic `require` frontiers, and two nonliteral
dynamic-loading frontiers. These counts overlap across rows and versions. They
measure work; they are not independent row wins.

### Intentionally unsupported CJS/no-ESM rows

- `@solid-devtools/babel-plugin@0.3.1|solid1|only`
- `@solid-devtools/ext-adapter@0.17.0|solid1|only`
- `@solid-devtools/extension-adapter@0.12.1|solid1|only`
- `@solid-primitives/countdown@1.0.9|solid1|only`
- `@solid-primitives/date-difference@1.0.2|solid1|only`
- `@solid-primitives/reducer@0.0.101|solid1|only`
- `@solid-primitives/until@0.1.1|solid1|only`

### Dependency rows

- `@solidjs/testing-library@0.8.10|solid1|only`
- `@tanstack/solid-db@0.2.40|solid1|only`
- `@tanstack/solid-form@2.0.0-alpha.2|solid1|only`
- `@tanstack/solid-hotkeys@0.10.0|solid1|only`
- `@tanstack/solid-query@5.102.5|solid1|only`
- `@tanstack/solid-query@6.0.0-rc.0|solid2|floor`
- `@tanstack/solid-query@6.0.0-rc.0|solid2|head`
- `@tanstack/solid-query-persist-client@5.102.5|solid1|only`
- `@tanstack/solid-query-persist-client@6.0.0-rc.0|solid2|floor`
- `@tanstack/solid-query-persist-client@6.0.0-rc.0|solid2|head`
- `@tanstack/solid-start-server@1.167.36|solid1|only`
- `@tanstack/solid-start-server@2.0.0-rc.2|solid2|floor`
- `@tanstack/solid-start-server@2.0.0-rc.2|solid2|head`
- `@tanstack/solid-store@0.11.1|solid1|only`
- `@tanstack/solid-virtual@3.13.37|solid1|only`
- `corvu@0.7.2|solid1|only`

### Conditional and bounded singleton rows

- `@solid-primitives/context@0.3.2|solid1|only` — its declaration imports
  `../node_modules/solid-js/types/reactive/signal.js`; accept this only when the
  compiler-resolved installed dependency and its exact archive member are both
  authenticated.
- `@solid-primitives/geolocation@1.5.5|solid1|only` — the published runtime
  exports `createGeolocation` and `createGeolocationWatcher` as arrow functions,
  while the current runtime-kind join returns a closed non-callable answer.
  Correct the symbol/entity join; do not special-case these names.

## What the code currently does

The current implementation has most of the local proof pieces, but they do not
yet compose through one certification interface:

1. `scripts/ecosystem-benchmark/lib/dependency-plan.mjs` builds a complete
   installed dependency graph for reporting. Its own header correctly says
   that the graph is planning evidence only. Its receipt leaves are
   unauthenticated and cannot authorize a parent.
2. `solid-facts-backend::contract_certification::dependencies` provides a
   canonical dependency-first queue and structural demand schedule, but it
   deliberately cannot authenticate a dependency receipt or create an
   accepted-composition witness.
3. `packages/cli/scripts/certify-contract.mjs` acquires and certifies one root
   package/case set. It has no graph transaction and publishes no bottom-up
   dependency receipts.
4. `packages/cli/scripts/artifact-resolution.mjs` can record an accepted
   dependency when a caller supplies one, but the public generator's
   `prepareArtifact` path never supplies accepted dependencies. A raw caller
   digest must not become the missing interface.
5. Rust snapshot closure replay already uses scope-resolved Oxc module-load
   facts and can distinguish an unshadowed literal `require`. The fast
   JavaScript graph planner intentionally stops earlier at
   `semantic-require-binding`; native certification should reuse the stronger
   Rust replay instead of teaching JavaScript to guess.
6. Runtime artifact selection recognizes `.cjs`, but export enumeration and
   semantic entry binding intentionally support ESM only. Phase 21 preserves
   that refusal rather than adding CJS interop or module-initialization meaning
   to stable semantic-model version 1.

## Target design

### Review decisions applied before implementation

Three details in the initial draft were internally inconsistent or crossed an
existing authority seam. The implementation follows these corrected readings:

- The acquisition input is a finite **set** and the output is canonical, so a
  permutation of otherwise identical input nodes must produce the same graph.
  “Reordered nodes fail” means reordered or transplanted resolver/edge
  identities fail replay; the array order supplied by Node is never authority.
- Proposal inference still runs through the Rust analyzer and Type Facts
  process boundary orchestrated by Node. The graph transaction accepts those
  open proposal bytes as untrusted comparison material, normalizes them in
  Rust, and independently replays their artifact identities. Reimplementing
  semantic inference directly over tar members inside the certification module
  would duplicate the analyzer and violate the existing fact-domain seam.
- The Context row is subject to the absolute TypeScript boundary before any
  layout adapter is considered. Its exact published declaration already emits
  TS2307 under the real hoisted peer layout, so Phase 21 records a confirmed
  upstream declaration defect instead of manufacturing a nested dependency.
  See `context-upstream-declaration-defect.md`.

Put the new behavior behind one deep module in
`solid-facts-backend::contract_certification`. The conceptual interface is:

```text
certify_published_contract_graph(
    root_request,
    untrusted_artifact_set,
    certification_authorities,
) -> CertifiedPublication | ExactRefusalSet
```

This is one interface, not a set of caller-driven stages. Its implementation
owns:

- exact graph discovery and independent replay;
- registry metadata, archive, integrity, package/version, entrypoint,
  condition, and importer identity;
- compiler-attested ESM export identities;
- bottom-up proposal generation and policy-2 finalization;
- authenticated dependency receipt tokens;
- parent composition witnesses;
- graph-wide finalization and atomic publication; and
- a fresh-process discovery postcondition for the root row.

Node remains the acquisition adapter: it performs network and package-manager
I/O and passes untrusted bytes plus exact lock selections. Rust remains the
authority. No Node object containing a digest, package name, or claimed
accepted dependency can construct an authenticated dependency token.

Keep `AcceptedContractIndex` unchanged as the analyzer-facing interface. CJS,
package-manager layout, graph traversal, archives, receipts, and Type Facts
sessions remain behind the certification seam and never enter ordinary query
code.

CJS remains outside the supported artifact interface. No CJS adapter, export
census, interop branch, or module-initialization proof family is added in this
phase.

Do not add a public `moduleSystem` field to stable semantic-model version 1
merely for routing. Bind module format inside artifact/proof roots. If a real
module-initialization semantic claim cannot be represented without changing
normalized meaning, stop and write a separate semantic-model-version decision;
do not smuggle the claim into an export summary.

## Implementation slices

Each slice has its own positive and negative fixtures and may land only while
the five upstream controls and seven intentional CJS controls remain refused.

### Slice 0 — Freeze the live cohort and repair refusal taxonomy

Generate a Phase 21 ledger directly from the Phase 20 ledger/report schemas.
Every one of the 30 rows must retain its exact artifact cases, installed
identity, integrity, dependency plan, and current certification disposition.

Replace the generic `unclassified` aggregate for the six package/artifact
failures with exact top-level classes:

- `published-target-missing` for Kobalte Themes, Animation, and Composites;
- `published-declaration-closure-missing` for Workers; and
- `authenticated-dependency-layout-required` for Context.

Classification changes no proposal or certification result. Add a test proving
that the reason text cannot relabel a dependency or export-kind terminal state.

Exit gate: the generated cohort totals 30, the five upstream controls are
machine-identifiable, and no row remains generically classified.

### Slice 1 — Fix geolocation's exact export-entity join

Add a focused published-shape fixture containing exported arrow-valued const
bindings, declaration aliases, renamed re-exports, namespace re-exports, and a
non-callable sibling. Assert the exact facts at each stage:

- runtime export binding and compiler symbol;
- entry export entity location;
- runtime binding kind and call/construct signatures;
- `ExportKindProof`;
- inferred summary before and after reconciliation; and
- terminal proposal outcome.

Fix the general entity join or Type Facts demand span revealed by the fixture.
Never infer callability from `const`, an arrow token, an export name, or the
declaration file alone. Keep the existing invariant that a closed non-callable
proof carrying function domains is refused.

Exit gate: Geolocation either becomes proposal-complete with a compiler-backed
callable proof or exposes a new, narrower exact fact refusal. The negative
non-callable sibling remains a value and mixed/unknown symbols remain refused.

### Slice 2 — Promote dependency planning into native certification

Replace the benchmark-only plan as the authority path with graph planning
inside the deep certification module. Node supplies an untrusted finite set of
exact registry metadata/archive bytes and lock selections. Rust independently
replays every edge from immutable bytes and constructs canonical node identities
from:

- registry origin, package name, version, and integrity;
- entrypoint, condition set, importer, and resolution kind;
- runtime and declaration targets and digests; and
- closure and resolved-import roots.

Use `DependencyCertificationQueue` only after Rust has rebuilt the entire graph.
Cycles, duplicate identities, depth/node limits, missing lock integrities, and
graph disagreement remain exact refusals. The JavaScript report planner stays
as diagnostic comparison code until a test proves its graph matches the native
graph; it never supplies authority.

Exit gate: a synthetic two-node ESM wrapper produces a dependency-first native
plan from untrusted archives, while omitted, substituted, reordered, or
transplanted nodes fail before witness acquisition.

### Slice 3 — Certify leaves and compose authenticated dependency receipts

Certify graph nodes bottom-up through the same policy-2 transaction used for a
root. Keep each `AuthenticatedPolicy2Receipt` opaque inside Rust. Construct an
accepted-dependency composition witness only when all of these match:

- exact parent demand ID and semantic claim ID;
- parent export and selected artifact case;
- dependency importer/specifier and resolved-import root;
- dependency receipt semantic, artifact, policy, issuer, verifier-build, and
  revocation identities; and
- the final graph root after pruning/replanning.

Stage graph receipts and catalogs privately. Publish the graph and root only
after the root case set finalizes and a fresh process authenticates and selects
the root. A valid leaf may be reused within the same run only under identical
receipt and resolved-import identities; do not reuse by package/version alone.

Tests cover direct and transitive positive claims, external export-all, nested
installations, the same name at two versions, exact subpaths, conditions,
cycles, revocation, stale receipts, dependency receipt mutation, parent/root
transplant, and a leaf whose only known behavior comes from an authenticated
dependency.

Exit gate: the single-edge fixture and Corvu tier complete before attempting
the larger TanStack graphs. A raw `acceptedContractDigest` still cannot unblock
anything.

### Slice 4 — Close exact dependency graph frontiers

Process terminal classes in proof-authority order, rerunning only their focused
rows after each sub-slice:

1. **Authenticated receipt unavailable (90 occurrences).** Eliminate through
   Slice 3 bottom-up issuance, not through a trust shortcut.
2. **Semantic `require` binding (27).** Use Rust's scope-resolved module-load
   facts and snapshot closure replay. Literal unshadowed loads become exact
   edges; shadowed or nonliteral loads stay refused.
3. **Dependency identity (45).** Bind the resolver's package ID, installed
   manifest, lockfile selection, registry metadata, and archive integrity.
   Disagreement is a refusal, not a choice of whichever identity is convenient.
4. **Artifact resolution (24).** Re-run after exact dependency identity.
   Missing targets and CJS-only dependency leaves remain refused; unsupported
   condition/custom-loader cases remain local.
5. **Unsupported external specifier (15).** Classify built-ins, type-only
   declaration dependencies, optional/peer dependencies, assets, and package
   subpaths separately. A Node builtin is not a package receipt. It may be
   verifier-bundled only under an exact runtime-image/library policy, and only
   for the domains that policy proves.
6. **Nonliteral dynamic loading (2).** Prove the load unreachable from every
   retained export/domain or keep an exact source-bound refusal. Do not attempt
   arbitrary runtime string evaluation.

Exit gate: every former leaf is either an authenticated composition edge or an
exact terminal shape with source location and owner. The absence of a leaf is
never interpreted as successful closure.

### Slice 5 — Authenticate Context's archive-relative dependency layout

Add a resolver fixture matching
`@solid-primitives/context@0.3.2` and cover hoisted, nested, conflicting-peer,
symlinked, and absent layouts. Accept the declaration import only when:

- TypeScript's resolved-module fact selects an exact file;
- the resolver package ID and nearest manifest identify the expected
  dependency/version;
- the package manager's exact lock selection is authenticated;
- the selected bytes match a member of that dependency's authenticated
  archive; and
- the dependency receipt and parent composition bind the same edge.

The root archive is not allowed to claim the dependency member, and path
similarity under `node_modules` is not identity. If the exact corpus install
does not satisfy the conditions, retain the refusal and record that this
release needs republishing; do not create a fixture-only layout that the real
install does not have.

Exit gate: the real row certifies under its actual authenticated layout or
terminates as a confirmed upstream declaration defect.

### Slice 6 — Performance and run-wide reuse

The final corpus must stay below the existing two-minute requirement even as
more rows attempt certification. Add timings before optimizing and preserve the
current generation/certification overlap.

Within one run:

- deduplicate exact graph nodes only by their full canonical identity;
- batch compatible Type Facts demands into one pinned producer session;
- share parsed immutable archive members and Rust closure facts by digest;
- parallelize independent graph leaves under the existing global worker cap;
- reserve certification capacity so dependency graphs cannot starve proposal
  generation; and
- publish only after per-root graph finalization, regardless of work reuse.

Never persist or reuse an opaque authority token across a changed policy,
issuer, revocation epoch, verifier build, runtime image, importer/specifier,
condition set, or graph root. Extend the gate-cache key before relying on any
new result cache.

Exit gate: under the performance-first handoff, one final-code uncached 418-row
run with certification enabled completes in less than 120,000 ms on the Phase
20 reference host. Cache equivalence remains pinned by focused key,
invalidation, and transaction-isolation regressions; the handoff deliberately
requires one authoritative full-corpus run rather than a second warm 418-row
measurement.

### Slice 7 — Final corpus disposition

Run the full corpus uncached with fresh checker and Type Facts binaries. Emit a
Phase 21 ledger that records, for every original fully refused row:

- before/after proposal state;
- all accepted and refused artifact cases;
- the complete diagnostic dependency-plan roots and terminal leaves, retained
  explicitly as non-authoritative planning evidence;
- certification attempt, exact stage, owner, and reason;
- fresh ordinary-process receipt authentication and exact-case selection for
  verified rows; and
- an explicit disposition state and remaining owner for every row.

The ledger does not serialize the native graph-root, receipt, or catalog
identities. Native certification reconstructs and binds them, and focused plus
end-to-end tests enforce their non-transplantability, but their absence from
the generated ledger remains a telemetry limitation rather than evidence of
authority.

The target is 12 intentional retained rows—the five upstream missing-byte rows
and seven CJS/no-ESM rows—and up to 18 newly verified rows. Do not force that
number. If a checker-addressable row exposes a new semantic refusal, keep it
exact and report the reduced honest yield.

## Verification matrix

| Changed area | Focused check | Additional gate |
| --- | --- | --- |
| `solid-facts` module-load facts | `facts-lib` plus exact positive/negative AST tests | universal set |
| Type Facts export/dependency identity | focused Go producer tests and Rust client decoding | rebuild stamped producer, process fixture |
| reactive-IR proof demand/composition | `ir-lib` | coverage comparison, universal set |
| backend certification graph/receipts | `backend-process` and focused certification tests | contract conformance, coverage comparison |
| CLI acquisition/resolution | `bun run --cwd packages/cli test` | native process transaction tests |
| ecosystem planner/ledger/cache | focused Bun script tests | uncached corpus and ownership gate |
| fixtures/expected outputs | one non-updating focused coverage run first | ownership gate; update only intentional snapshots |

Every semantic slice also needs a published-typing `tsc --noEmit` oracle case.
No checker finding may duplicate a diagnostic from the package's real typings.

Before final handoff run:

1. the proportional focused checks above;
2. `make contract-corpus` with the fresh native binary;
3. the uncached ecosystem corpus with certification enabled;
4. the Phase 21 ledger consistency tests;
5. the ownership and TypeScript-oracle gates; and
6. `make verify`.

## Adversarial invariants

- A report graph, lockfile digest, proposal, or caller-supplied accepted digest
  is never receipt authority.
- CJS and module-initialization semantics remain unsupported and fail closed.
- A missing target, declaration, dependency, receipt, graph edge, or export
  binding is absence of proof, never proof of harmlessness.
- The five upstream controls and seven intentional CJS controls remain refused
  under their exact versions.
- An empty or CJS-only export census does not prove inert module initialization.
- A dependency receipt cannot be transplanted across package version,
  integrity, importer, specifier, subpath, conditions, policy, issuer,
  revocation epoch, verifier build, or graph root.
- Graph pruning forces finalization and replanning; an earlier receipt cannot
  bless the pruned graph.
- A shadowed `require` binding is an ordinary local symbol and never ambient
  loader authority.
- Dynamic export mutation and nonliteral loading remain fail-closed unless a
  finite exhaustive model proves them.
- Solid 1.x and Solid 2.0 vocabulary stays in the dialect owners; artifact and
  dependency certification remains dialect-neutral.
- Stable schema version 1 and semantic-model version 1 are not redefined to
  improve the row count.

## Definition of done

Phase 21 is complete when:

- all 30 baseline fully refused rows have a reproducible before/after
  disposition in a generated ledger;
- the six former raw `unclassified` observations have exact Phase 21 ledger
  `terminalClass` values while preserving `observedClass` for provenance;
- all seven CJS/no-ESM baseline rows remain explicit unsupported-artifact
  refusals and are not counted as verified;
- no row remains blocked solely because an available exact dependency receipt
  could not enter native bottom-up composition;
- every remaining dependency leaf names an exact unresolved identity,
  artifact, runtime/library policy, source-bound loader frontier, or semantic
  proof refusal;
- Context either verifies against its real authenticated installed layout or
  is recorded as a confirmed upstream declaration defect;
- Geolocation has a correct general export-kind disposition backed by the
  exact compiler symbol/entity facts;
- the five missing-byte upstream controls and seven intentional CJS controls
  are not counted as verified;
- every verified row satisfies the complete Phase 20 row definition and passes
  fresh-process receipt discovery;
- aggregate proposal and certification-stage timings are reported, and the
  single final uncached certification-enabled corpus run remains under 120,000
  ms;
- no TypeScript diagnostic is duplicated; and
- `make verify` passes with all generated artifacts reviewed.

The success metric is the number of formerly refused rows that become
independently authenticated without weakening the proof contract, accompanied
by exact terminal explanations for everything that remains.
