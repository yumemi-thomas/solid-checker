# ADR 0158: Attribution follows extensionless source edges and class entries

- Status: accepted and implemented (2026-09-29); written with the implementation
- Date: 2026-09-29
- Owners:
  - the runtime edges (`packages/cli/scripts/artifact-resolution.mjs`,
    `runtimeModuleResolutions`, `uniqueBundlerLanding`);
  - the ladder's landing rule (`rust/crates/solid-facts-backend/src/main.rs`,
    `relative_landing`, `read_runtime_edges`);
  - the class syntax fact (`rust/crates/solid-facts/src/ast/class_obligation.rs`);
  - the super-argument rung (`export_names_of_super_argument_obligation`);
  - the reach walk (`rust/crates/solid-reactive-ir/src/attribution.rs`,
    `ObligationReach::class_sites`) and its reader
    (`export_names_from_reachability`);
  - the diagnostics: attribution records' `document`, the
    `<output>.attribution.json` sidecar (`generate-package-contract.mjs`), the
    graph node records (`graph-node-records.mjs`), the certification metric's
    `attribution widening` class (`scripts/certification-metric.mjs`), and the
    namespace-member package of a `dialect-silent` decline
    (`rust/crates/solid-reactive-ir/src/creates_walk.rs`).
- Relation: same family as ADRs 0132-0137 and 0142. It adds no rung and no
  claim form; it lets three existing rungs answer where they refused. No
  protocol change.

## Context

The tanstack scope (`docs/package-contract-v2/phase22/2026-09-29-tanstack-scope.md`)
found that `@tanstack/solid-router@2.0.0-rc.8`'s root node, on the rc.9
triple, had 22 obligations the ladder attributed `fallback-all`, each
marking all 95 exports. 13 of them sat at import specifiers. The scope
guessed at the cause: a sibling `.d.ts` beside every module.

**The cause was measured, not guessed, and the guess was half right.** The
native checker was wrapped to keep its argv, stderr and scratch batch inputs,
and each batch was replayed against the same bytes.

- The `[import]` case loads `dist/esm/*.js`. Its specifiers are
  `"./Transitioner.js"`, which ADR 0137 already joins across the `.d.ts`
  split. Three widenings were left there.
- The `[import, solid]` case loads `dist/source/*.jsx`, a bundler-only source
  build. Its specifiers are extensionless: `'./Transitioner'`, `'./route'`.
  ADR 0137's generator writes no edge for them, on purpose, because Node's
  ESM loader resolves none. So `runtime_symbol_redirects` has nothing to
  join. ADR 0135's guard (`imports_join_the_implementation`) and ADR 0134's
  package scope (`classes_stay_in_their_module`) each meet a relative
  specifier that "does not resolve to exactly one file", and they refuse.
  19 widenings came from this.

The experiment isolated that cause. The replayed batch's
`batch-runtime-resolutions.json` gained every extensionless edge with exactly
one candidate file (102 edges, none ambiguous). With the old binary that
alone did not help, because the backend's own landing rule still refused.
With the landing rule reading the edge, the source case's widenings fell from
19 to 7.

Two shapes were left once landings were exact:

- `this.notFound = (opts) => notFound(…)` in `class Route extends BaseRoute`,
  with `BaseRoute` from `@tanstack/router-core`. ADR 0134's amendment refuses
  every member position on a base this module cannot see, because that base's
  constructor may invoke the member.
- `super(options, getStoreFactory)` on `RouterCore`, whose accepted contract
  leaves `callbacks` open. ADR 0139 § 3 refuses any base that does not state
  a closed `result-access` item.

A third shape was left too: `primeRouterFromRegistry(this)` inside the
`Router` constructor. The reach walk stops at a class member, because member
dispatch enters it, so the reach is incomplete and reachability refuses.

**Why `createFileRoute` and `useNavigate` had no exact record.** No rung ever
named them. `useNavigate` holds no obligation of its own. `createFileRoute`'s
open facts arrive through `createRoute` → `new Route` → the class rung, and
that rung names the export lexically containing the `new` site (`createRoute`).
All of their open domains came from the widenings.

## Decision

1. **An extensionless relative specifier lands exactly when one file answers
   it** (the producer, `runtimeModuleResolutions`). Take a specifier that ESM's
   rule does not land, and whose own extension is no runtime extension. It is
   recorded as an edge when two conditions hold:
   - exactly one file exists among `base + ext` and `base/index + ext`, over
     every extension a mainstream resolver probes (`RUNTIME_EXTENSIONS` plus
     `.json`, `.css`, `.wasm`, `.node`);
   - that file is a runtime module of the same closure, and not a declaration.

   Two candidates, an index beside a file, or a candidate outside the closure
   write no edge. The document keeps version 1.

   The backend's `relative_landing` takes the ESM landing first. Failing that,
   it takes the edge for that exact `(canonical importer, specifier)`, and
   failing that, `None`, as before. A pair the document names with two
   targets is absent. Every relative-landing check in the ladder reads this
   one rule: ADR 0135's guard, ADR 0134's package scope, and ADR 0139's.
