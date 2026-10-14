# Plan: giving up package contracts

Status: approved direction (owner, 2026-10-14). P0 measured; the bounded
tuple correction and independently reapplied P3 refactor are integrated.
P1 and the combined ADR 0309 production corrections remain held.

Dependency behaviour comes from analyzing installed package source (ADRs 0284,
0290, 0291, 0295–0300). Package contracts are removed: first their
consumption, then, one release later, the whole emission and certification
pipeline.

## Decisions

1. **No declared host, or no Solid Vite plugin.** Infer the host where it can
   be proven (ADR 0270). Otherwise every package call is uncertifiable.
2. **WASM.** A host protocol: the JavaScript host supplies package files,
   their Type Facts and resolution rows, and WASM runs the same source
   analysis.
3. **Non-Solid packages.** Analyzed from source when a selected package
   re-exports or forwards their values, or, by demand (owner, 2026-10-14,
   after P0), when a project call passes a reactive value or callback into
   one, or reactive code reads a value from one. Any other package stays
   unknown behaviour, which is uncertifiable.
4. **Pipeline.** Contract consumption is switched off first. The code stays
   for one release as a rollback, then everything contract-only is deleted.
5. **Rollback.** The switch-off is one commit, rolled back with `git revert`.
   There is no hidden flag, so no contract gate or fixture is kept alive for
   that release.
6. **Read-only projects.** The combined-program tsconfig is written to a
   scratch directory with the same anchoring, so these projects still get
   source analysis.
7. **Host-free runs, two passes.** The host is inferred from project code
   first, then package source is selected and analyzed under it. If the build
   (development or production) is not proven, or scopes mix browser and server
   execution, package calls stay uncertifiable.

Premises a clean result rests on, stated in reports: Vite with the Solid
plugin, standard runtime configuration, package integrity (ADR 0300), and the
Window realm where declared (ADR 0295).

## Integration cadence (owner, 2026-10-14)

Finish the current implementation rounds within their existing scope. Keep
their branches and review evidence intact while preparing smaller integration
slices. New independent findings become separately scoped work; they do not
automatically widen an active correction.

1. Review and land the smallest independently sound P1b slice, with its
   fixtures, snapshots and required producer/client changes kept together.
   The allowlist merge bar below still applies.
2. Integrate the reviewed P3 refactor after that slice when it clears its
   proof. If P1b remains held, P3 may be reapplied independently: its four
   selected patches have no semantic dependency on namespace admission or
   the handoff allowlist. Preserve the lead's admission decisions and require
   byte-identical findings against the actual new baseline; host extraction
   stays in P4. Do not import P3's P1b merge ancestry.
3. Isolate ADR 0309's shadow infrastructure from its production corrections.
   An isolated shadow slice must preserve public findings and keep disputed
   execution open in its audit. Hold production corrections that lack exact
   admitted-runtime evidence; a documented new false positive is not cleared
   for integration.
4. Keep the tuple correction conservative and bounded by its visible mutation
   evidence. Missing runtime authority remains an explicit uncertifiable
   result. Precision recovery, including the renderer cases, is separate work.

Before another semantic patch, write its proof premise, supported boundary,
decisive positive and negative regressions, and acceptance criterion. Review
that reasoning before implementation. Adversarial review remains required
before integration; each resulting commit must be independently green.
Use focused checks during a slice and one handoff verification pass after its
final change, as prescribed by AGENTS.md and verify-handoff.

## Phases

Each phase lands with fixtures, an adversarial review round until it comes
back clean, a per-site corpus sweep, and `make verify`.

### 0. Measure

A corpus sweep in source mode with every contract disabled, compared per site
with contract mode. The difference is the work list for phase 1.

**Result (2026-10-14, release binary at `d9ed2224d`, 49 projects, browser
host, no realm flag, no errors in any run):**

| Run | Violations | Uncertifiable |
| --- | --- | --- |
| default (source mode, contracts as fallback) | 561 | 3,239 |
| source mode, `--no-bundled-contracts` | 556 | 3,279 |
| contract mode, `--no-dependency-source` | 561 | 3,702 |

Disabling contracts drops one site and adds no violation:

- `probus-hk/src/components/VirtualRows.tsx:81:23`, which carries 5
  `strict-read-untracked` findings. The project imports
  `@tanstack/virtual-core` (a non-Solid package) **directly** in
  `src/lib/tanstack/virtual.ts`. Decision 3 covers a non-Solid package only
  when a selected package forwards it, so this case is outside it. The same
  direct import accounts for the `package-contract-incomplete` rows in
  ai-memory-ui, compass-ui and openbot.
- 40 openbot uncertifiable rows, all from callbacks passed to `Button`, which
  wraps `Root` from `@kobalte/core/button`. This is a subpath import, not a
  namespace import. The install is patched, and an authored contract covers
  it (`@kobalte/core@2.0.0-alpha.0+openbot-patch`). Without that contract the
  import reports `package-contract-incomplete` at `button.tsx:1:1`, so source
  mode does not analyze it. Why is still undiagnosed: the patch, the selection
  filter, or the subpath.

The other gap packages (`@kobalte/solidbase`, `@solidjs/start`,
`@rigelbuild/solid-markdown`, `@solid-primitives/controlled-signal`) move no
site on this corpus. Their phase 1 work rests on fixtures alone.

Contract mode compared with the default: the kui router getter
(`TopicsRoute.tsx:425:75`) and two spotify `prefer-show` sites (realm, which
`--runtime-realm window` recovers) exist only with contracts. The 3 probus
router reads exist only with source.

### 1. Close the source gaps

These are the six packages the corpus still keeps on contracts:

| Gap | Packages | Work |
| --- | --- | --- |
| Namespace imports | `@kobalte/core`, `@solid-primitives/controlled-signal` | redirect `ns.member` exactly; refuse escaping uses |
| Non-Solid packages | `@rigelbuild/solid-virtual` → `@tanstack/virtual-core`; direct imports of `@tanstack/virtual-core` (probus, ai-memory-ui, compass-ui, openbot) | decision 3, forwarding and demand |
| Patched subpath install | `@kobalte/core/button` (openbot) | diagnose why source mode refuses it |
| `browser` field map | `@rigelbuild/solid-markdown` | apply the map for a browser host, as Vite does |
| `?raw` / JSON modules | `@kobalte/solidbase`, `@solidjs/start` | model them as inert data modules |

Also in this phase:
- the contract-mode `export *` gap stops mattering once contracts are gone.
  Until then it is fixed (ADR 0301, in progress);
- optionally, kui's router getter through package-object escape analysis
  (parked in docs/precision-backlog.md).

**Exit criterion (owner, 2026-10-14):** no false positive and no wrong
certification. A proof that contracts give and source cannot is an honest
uncertifiable result, not a phase 1 work item. Under this criterion:
- the hand-off allowlist (ADRs 0307 and its round 3) lands, because the
  review rounds showed that source mode at `d9ed2224d` certifies untracked
  reads through ordinary helpers;
- demand selection (ADR 0304) lands only within about 1.5× each project's
  wall time and without losing a proof;
- the virtual-core constructor proof and kobalte's `Dynamic` callback timing
  move to phase 1b;
- the `browser` map and the `?raw`/JSON modules need fixtures only, since
  they move no corpus site.

**P1b allowlist merge bar (2026-10-14):** the allowlist (ADRs 0305, 0307,
0308) merges when a review confirms no tsc duplicate, no false positive, and
no wrong certification that round 2 or the base refused. Remaining
wrong certifications that the base also has are recorded in
docs/precision-backlog.md with ADR 0309 as their owner, because each review
round finds new shapes and ADR 0309 is the structural fix.

### 1b. One value-flow fact

ADR 0309: one fact records where each proven reactive value can flow and
which call sites can read it. It replaces the shape-by-shape slices whose
gaps the phase 1 reviews kept finding. It is built in shadow mode, at least
as strict as the allowlist, and then takes over consumers one at a time.
The recall that phase 1 deferred comes from this fact.

### P3 decisions (owner, 2026-10-14)

Design: `rust/target/research/p3-package-boundary-design.md`. P3 is a pure
refactor, five individually green commits, no finding moving:
1. Recorded reasons do not emit new SC9005 findings; wording changes at P7.
2. Every cause is kept internally; today's collapsed output stays until P7.
3. Host extraction only; inference changes belong to P4.
4. Demand selection of non-Solid packages lands only after its recall and
   cost gate; P3 does not depend on it.
5. No new refusal of patched installs or installs without integrity data.
6. A workspace link in the analyzed program is project code, not unknown.
7. `packageSummaries` and notes stay byte-identical through P3; P7 decides,
   with a changelog.

Clarification (2026-10-15, ADR 0310): item 1 concerns *recorded reasons*, which
still emit nothing. A **source-edge refusal** is a different input: an
explicitly recorded exact occurrence that the analysis reports as one neutral
SC9005 finding and whose accepted contract it withholds. It is a dormant
consumer — no production caller records one — and the producer, cache,
admission, emission and certification obligations stay open (ADR 0310).

P3's four selected slices are integrated through the independently reviewed
`p3-independent` extraction, with no finding moving. The original
`p3-boundary` history remains intact. Slice 4, extracting the host index from
the contract views, touches every IR consumer, so it becomes the first commit
of P4, under the same byte-identity gate.

P3 was reapplied independently while the P1b allowlist remained held, under
the integration cadence above. Subsequent work uses the shared analysis-input
and `SemanticLookup` seam; the shadow slice must add no production authority.