2. **Class entries answer as construction.** Each of these is attributed to
   the class's creators (ADR 0134 § 1), in every domain. Every other ADR 0134
   check still applies.
   - (a) *A member position on another package's base* (`class_obligation`).
     The base is a bare-specifier import. Its constructor may invoke any
     member during `super(…)`, so the member may run at construction. That
     is the construction answer, whose domains are a superset of the
     instance-member one. It stays exact only while no instance escapes this
     module's own code (`instance_escapes` over the involved classes). A
     sibling module's class, a global, or a local value that is not a
     module-level class still refuses.
   - (b) *A function passed to `super(…)` of a base whose accepted contract
     leaves `callbacks` open*, or closes it with no `result-access` item from
     that slot. It is construction, where it refused before. A closed
     `result-access` item with no call item, and a constructor that touches
     no member, still answers `SuperArgumentMember`.
   - (c) *A call site inside a class member on the reach path.* The reach walk
     no longer enters a class member. It stops at the site and records it in
     `ObligationReach::class_sites`, with
     `complete_outside_classes` saying whether that is the only gap. The
     reachability rung answers each site by the class rung at that exact
     site. Construction adds its creators. An instance member, or a site the
     class rung refuses, makes the whole rung refuse. The obligation's own
     function being a class member is no site: that reach stays incomplete
     for the class rung proper.
3. **The widening is kept as evidence** (diagnostics only).
   - The native attribution record names the `document` it was written for.
   - The generator keeps each document's `fallback-all` records (exports
     marked, domains, location folded to `<package-root>`) in
     `<output>.attribution.json`, beside the refusal audit rather than in it,
     so the corpus's `expected-refusals.json` pins keep their bytes.
   - The graph lane's node records carry them as `attributionWidenings`.
   - The certification metric classifies an unresolved claim that a widening
     covers as `attribution widening: fallback-all: <obligation>`, not
     `missing claim form: … never proposed`. It maps `reads` to
     `reactiveReads` and `creates` to `ownerRequirements`.
   - A `dialect-silent` decline for a namespace member (`Solid.createSignal`
     with `import * as Solid from "solid-js"`) takes its package from the
     namespace import it is rooted at: two-segment static callees only, and
     only a namespace binding. The metric prints `<no package>`, never
     `undefined`, where no resolved fact names one.

## Soundness

- § 1 names only which file loads. A resolver that resolves the specifier
  at all must load the one existing candidate. It is also the file the
  closure walk (`localModuleTarget`) already analyzed, so the edge adds no
  file to the closure. Every symbol-level join is still the compiler's
  (ADR 0137). Residual assumption, shared with ADR 0137 and the closure walk:
  no package `browser` field or bundler alias remaps a relative module path.
- § 2 (a) and (c) widen what the class rung already proves. Its creators are
  every export that can construct the class (ADR 0134 § 3 and § 4, unchanged).
  Code that runs during construction runs inside their calls. Construction
  marks every domain, `returns` included, so an instance member invoked
  later by a holder of the returned instance is covered too. An instance
  that escapes to this module's own code would let a non-creator invoke a
  member. That case refuses. A base that keeps the instance hands it only to
  dependency calls, which carry their own obligations (ADR 0134, Soundness).
- § 2 (b): a base that may call `F` at construction, or keep it for a member,
  is covered by construction on the same argument. A later call of a stored
  `F` from another export of the base runs inside that export. That export's
  accepted contract either states the invocation or leaves `callbacks` open,
  and its caller's obligation is attributed exactly.
- § 3 changes no claim, proposal byte, receipt or certification input.

## Consequences

- **Pinned** (each falsified separately by an env-gated build that disabled
  just that half):
  - `scripts/contract-runtime-edge-attribution.test.mjs`: `extensionless`
    now joins, while `ambiguous` (a second `.jsx`) and `indexed` (a
    `helper/index.js`) do not;
  - `scripts/contract-render-edge-attribution.test.mjs`: `split` renders
    across `./panel` with a sibling `panel.d.ts`, while `ambiguousSplit`
    does not;
  - `scripts/contract-class-attribution.test.mjs`: `inherited` is
    construction; `inheritedEscaping` still marks every export;
    `helperFromConstructor` opens only its creator; `helperFromMember` still
    marks every export;
  - `scripts/contract-super-argument-attribution.test.mjs`: `open` is
    construction;
  - `class_obligation` unit tests: a dependency base is construction; an
    escaping instance, a sibling-module base, a global or a local-value base
    refuses.