### 2. Host-free runs

Host inference supplies a browser host where it is proven, without contract
views. An unprovable host means package calls are uncertifiable, and SC9005 is
reworded to "package behaviour unknown" or replaced.

### 3. WASM host protocol

Extend the WASM request with selection rows, package files, Type Facts for the
combined program, and fold inputs. Selection, redirects, folding and analysis
are shared with the native path.

### 4. Switch off consumption

- No catalog, authored tier or bundled contract is bound.
- `--no-dependency-source` is removed.
- Snapshots, gates and docs move to source-only.
- The pipeline code stays one release as the rollback.

### 5. Delete

Everything that exists only for contracts goes:
- consumption, catalogs, admission, trust and receipts;
- the authored tier, emission and certification;
- probes and proof sidecars;
- the contract corpus and registry pins;
- the schema, CLI contract commands, their fixtures, gates and docs.

The exact inventory is in
`rust/target/research/contract-deletion-inventory.md`, checked against
`8f70a211e`. It covers about 250k Rust and Go lines, 110k JavaScript lines and
3,000 data files. Some IR types named `Contract*` carry ordinary analysis
summaries, so they are renamed rather than deleted.
Shared parts stay: the dialect vocabulary, Solid's runtime model, Type Facts
used by ordinary analysis, host constants and host inference.

## Removal order

Each step lands green on its own. The inventory's section 5 has the details.

| Step | Content | Size |
| --- | --- | --- |
| P0 | Measure: per-site sweep with contracts disabled; a dual-mode comparison script | small |
| P1 | Source gaps: namespace redirects, non-Solid packages, `browser` map, `?raw`/JSON modules; seed a source-selection corpus gate; hand-off allowlist | large |
| P1b | Value-flow fact (ADR 0309): shadow mode, consumer moves, then the deferred recall | large |
| P2 | Neutral IR vocabulary: move the `Contract*` summary types (no behaviour change) | medium |
| P3 | `PackageBoundary`: imports that cannot be analyzed, each with a reason; host passed separately (no finding moves) | medium |
| P4 | Host-free runs (decision 7) | large |
| P5 | WASM host protocol | large |
| P6 | Move callers and fixtures off contract mode; scratch tsconfig for read-only projects | large |
| P7 | Switch consumption off (one commit, the revert point) | medium |
| — | one release | — |
| D1–D8 | Delete consumption (backend, IR), emission, certification and probes, the dialect's negative authority, the Type Facts certification protocol, historical ledgers and benchmarks, schemas and docs | large in total |

These constraints are fixed:
- P1b comes before P2, whose IR renames collide with it. P3–P5 are backend
  work and run in parallel with P1b (owner, 2026-10-14).
- P2–P6 come before P7.
- P1's seed recycling comes before D3.
- D6 comes after D4.

### P2 status

P2 is not complete; only its first slice, the shared callback and return
summary vocabulary (`summary_semantics`: `InvokeProtocol`,
`enumeration_get_is_guaranteed`, `CallbackSchedule`, `CallbackSummary`,
`ReturnSummary`), has moved. It is a pure move with the old spellings kept as
aliases and re-exports, so a rebase onto any held branch needs no rename.
Recorded for the remaining P2 slices:

- the ordinary export summary (`ContractExport`, `ContractClaim`,
  `ContractReactiveRead`, `ContractOwnerRequirement`) mixes ordinary fields
  with contract-only ones (`capture_sources`, `captured_arguments`,
  `event_handler_props`, `open_claims`, `leaf_forbidden_operations`,
  `callback_results`, `*_closed_empty`); it needs field pruning, not a rename,
  and the ordinary owner requirements must survive it;
- `own_returns` is a pure AST helper with ordinary readers and belongs with a
  returns-walk or facts-side slice;
- the contract-side `wire_name` and `enumeration_cardinality`, the
  `ReturnSummary::prototype` edge to `PrototypeInstanceRecipe` and
  `prototype_instance.rs`, the escape/stability helpers and the return-kind
  constants stay while contract emission or consumption exists. The
  contract-removal inventory allocates the prototype model to D3 while this
  slice's design placed it at D2; the disposition is compile-driven, not an
  unconditional deletion at D2;
- the `ContractCallback` / `ContractReturn` aliases and the
  `contract_semantics::InvokeProtocol` re-export stay through the rollback
  release and leave when their last reader migrates (a count ratchet:
  `rg -w "ContractCallback|ContractReturn"` must not grow).

## Risks

- Every new source path needs review rounds; so far each round has found
  contrived soundness holes, all of them fixed.
- The corpus is small (49 projects). Use the seeded-defect harness
  (`solid-checker-rc13-corpus/experiments/recall`) and bug-fix mining to
  check that recall does not drop at each phase.
- Host-free users lose package behaviour until inference covers them.