- **Measured** on router rc.8 × rc.9, host free, release binary, before
  (`9391013c`) and after (this ADR). The root node's all-export widenings go
  from 22 to 8 (router-core's 2 are unchanged):

  | widening | moved to |
  | --- | --- |
  | `dist/source/Transitioner.jsx` ×3, `headContentUtils.jsx` | `reachability` (§ 1) |
  | `dist/source/route.jsx` ×4, `router.js` ×2 | `class-construction` (§ 1; `notFound`: § 2 a) |
  | `dist/esm/route.js` `notFound` | `class-construction` (§ 2 a) |
  | `dist/source/registryTransfer.js` ×2 | `reachability` (§ 1, § 2 c) |
  | still `fallback-all`: `routerStores` ×4 (esm and source) | `super(options, getStoreFactory)`, then `primeRouterFromRegistry(this)` hands the instance to package code, so the super-argument site escapes |
  | still `fallback-all`: `not-found.jsx` ×2 | `getNotFound`'s callers are entered as component values in a memo `Dynamic` renders (ADR 0137's note) |
  | still `fallback-all`: `flightData.js` `applyResponseMetadata`, `scroll-restoration.jsx` | not investigated |

  **App sites that stop widening: 0 of 308.** The four `routerStores`
  widenings each mark all 95 exports in every domain, in both cases. So every
  demanded export's `callbacks`, `returns` and `creates` is still widened,
  and exports stay 2 clean / 2 partial / 94 degenerate. The metric now says
  so: `attribution widening: fallback-all: PackageContractExportMissing` or
  `: ReactiveDispatchUnresolved`, where it said "never proposed".
- **The next wall** is the escape in `Router`'s constructor. It needs a rule
  for an instance handed to a package function that the reach walk can
  enumerate. It is not decided here.
- **Known gap, not closed here.** ADR 0134 gives each `new` site to the
  export that lexically contains it, not to every export that reaches it.
  `wrapThing(o) { return createThing(o); }` still proposes `creates` closed
  while `createThing` does not (`contract-class-attribution.test.mjs`,
  `wrapped`). For the router this is `createFileRoute` → `createRoute`,
  masked today by the widenings. Only the certifier's own `creates` census
  stands between that proposal and a closure, and this ADR did not verify
  that it refuses.
- Coverage (131 projects) is unchanged. The contract corpus (112 fixtures)
  has no moved contract, plan, or walk verdict.

## Amendment of 2026-09-29: the `new`-site caller gap is sound, and still open

**Certification refuses it.** Measured on
`fixtures/package-contracts/class-creator-caller-creates`: a module-private
`class Thing` whose constructor calls `createEffect(read, () => {})`,
`createThing(read) { return new Thing(read); }`, and
`wrapThing(read) { return createThing(read); }`. The tracer
`a_creating_constructor_withholds_creates_from_every_export_that_reaches_it`
plans the fixture's own generated document byte for byte (integrity
rebound, as the census tracers do) and certifies it with the pinned
producer and probe harness.

- The generator proposes `creates: []` for `createThing` and `wrapThing`
  both. The gap is wider than the Consequences above say: no caller is
  needed. The proposal walk (`creates_walk.rs`) follows calls only into
  project *functions*. `new Thing(read)` resolves to the class's
  `Constructor` declaration (Type Facts' resolved declaration, span
  364..443 in the probe), and the walk never enters it. The same
  `createEffect` call in a plain function declines `dialect-silent`.
- The certifier's `creates` census walks the construction into the
  constructor (`census_call_disposition` dispositions `Construct` as it
  does `Call`). It withholds both closures by name at that call:
  `census refused: … (\`createEffect(read, () => {})\`)`. That refusal is
  directly on `createThing`, and on `wrapThing` through `createThing`.
  Neither can certify `creates` closed. The control `plain` passes the
  census; its closure is withheld only by the veto, because the probe
  worker cannot import `solid-js` in a transaction that carries no
  `solid-js` archive.

So no claim is withdrawn and no certified contract is affected. What
remains is a proposal that cannot certify:

- **Open (proposal side).** The walk should treat a construction whose
  resolved declaration is a project `Constructor` as an edge into the
  enclosing class's span. It must key on that exact resolved declaration,
  not on the class name: the class name has no entity in the walk's table
  (`classes_by_symbol` over `entity_symbol` found none in the probe). This
  was not implemented. The owner's priority moved to the
  `@solid-primitives` checkpoint.
- **Open (`Router` escape; dropped before investigation).**
  `routerStores.js`' four widenings still mark all 95 exports. Their
  super-argument site (`super(options, getStoreFactory)`) refuses because
  `Router`'s constructor passes `this` to `primeRouterFromRegistry`, which
  `SuperArgumentSite::escapes` counts as an escape. Narrowing it would need
  a premise that every function the escaping `this` reaches is enumerated
  by the reach walk and performs no instance-member obligation. That
  premise was not established or replayed here. Deliverable router sites
  on rc.8 × rc.9 stay 0 of 308.
