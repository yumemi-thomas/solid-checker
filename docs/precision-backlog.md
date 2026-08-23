# Precision backlog

## Uncertifiable baseline and evidence-owner matrix (2026-08-21)

The dirty-worktree baseline was coherent on the current source: the Reactive
IR library tests passed, all 76 armed backend process tests passed, and the
fresh-debug-binary coverage comparison passed for 72 fixture projects (517
findings). After the reviewed runtime-identity, environment-selector,
package-owner, closed-local-callback, dialect-selection,
rendering-premise, caller-witness, callback-extent, nested-transport,
object-graph, and program-boundary slices below, the snapshots contain 130
\`uncertifiable\` findings across 524 findings in 77 fixture projects. This is an inventory of the current proof obligations, not a
promise that every row is reducible; the last column records the only sound
owner that could discharge it.

The count moved 126 → 130 while precision improved, and the two are not in
tension. Every new proof path below needs its fail-closed controls pinned:
\`props-caller-witness\` contributes two honest uncertifiable results doing
exactly that, and the nested-transport slice two more. A negative control that
reports an obligation is the evidence that the new positive path did not
overreach; deleting one to lower the number would remove the only thing holding
the reduction honest. **The count is not a precision score, and the sections
below are the reason.**

### The corpus was the blind spot, not the facts (2026-08-21)

All four fact sources are saturated: every field `EntityFact` emits (15), every
`AstFacts` table (28), every schema-v1 contract property (39), and every
compiler execution-map table is consumed by a rule. "Extract more from the
producers" is largely finished as a strategy, and the two capabilities that
were genuinely missing are now supplied.

What is not finished is the corpus. Three real defects were found in one week
— a discarded caller witness, a false violation from a mis-attributed callback
read, and an undemanded nested library identity — and **none of them moved a
single fixture** across 76 projects. All three were found by writing a scratch
project by hand. The fixtures test the shapes their authors thought of: the
`interprocedural` fixture was two files and seventeen lines with one shape, and
the component fixtures are single files whose exported components have no
callers at all.

Two things came out of that:

- `scripts/obligation-audit.mjs` turns the manual probing into a gate. Every
  obligation states the evidence that would settle it and what the checker must
  say once it is present, so an over-conservatism can no longer pass as a
  missing fact. An obligation that closes on its own fails too — that is a real
  change, and it should be recorded rather than absorbed. Seven obligations,
  eleven closures, run against the audited published typings.
- `fixtures/reactive-ir/realistic-topology` is a project shaped like a project:
  components in their own files rendered by other components, a helper called
  from a component body, a module-scope source read across files. Under a closed
  boundary it produces six analyzed sites, three findings, all proven, and no
  obligations — which is what a well-analyzed application should look like, and
  is only reachable because the topology supplies the facts.

### Where the floor actually is (measured 2026-08-21)

The remaining obligations were probed rather than classified by eye: for each
large cluster, the closing evidence was supplied in a scratch project and the
checker was asked whether it closes.

| Cluster | Count | Closes when the evidence is supplied? | Why the fixture case cannot |
| --- | ---: | --- | --- |
| SC1001 / SC1003 props | 49 | Yes. A dynamic in-project caller proves the violation; a complete static caller set certifies it silent. | The engine and corpus fixtures have no in-project callers at all. |
| SC4001 owner | 27 | Yes. A \`createRoot\` call site certifies silent; a module-scope call site proves the violation. | Exported helpers whose caller set is incomplete. |
| SC9005 | 22 | Not applicable. | Wrong subpath, absent export, or unreviewed package — the fixtures exist to pin that. |
| SC9012 | 9 | No. | Divergent dispatch, globals, and opaque adapters by construction. |
| SC7005 | 5 | Never. | Per-request settlement race; no source fact decides it. |
| Type Facts–owned | 17 | Partly — and no longer fact-limited. | Broad numbers, non-exact tuples, dynamic serializer config. Object graphs now certify or report; what remains there is a non-finite `number` and three deliberate controls. |

Only **two** of the 130 were ever limited by the facts themselves, and both
were found by reading the produced facts rather than the rules. Both are now
closed, and the object-graph residue is at its floor — see the
nested-transport and object-graph entries below. What is left in that row is
irreducible in principle (a broad `number` may be non-finite) or a deliberate
control (a getter, a twice-referenced binding, a spread).

So the number is bounded by the corpus, not by the checker: the top two rows —
76 of 128 — are cases where a fixture deliberately withholds the closing
evidence, and the machinery demonstrably closes them the moment it is present.
Lowering those rows would mean adding callers to fixtures whose purpose is to
pin the open-world boundary, which destroys the case rather than improving
precision. Two rows are irreducible in principle.

One genuine fixture-hygiene item is recorded rather than taken: several SC4001
obligations in \`dialect-solid-1x\`/\`dialect-solid-2\` are *incidental* to
fixtures whose subject is \`createEffect\` argument shapes, and the technique
that removes them without weakening the claim is now demonstrated in
\`summary-callback-extent\` (render the host at an exact JSX call site so it is
a proven owner, or wrap the call site in \`createRoot\`). It was not applied
here: that pair is the pinned differential-dialect fixture and keeps message
wording, so restructuring it is a deliberate change on its own, not a
by-product of chasing a count.

| Finding | Count | Current contexts | Missing evidence and audit classification |
| --- | ---: | --- | --- |
| SC1001 | 35 | component props aliases/read sites in the engine and eslint corpora; Solid 1.x sources; \`solid2-precision\`; v1 reactivity; upstream component cases | Exact JSX callers, immutable/enumerable prop backing, or a component contract. Project IR can reduce closed-world/cross-file cases; exported/open-world props remain genuinely uncertain. |
| SC1002 | 1 | \`props-callers\` callback after \`await\` | Exact synchronous callback extent and caller-proven prop/accessor identity. Project IR/compiler facts are reducible; opaque callbacks remain fail-closed. |
| SC1003 | 14 | component parameter/body destructuring in engine/corpora and wrapped components | Proven component identity plus exact prop backing/caller set. Project/compiler facts can reduce exact JSX calls; ordinary/exported components remain uncertain. |
| SC1004 | 2 | conditional component returns in the engine corpus | Proven component execution identity and return control-flow shape. JSX/compiler evidence is reducible; unknown component calls remain uncertain. |
| SC1007 | 3 | reactive handler reads in shared Solid 2 and v1 reactivity fixtures | Exact runtime handler domain/tuple shape and reactive prop backing. Existing TypeFacts closes exact values; mixed/opaque prop sources remain uncertain. |
| SC3001 | 1 | exported \`onSettled\` helper in \`leaf-owner\` | Exact callback identity and synchronous dynamic extent. The exported helper's owner call sites remain open-world; closed local callback adapters are now followed. |
| SC4001 | 27 | exported/ambiguous component and helper owners across dialect, engine, corpus, and precision fixtures | Compiler owner regions, exact caller topology, and package callback owner behavior. Local/closed callers are reducible; exported library callers and conditional owners remain open-world obligations. |
| SC5001 | 1 | async boundary with opaque source options | Exact option-object initializer (\`loadingValue\`/\`seedLoadingValue\`) and selected runtime entry. TypeFacts/options facts and explicit runtime conditions are reducible; dynamic options remain uncertain. |
| SC5003 | 1 | unresolved CSR/SSR boundary fixture | An explicit rendering selector now discharges this outright: \`rendering: "csr"\` proves the premise false and SSR proves it true. The remaining case has no selector, and no visible server entry does not prove CSR-only, so it is reducible only by user configuration or a cross-project compiler fact. |
| SC7001 | 2 | spread-hidden Solid 1.x/2.0 effect callback and \`"use server"\` controls | Exact tuple-slot/expanded spread facts plus selected runtime/framework entry. TypeFacts tuple arity is reducible; framework directives without an explicit compiler contract remain uncertain. |
| SC7005 | 5 | HTTP response writes in CSR and SSR flush fixtures | Request-dependent settlement relative to shell flush. An explicit \`rendering: "csr"\` selector now discharges the claim entirely — no shell, no committed response head — but that removes the subject rather than deciding the timing. Where SSR is selected or unresolved, whether a boundary settles before or after the flush is a per-request race no source fact can decide, and all five remain irreducible. |
| SC7007 | 4 | server-function rich arguments and dynamic serializer configuration | Exact immutable serializer options and closed finite literal graphs. TypeFacts can reduce exact constants/primitive domains; arbitrary object graphs, casts, spreads, and dynamic configuration remain uncertain. |
| SC9005 | 22 | missing/partial Solid contracts, unknown package callbacks/exports, wrong subpaths, and stale fixture contracts | Exact reviewed package/entrypoint/export summaries, runtime identity, and selected variants. Contract schema/generator/consumer parity and bundled ecosystem coverage are reducible; unreviewed or absent packages remain correctly uncertain. |
| SC9011 | 1 | exported reactive source in v1 reactivity | Exact caller capture or package/source contract. Closed local callers are reducible; an exported source escaping to uncontracted code is genuinely open-world. |
| SC9012 | 9 | divergent method dispatch, opaque package/leaf callbacks, structured returns, and Solid 2 precision | Exhaustive equivalent target summaries, exact returned adapters, callback owner behavior, and contract propagation through aliases/re-exports. Indexed identity/contract fields are reducible; divergent/opaque targets remain fail-closed. |

The package-contract audit therefore starts with SC9005/SC9012 and the
contract-owned portions of SC1001, SC3001, SC4001, SC7001, and SC9011. The
environment-dependent SC5003/SC7001 paths and the TypeFacts-owned SC5001/
SC7007 paths are separate workstreams. SC7005 is intentionally retained in
the irreducible ledger even when SSR is explicitly selected.

- **2026-08-21 — an explicit program boundary is evidence, and it is the
  largest lever in the corpus.** Seventy-six of the obligations came from one
  assumption: an exported symbol may be imported by code this build cannot
  see, so its callers cannot be enumerated and neither its props' backing nor
  its owner can be settled. Nothing inside a tsconfig proves the opposite,
  which puts this in the same class as `rendering` — a premise only the user
  can supply. `RuntimeEnvironment::program_boundary` now carries it
  (`--program-boundary open|closed`, `"programBoundary"` in
  `.solid-checker/runtime.json` and in the ESLint adapter's runtime settings,
  where it joins the snapshot cache key).
  Selecting `closed` removes exactly one assumption: that an *additional,
  unseen* caller exists. It licenses nothing else. Two places consume it, and
  each drops one open-world artifact. `classify_one_component` stops treating
  exportedness as an escape, and stops treating an `export { Card }` specifier
  as a non-JSX reference — that specifier reaches an importer only if one
  exists, and under a closed program every importer's use is itself in the
  reference list. Aliasing and passing the component to a receiver still
  escape, because closing the program says nothing about what the receiver
  does. The owner graph stops *seeding* an exported non-component helper
  `UNOWNED`; that seed is the unseen caller, and with it gone the enumerated
  call sites decide the owner.
  Everything else is unchanged and deliberately so: a caller set must still be
  enumerated exactly, a reference that resolves to a use the analyzer does not
  understand still escapes, and a missing reference list is still the absence
  of a fact rather than proof of no callers. `program-boundary-closed` pins all
  five rows, including the two that prove the assertion cannot manufacture a
  finding (a dynamic witness and an unowned module-scope call are violations
  either way) and the one that proves it is not a blanket amnesty. A unit test
  pins that the boundary never reaches `selected_conditions`, so asserting a
  closed program cannot silently select a different package entrypoint. The
  open-world fixtures keep the default and keep their obligations: they exist
  to pin what is provable *without* this premise.
- **2026-08-21 — the object-graph floor: the binder resolves the reference,
  and a property kind closes the literal.** Two facts finished the job the
  nested-transport slice started, and each removed a different obstacle.
  `ArgumentFact::binding_declaration` records the declaration Oxc's scope tree
  resolved an identifier argument to — the same contract
  `ObjectPropertyFact::shorthand_binding` already carried. The demand plan now
  follows `save(payload)` to the literal `payload` was built from through the
  binder's own answer: one reference, one declaration, one literal. No
  spelling match, and none of the file-wide sweep that made this look
  unbounded. `ObjectPropertyFact::data` records whether a property is a plain
  data property (`kind: Init`, not a method), which is the fact a consumer
  needs to close a literal *against accessors* and so conclude something about
  every value in it. Without it `{ get when() { return new Date(); } }` is
  indistinguishable from `{ when: "2026-01-01" }`, and would read as JSON-safe
  when it is not; `exact_object_literal` carries the same guarantee but only
  for a literal written directly as an argument.
  Together they make the graph walk two-sided: witnessing a rich leaf needs
  only that nothing displaced it (no spread, no computed or duplicate key),
  while certifying the graph safe additionally needs every property to be a
  data property with a proven JSON-safe leaf. `savePlain(payload)` and a
  nested container now certify silent, `saveBoxed(boxed)` is a proven
  violation, and the getter, twice-referenced-binding, and spread cases are
  each pinned as obligations. Both new demands are cache-stable — a library
  identity and a primitive domain are the same for every inhabitant of a type,
  so unlike a type descriptor or a constant value neither makes `{ n: 0 }` →
  `{ n: 1 }` invalidate the table. Performance re-certified.
- **2026-08-21 — JSON reaches nested values, and so does the proof.**
  `JSON.stringify` flattens a Date sealed inside an object exactly as it
  flattens a top-level one, but SC7007 only ever checked the argument's own
  library identity, so `save({ title, when })` shrugged where `save(when)`
  reported. The demand plan now asks for the same library identity at each
  property value of an object-literal argument — spans taken from *inside* the
  argument, so the cost is bounded by the argument rather than the project, and
  a library identity is stable across a type's inhabitants, so unlike a type
  descriptor or constant value it cannot make `{ n: 0 }` → `{ n: 1 }` a
  table-invalidating edit. The consumer witnesses a rich leaf at any depth.
  This is the *presence* half of the proof and only that half: it never
  concludes a graph is JSON-safe. Every condition is a soundness requirement —
  no spread at any depth (a later spread overwrites an earlier explicit
  property), no computed key (it may collide with the witness's name), distinct
  static keys (a duplicate later key wins), and for the through-a-binding path
  an immutable binding referenced exactly once, so nothing can mutate a
  property or hand the object to something that does between construction and
  the call. Shorthand properties resolve through the binder's recorded
  `shorthandBinding`, because a symbol query at a shorthand span answers with
  the property's symbol, never the value binding's. The oracle ledger entry
  that documented the old limitation now records the proof; `tsc` is still
  silent there, so this remains transport behavior no signature expresses.
- **2026-08-21 — a caller witness survives the open world.** Caller-proven
  prop reactivity is two questions with opposite quantifiers. "Some caller
  passes a reactive expression" needs one witness and is *monotone*: a
  consumer outside the project can add a call site, never unwrite the one
  written here. "Every caller passes a static value" is falsified by a single
  unseen caller and needs the complete set. `PropsReactivity` had one state
  for both — an exported component collapsed to "nothing about its props is
  provable" — so an in-project `<C title={n()} />` was discarded and the
  untracked read of `title` reported an obligation where a violation was
  proven. The state is now `Escaping`, carrying the witness sets from the JSX
  call sites that *are* visible while refusing to conclude `Static` for
  anything. Witnesses are per prop name, so one dynamic prop does not make
  every prop on the same component report, and a spread anywhere on an element
  discards that element's witnesses entirely (a later spread wins over an
  earlier explicit attribute, so it can overwrite a dynamic value with a
  static one). `props-caller-witness` pins all four rows. No existing fixture
  moved: none has an exported component with an in-project dynamic caller,
  which is why the over-conservatism survived.
- **2026-08-21 — a read inside a tracked callback is not its caller's read.**
  The interprocedural read summary excluded exactly one shape — Solid 2.0's
  `createEffect` *apply* slot, matched by primitive name and `Deferred`. A
  Solid 1.x effect's callback is `Tracked`, so it fell through, and a helper
  whose only read sat inside `createEffect` exported that read to its callers.
  Calling it from a render scope produced a **proven SC1001 violation** for a
  read that never happens at the call site — while the identical read inside
  the helper was correctly silent. The two halves of the analyzer disagreed
  and the interprocedural half was wrong; a false violation is worse than a
  missing one. The filter is now `read_escapes_synchronous_extent`, keyed on
  the dialect's own callback vocabulary: `Inline` reads "subscribe whatever
  was tracking at the call site" and propagate, while `Tracked` and `Deferred`
  do not. It also requires a function literal between the read and the
  argument, so an eagerly evaluated argument — `onMount(compute(count()))`,
  where the slot is Deferred but nothing defers the read — still propagates.
  `summary-callback-extent` pins all five executions and reports three
  violations and nothing else. No existing fixture moved: the corpus's
  `interprocedural` fixture covers only the direct-read case, which is why
  this survived.
- **2026-08-21 — a silently mis-dialected parity corpus, and the gate that
  catches the next one.** `fixtures/reactive-ir/eslint-plugin-corpus-v1`
  shipped an *empty* `node_modules/solid-js/` directory. Git cannot record an
  empty directory, so the stub never existed, dialect selection fell back to
  the 2.0 default, and the fixture named `-v1` — with a `solid-js.d.ts`
  headed "Solid 1.x declarations … verified against solid-js 1.9.14" — pinned
  the 2.0 catalog. Its snapshot recorded that as if intended: no rule carried
  the `v1/` prefix. Adding the tracked stub moves 10 findings. Four SC9005
  obligations discharge against the reviewed bundled Solid 1.x contract, which
  exports `Index`, `mergeProps`, and `splitProps` from `.` — they were only
  missing because the *2.0* contract was being consulted. Seven SC1001/SC1003
  props obligations become proven violations, because 1.x props are always the
  compiler's reactive proxy while 2.0 needs caller-proven backing. Three
  findings on files named `*-valid.tsx` disappear: two SC7001
  `missing-effect-function` and one SC2001 were the 2.0 catalog misreading
  1.x `createEffect`, i.e. false positives on the corpus's own negative cases.
  Four SC1001 violations inside `Show`/`For` callbacks become uncertifiable,
  which is correct and not a regression: `direct_jsx_return_is_component()` is
  false for 1.x, where an exported PascalCase function returning JSX may be a
  *tracked render callback* rather than a component, and its callers cannot be
  enumerated. `control-flow-invalid.tsx` was a byte copy of the 2.0 corpus
  file, `keyed` prop and all; 1.x `For` has no `keyed`, so `keyed={false}` and
  `keyed={item => …}` are TS2322 against the fixture's own 1.x declarations.
  The accessor-item case is now spelled the way 1.x spells it, with `<Index>`;
  custom keying has no 1.x equivalent and stays covered only by the 2.0
  corpus. `fixtures/package-contracts/solid-reexport` had the second shape of
  the same trap — a stub present locally but with no `.gitignore` exception,
  so absent in CI. `scripts/coverage.mjs` now holds every fixture dialect stub
  to being present, parseable, versioned, and git-tracked, and names the
  `.gitignore` lines to add; both shapes fail it.
- **2026-08-21 — an explicit client-only rendering selector is evidence.**
  `project_server_renders` returned a bool, folding "the user selected CSR"
  into the same state as "no server entry is visible here". So selecting
  `--rendering csr` produced an SC5003 uncertifiable result whose own message
  read "the analyzed project cannot prove whether a server-rendering entry
  exists" — it could; the user had said so. The fact is now three-state
  (`ServerRenderingPremise::{Renders, ProvenClientOnly, Unresolved}`). A bare
  `ssrSource: "client"` source is a server-render hole only under `Renders`
  and a proof obligation only under `Unresolved`; under `ProvenClientOnly`
  the hole cannot exist and SC5003 makes neither claim. SC7005 follows the
  same premise for the same reason: its whole subject is the SSR shell flush
  committing the response head, and under proven CSR there is no shell, no
  committed head, and nothing to drop. `Unresolved` still reports for both,
  because a server entry in another tsconfig or package would make the defect
  real, and absence of a visible entry is not evidence of absence. The new
  `rendering-csr-selected` fixture pins the third state and carries a positive
  control — SC5003's *async* arm does not depend on the rendering premise and
  still fires — so the fixture cannot pass by containing nothing analyzable;
  dropping its selector turns the two quiet cases into three uncertifiable
  findings. This adds a proof path and its coverage; it lowers no count in the
  baseline above, because no pre-existing fixture selects a rendering mode of
  CSR, and neither `ssr-client-boundary-csr` nor `http-response-flush-csr` may
  gain one — pinning the unresolved state is exactly what they are for.
- **2026-08-21 — primitive domains, tuple arity, and runtime identity close
  three compiler-owned gaps.** Type Facts now exposes an alias-transparent
  primitive value domain, an all-numeric-constituents-are-finite guarantee,
  exact required-only tuple length, and exact runtime identity through the
  existing schema-v1 lifecycle. SC7007 certifies declared safe primitive
  aliases/unions and proves bigint/symbol/undefined-only arguments unsafe;
  broad numbers and object graphs remain uncertifiable. SC7001 proves an
  absent Solid 2 apply slot through an exact one-element spread tuple while
  hidden tuple contents and non-exact tuple shapes remain uncertifiable.
  Structured-return shorthands use compiler runtime identity and exact symbol
  declarations to close tsconfig paths and compiler-selected relative targets;
  external packages (including relative project re-exports) and globals still
  produce SC9012. The producer keeps compact bitsets, preserves the
  retained entity-row budget, and showed no material latency/allocation
  regression in the retained benchmark.
- **2026-08-21 — closed local leaf callback adapters.** `cleanup.rs` now
  follows a callback-producing call only when its exact in-project function has
  one unconditional return of a function literal or the exact callback
  parameter. The returned function is then scanned in its own synchronous
  extent, so the factory and identity-wrapper cases in `leaf-owner` become
  proven SC3001 violations instead of SC9012 obligations. Conditional returns,
  local aliases, package calls, and missing/invalid facts remain fail-closed.
- **2026-08-21 — reviewed contract joins and explicit runtime selection.**
  Exact Type Facts `runtimeIdentity` now joins direct package bindings to
  relative project re-export barrels in one indexed pass; conflicting exact
  summaries stay explicit SC9012/SC9005. The structured-return fixture lost
  its two external-barrel SC9012 obligations while its global remains
  uncertifiable. Native CLI, daemon, ESLint, and coverage metadata now carry
  target/build/rendering/condition/transform selectors in cache identity;
  conditional entrypoints and variants are consumed only when their selected
  evidence is exact. Explicit CSR/SSR selects the rendering premise but does
  not discharge request-dependent SC7005 timing.
- **2026-08-21 — schema-v1 package owner fields.** Additive callback `owner`
  rows and exported-call `ownerRequirements` preserve reviewed owner and leaf
  behavior across a package boundary. The incremental owner index consumes
  both fields; missing rows remain fail-closed and generated contracts put
  owner rows on the review checklist. The callback/owner consumer fixture now
  has one proven SC4001 owner violation and no false owner finding for its
  reviewed leaf callback.
- **2026-08-21 — source-vs-contract differential audit.**
  `scripts/contract-differential.mjs` now analyzes a source implementation,
  generates a contract, and compares the equivalent declaration/runtime
  consumer after an explicit reviewed promotion inside the harness. The
  generated-contract path now carries exact non-conditional owner requirements
  found inside direct exported functions; runtime-conditional and
  request-dependent owner paths remain review obligations. The generator
  checklist calls out both missing callback-owner rows and generated owner
  requirements.
- **2026-08-21 — contract proof boundary and exact conditional summaries.**
  Discovered inferred/generated contracts remain visible as SC9005
  `unverified` status but no longer enter Reactive IR, so an unreviewed claim
  cannot create a violation or suppress an obligation. Conditional generation
  collapses evidence-only and redundant development branches, merges their
  probe modes, retains genuinely disjoint target variants, and refuses
  overlapping semantic differences that schema v1 cannot express without
  negative predicates. Runtime configuration rejects contradictory target,
  build, and rendering selectors. Generated owner requirements now attach by
  canonical compiler symbol (including aliases and anonymous defaults) and by
  the immediate containing function, eliminating name and broad-span matches.
- **2026-08-21 — asserted server arguments use runtime-value facts.** SC7007
  now demands and consumes primitive, constant, and library facts at the peeled
  runtime expression behind transparent TypeScript wrappers. A bigint asserted
  as a safe scalar remains a proven violation; a finite number asserted as an
  unsafe scalar remains silent. The paired strict-`tsc` oracle is clean and the
  fixture adds one violation without increasing the uncertifiable baseline.

### Package-contract parity ledger

The source `ContractExport`/`ContractCallback` surface is now audited against
the consumer boundary. These claims are representable and consumed: reactive
reads, callback timing, callback owner context (including `leaf`), structured
returns, async behavior, exact conditional variants, exact runtime identity,
and direct exported owner requirements (`effect`, `cleanup`, `boundary`, and
`settled-cleanup`). Inferred rows still require review or attestation before
certification.

The following source/runtime behaviors remain explicit fail-closed obligations,
not silent omissions: argument-dependent computed callback maps; component
identity and reactive-prop obligations; reactive-write/action
constraints inside owned or leaf scopes; returned adapters whose callback
behavior appears only when the adapter is invoked; async/reactive-source
settlement through an uncontracted package; and conditional behavior whose
environment has not been selected. Each needs a stable contract field plus a
consumer proof path before it can be reduced. The generator currently refuses
CJS-only targets rather than inventing claims, and review output records
callback/owner gaps. No unreviewed inferred contract is used as certification.

The exact reviewed `@solidjs/signals@2.0.0-rc.0` `isEqual` contract now closes
the v2 oracle's inert-comparison gap. Its other exports remain intentionally
unmodeled until each runtime surface is audited; the v1 equivalent and
arbitrary uncontracted packages remain SC9005/SC9012 obligations.
- **2026-08-20 — cross-rule ownership follows effective enablement.** SC1004,
  SC5001, and Solid 2 SC1007 suppress an overlapping SC1001 only after both
  findings pass rule enablement. Disabling a more specific owner therefore
  restores the strict-read finding instead of silently deleting the whole
  defect. Retired JSX policies remain pinned by explicit negative ownership
  cases (including valid-jsx-nesting and no-implicit-draggable) as well as the
  permanent registry and migration tests.
- **2026-08-20 — control-flow preferences require reactive governing inputs.**
  SC8014 `prefer-for` now reports only when evaluating the rendered `.map`
  receiver performs a proven reactive read; SC8015 `prefer-show` applies the
  same requirement to the `&&` left operand or ternary test. Exact
  accessor/memo calls, store paths, interprocedural and package-contract read
  summaries, and Solid 2 caller-proven prop/accessor-prop reads are supported.
  SC8014 additionally requires an array/tuple Type Fact. Static values,
  once-captured snapshots, unknown calls, non-array members, and reads confined
  to callbacks or branches fail closed. Async callbacks are TypeScript-owned
  and skipped in Solid 1.x; Solid 2.0's published types accept them, so they can
  report but never receive the synchronous rewrite. Neither dialect promotes
  uncertain prop backing into proof for these preferences.
- **2026-08-21 — control-flow preferences use exact dispatch and scoped
  demands.** SC8014 now requires the compiler-selected declaration to be the
  standard-library `map` signature as well as an array/tuple receiver; a local
  or overridden same-name method fails closed. Its safe fix is limited to
  one-parameter arrows because a regular function can observe Array#map's
  three callback arguments through `arguments`. Array-shape Type Facts are
  requested only when `prefer-for` is effectively enabled. With the rule now
  default-on, default native and WASM analysis request them; an explicit native
  rule disable still removes the demand.

## 2026-08 preference defaults

`prefer-for` and `prefer-show` remain style preferences in both catalogs,
alongside `v1/prefer-classlist`, but all five external rule identities are now
enabled by default. Native callers opt out with `enabled: false` in rule
options; ESLint callers set the corresponding generated dialect rule to `off`.
The legacy preset and preference configs remain accepted but redundant. WASM
still lacks a rule-options transport, so it uses the new defaults and cannot
yet opt out.

The analyzer's known approximations, recorded so each is a decision with an
owner rather than a rediscovery. Items live here when a fix is a *design
change* — it would move findings broadly and needs its own fixture-gated
change — as opposed to the bounded corrections that land as ordinary fixes.

Direction legend: **FN** — misses real defects; **FP** — reports correct
code; **Both** — either, depending on the code.

## Rule-catalog reduction release notes

- **2026-08-20 — owner diagnostics merged.** `no-owner-effect`,
  `no-owner-cleanup`, `no-owner-boundary`, and the Solid 2-only
  `no-owner-settled-cleanup` now report as `missing-owner` / `SC4001` (with
  the `v1/` namespace in the Solid 1 catalog). Old rule-options keys are
  aliases to the merged family, so disabling any old member now disables all
  missing-owner variants. Explicit ESLint keys for the former members are a
  breaking removal. The `onSettled` cleanup message retains error severity;
  other proven variants retain warning severity.
- **2026-08-20 — leaf-owner diagnostics merged.**
  `cleanup-in-forbidden-scope`, `primitive-in-leaf-owner`, and
  `flush-in-forbidden-scope` now report as `leaf-owner-forbidden-call` /
  `SC3001`. This is a declared configuration break: old rule-options keys are
  accepted as retired no-ops, their disables do not transfer to the merged
  family, and the former explicit ESLint keys are removed.
- **2026-08-20 — unsuspendable pending reads merged.**
  `pending-async-untracked-read` and `pending-async-forbidden-scope` now report
  as `pending-async-unsuspendable-read` / `SC5001`. This is a declared break:
  old rule-options keys remain accepted as retired no-ops, disables do not
  transfer, and the old explicit ESLint keys are removed. Untracked variants
  retain error severity and leaf-owner variants retain warning severity.
- **2026-08-20 — loading-boundary diagnostics merged.**
  `ssr-client-source-outside-loading-boundary` now reports through the existing
  `async-outside-loading-boundary` / `SC5003` identity. The absorbed key is a
  retired no-op: its disable does not transfer and its explicit ESLint key is
  removed. Proven SSR client-source holes retain error severity; ordinary
  missing-fallback findings retain warning severity.
- **2026-08-20 — package-contract gaps merged.** Missing contracts, missing or
  environment-dependent exports, and unknown callback execution now report as
  `package-contract-incomplete` / `SC9005` (with `v1/` in the 1.x catalog).
  All six old rule-options keys alias the merged family, so disabling one now
  disables every contract-completeness variant. The old explicit ESLint keys
  are removed as a breaking change.
- **2026-08-20 — SC1003 names unified.** Solid 2.0's
  `component-props-destructure` is now `no-destructure`, matching the 1.x stem.
  The old rule-options key aliases the new identity; the explicit ESLint key
  remains temporarily as a deprecated delegate.
- **2026-08-20 — SC1004 names unified.** Solid 2.0's
  `component-returns-conditionally` is now `components-return-once`, matching
  the 1.x stem. The old rule-options key aliases the new identity; the explicit
  ESLint key remains temporarily as a deprecated delegate.
- **2026-08-20 — SC1007 renamed.** `expected-function-got-expression` and
  `v1/expected-function-got-expression` are now `reactive-handler-frozen` and
  `v1/reactive-handler-frozen`. Both old rule-options keys alias their current
  identities; both explicit ESLint keys remain deprecated delegates.
- **2026-08-20 — SC2004 renamed.** `resolve-in-reactive-scope` is now
  `resolve-in-tracked-scope`, naming the precise execution fact the rule
  proves. The old rule-options key aliases the new identity; the explicit
  ESLint key remains temporarily as a deprecated delegate.
- **2026-08-20 — SC7002 renamed.** `sync-node-received-async` is now
  `sync-computation-received-async`, naming the affected computation rather
  than an implementation node. The old rule-options key aliases the new
  identity; the explicit ESLint key remains temporarily as a deprecated delegate.
- **2026-08-20 — three proven rule arms ported to Solid 2.0.** SC8014
  `prefer-for`, SC8015 `prefer-show`, and SC8003
  `jsx-no-duplicate-props`' intrinsic content-competition arm now run in the
  2.0 catalog. The 1.x DOM-slot folding arm stays dialect-restricted, and the
  2.0 list fix uses `<For keyed={false}>` because `Index` was removed.

### Reduction evidence retained with the release

The deletions were driven by runtime/compiler probes and real published
typings, not by catalog-size targets. Representative probe transcripts:

```text
createReaction callback: owner=PRESENT
createReaction callback cleanup: RAN
directive apply: owner=PRESENT
directive effect sees tick=0
directive effect sees tick=1
directive onCleanup RAN
1.9.14 spread onClick -> ATTACHED via delegated $$click
```

Those results removed the three false v1 ownership rules and the false
`warnOnSpread` premise. Runtime source also confirms handler/data arrays are a
supported dispatch form, array seeds are threaded into v1 effect callbacks,
and component props preserve their exact keys. Generic HTML, CSS, injection,
formatting, and naming policies were retired because they are outside the
checker's certification domain.

The real-typings probes also fixed the boundary with TypeScript. Representative
compiler output, in both strict and non-strict passes where applicable:

```text
TS2305: Module '"solid-js/web"' has no exported member 'createEffect'.
TS2540: Cannot assign to 'count' because it is a read-only property.
TS2322: Property 'dangerouslySetInnerHTML' does not exist on the intrinsic props type.
TS17001: JSX elements cannot have multiple attributes with the same name.
```

Accordingly `v1/imports` was removed entirely, readonly store-root writes stay
TypeScript-owned, and SC8003 retains only content-slot or compiler-folding
collisions that the type system does not already report. The executable
`fixtures/tsc-oracle/rule-cases.json` ledger remains the source of exact snippets,
diagnostic codes, and checker expectations.

## The `tsc` redundancy ledger (audited 2026-08-17)

AGENTS.md carries an absolute rule — never report what `tsc` reports, judged
against the library's *real published typings*. This section is that rule
applied to every rule in both catalogs, once, with evidence.

**How the evidence was produced.** `scripts/tsc-oracle.mjs` compiles a snippet
against packages installed from `fixtures/tsc-oracle/packages.json` at the
versions this repository audits — `solid-js@1.9.14` for the 1.x catalog (the
version `pkg/contracts/bundled/solid-v1/solid-js.json` was generated from) and
`solid-js`/`@solidjs/signals`/`@solidjs/web`@`2.0.0-rc.0` for the 2.0 catalog
(the versions in `pkg/contracts/bundled/runtime-lock.json`). It never reads a
fixture stub. Two passes run, `strict` and non-`strict`, because "the project
may not be `strict`" is a distinction a ledger entry has to state even though
the absolute rule refuses it as an exception. TypeScript 5.9.3.

**The product-owned ledger is held to its claims, by span.**
`scripts/ownership-gate.mjs` runs every case in
`fixtures/ownership-cases/cases.json` through the checker and through strict and
non-strict TypeScript over identical bytes. Each expected finding declares one
of `checker-only`, `typescript-owned`, or `distinct-claim`, with exact UTF-8
byte spans and any expected TS codes. Unlisted findings, overlapping claims,
missing fixes, and a TypeScript-owned finding emitted by the checker all fail.

The former 465-case upstream corpus is fully reconciled by
`migration-ledger.json`: 254 cases migrated into the product-owned manifest,
211 dropped with reasons, and zero pending. The old parity and deviation files
were deleted only after `make ownership-gate` enforced that completion.

### Duplicates the span comparison caught, both now narrowed

Found by the span comparison, each suppressed in `PENDING_NARROWING` with a
pointer here rather than left to fail:

**Landed 2026-08-17: `v1/jsx-no-duplicate-props`'s `children`-prop-plus-JSX-children
pair.** TS2710 is *"'children' are specified twice. The attribute named 'children'
will be overwritten."* — word for word the finding's claim, in **both** passes and
on components as well as intrinsic elements. (An earlier draft of this entry said
strict-only; that was a misreading — the strict-only diagnostic in this family is
TS2783, for the attribute-then-spread duplicate.) Only that exact pair is covered:
`innerHTML` with `textContent`, and `innerHTML` with JSX children, draw no
diagnostic at all, so a set including either still reports — the finding then
asserts more than TS2710 does even where TS2710 also fires. Pinned by
`eslint-compat`: the two surviving conflicts report, and the children pair is a
negative on both an intrinsic element and a component.
**Landed 2026-08-17: `v1/no-innerhtml`'s `dangerouslySetInnerHTML` arm** (upstream
cases 09, 10, 11). TS2322 *"Property 'dangerouslySetInnerHTML' does not exist"* and
the finding *"The dangerouslySetInnerHTML prop is not supported; use innerHTML
instead"* are the same claim. Narrowed to components, where props are whatever the
component declares and TypeScript is silent; the `innerHTML` arm is untouched
because `innerHTML` is a declared Solid prop and every claim about it is
independent. Pinned by `upstream-divergences`'s `ReactMarkupProp` — the silent
intrinsic, the reported component with its rewrite fix, and the reported component
whose extra object entry leaves no unambiguous rewrite.

Both are landed, and the ownership gate's confirmed-duplicate list is empty.

**The gate.** `scripts/tsc-oracle-gate.mjs` enforces
`fixtures/tsc-oracle/rule-cases.json` in `scripts/verify.sh` and as
`make tsc-oracle`. A rule whose positive case is also a `tsc` error fails CI, and
a removal justified by a diagnostic fails CI if that diagnostic ever disappears.
Verified in both directions.

It also enforces **completeness**: every rule in either catalog must have a case,
or an `EXEMPT` entry in the gate script saying why no snippet can express its
subject (the package-contract family, whose subject is a third-party artifact;
`execution-map-incomplete`, unreachable from real source by construction; the
server-surface and SSR rules, which need a rendering mode rather than a type; and
`v1/jsx-uses-vars`, which has no diagnostic of its own). That is what turns the
absolute rule from documentation into a mechanism: a new rule cannot be merged
without its positive spelling and the oracle's verdict on it. Verified by
deleting a case and watching the gate name the rule.

**Why this was invisible for a full cycle.** Every fixture stubs Solid with a
reduced `solid-js.d.ts`, and two of those stubs were *looser* than the real
package in exactly the place a rule's proof depended on:

| Stub said | Real package says |
| --- | --- |
| `apply: (value: T) => unknown` | `EffectFunction<Prev, Next extends Prev = Prev> = (v: Next, p?: Prev) => (() => void) \| void` |
| `onSettled(callback: () => unknown)` | `onSettled(callback: () => void \| (() => void))` |
| `createTrackedEffect(callback: () => unknown)` | `createTrackedEffect(compute: () => void \| (() => void), options?)` |
| `refresh(target: unknown)` | `refresh<T>(target: Refreshable<T>)`, where `Refreshable<T> = T & { readonly [$REFRESH]: any }` |
| `affects(target: unknown, key?: PropertyKey)` | `affects(target: Accessor<unknown> \| Store<object>)` / `affects<T extends object>(target: Store<T>, key: keyof T)` |

Each loosening manufactured a defect no real project can produce, and every
gate stayed green while the rule duplicated `tsc`. The proof-bearing
signatures are now byte-faithful in the fixtures that exercise them
(`solid2-precision`, `leaf-owner`, `execution-phases`, `eslint-plugin-corpus`,
`engine/eslint-reactivity-v2`, `package-callback-producer`); where a stub stays
deliberately loose (`static-api`, `static-api-unresolved`, whose subject *is*
the malformed call) the stub now says so in a comment naming the real signature
and asserting that no surviving rule's proof depends on the looseness.

### The general mechanism: TypeScript does not check hyphenated JSX attribute names

Found on 2026-08-17 by the predecessor span audit and now pinned in the
product-owned TypeScript cases; it is the boundary of every "this attribute is
TypeScript's" argument above.

TypeScript exempts a JSX attribute whose **name contains a hyphen** from the
excess-property check entirely — a deliberate allowance for HTML's own hyphenated
custom attributes. Verified against `solid-js@1.9.14`: `data-x`, `my-prop`,
`on-foo`, `html-For`, and the namespaced `class:mt-10` are all accepted on a
`<div>`, while `myProp` is TS2322. The *duplicate-name* check (TS17001) is
syntactic and is **not** exempt, so it still fires on `on-foo` written twice.

Three of the narrowings above were written per element rather than per name and
lost findings to this. All three now ask
`upstream_compat::jsx_name_is_type_checked` before staying silent:

- **SC8012** — `<div class:mt-10={true} />` and its shorthand are upstream's own
  cases 04 and 05. They were declared `status: "policy"` on the grounds that
  TypeScript reports them; it does not. Restored, and the two deviations removed.
- **SC8001** — `<div onFoo-bar="a" />` has an alphabetic third character, so the
  rule looks at it, and the name is never type-checked. Its static-value and
  ambiguous-name arms are restored for any hyphen-bearing name.
- **SC1005** — `<div data-count={count} />` is the one native-attribute value
  position that survives: the accessor is stringified into the attribute and no
  type objects.

All four shapes are pinned in `fixtures/reactive-ir/eslint-compat` and in the
oracle gate's `silent` cases.

### Removed: eight rules, 72 findings

Every one is a **violation the type system already reports on the same code**,
or an **obligation whose whole domain the type system closes**. The first seven
were 2.0-catalog rules; the eighth, `v1/imports`, was a 1.x rule. Their former
upstream cases are permanently reconciled in `migration-ledger.json`.

| Code | Rule | Findings | Why |
| --- | --- | --- | --- |
| SC3004 | `invalid-cleanup-return` | 29 | Every spelling is TS2345/TS2322 against `EffectFunction`'s `(() => void) \| void` return |
| SC9002 | `cleanup-return-unresolved` | 18 | Its whole domain was the *legality* of a returned value, which the same type closes |
| SC7003 | `invalid-refresh-target` | 6 | `Refreshable<T>` is the source brand as a type; every invalid target is TS2345 |
| SC7003 | `invalid-affects-target` | 2 | Same, against `Accessor<unknown> \| Store<object>` |
| SC7004 | `affects-keys-on-accessor` | 2 | A key on an accessor target selects the one-argument overload; the key is TS2345 |
| SC9003 | `refresh-target-unresolved` | 3 | Asked whether the target carries the brand — a question the type answers |
| SC9003 | `affects-target-unresolved` | 3 | Same |
| SC8002 | `v1/imports` | 9 | Its one condition — the named module does not export the name — is exactly TS2305's; audited later, see below |

#### SC3004 `invalid-cleanup-return`

`tsc --noEmit`, real `2.0.0-rc.0` typings, **both** passes (so no `strict`
argument is available):

~~~
sc3004.tsx(5,29) TS2345: Argument of type '(value: number) => number' is not assignable to
  parameter of type 'EffectFunction<number, number> | EffectBundle<number, number>'.
    Type 'number' is not assignable to type 'void | (() => void)'.
sc3004.tsx(6,29) TS2345: ... (explicit `return value + 1`)
sc3004.tsx(7,29) TS2345: ... (`() => makeCount()`, a returned call)
sc3004.tsx(8,29) TS2322: ... (`() => teardown.count`, a member return)
~~~

The legal spellings — `() => teardown.dispose`, `() => () => {}`,
`() => undefined`, `() => {}` — are **accepted**. The type does not merely
reject more than the rule did; it draws the same line. `(() => void) | void` is
a union, not bare `void`, so return-value-ignoring assignability does not apply.

#### SC9002 `cleanup-return-unresolved`

The obligation had four sources. Three are TypeScript's, and the fourth is not
a defect:

- **mixed union / `unknown`** — TS2345 and TS2322 in both passes.
- **a non-callback second argument** (`undefined`, `null`, `5`, `"apply"`, and
  1.x-style value threading) — TS2345 in both passes.
- **an unconstrained generic return** — TS2345.
- **`any`, and an unresolved wrapper callback** — `tsc` is silent, and that is
  not licence to report. Absence of a type error because the type is `any` is
  *missing evidence*, not proof (AGENTS.md's own trap list). More decisively:
  when the program type-checks, TypeScript has *proven* the returned value
  legal, so an obligation asserting uncertainty about its legality is noise
  about code the type system has cleared. The ownership consumer needs no
  finding for this — it simply does not get a "cleanup was handed over" fact,
  which is correctly modeled as an absent proof.

One of those unresolved-callback obligations was worse than noise: three of the
18 sat on `createEffect(compute, { effect, error })`, which is the **supported
`EffectBundle` form**. `tsc` accepts it; the checker raised an obligation on
idiomatic code. `rule_quality_process.rs` pins all of these at 0 so a
reintroduction fails there.

#### SC7003 / SC7004 / SC9003 — the refresh and affects target family

This family was hidden by the same mechanism, one layer deeper: the fixtures
type `refresh(target: unknown)`, while `@solidjs/signals` brands its
refreshable sources in the type system —

~~~ts
export type Refreshable<T> = T & { readonly [$REFRESH]: any };
export declare function refresh<T>(target: Refreshable<T>): void;
export declare function affects(target: Accessor<unknown> | Store<object>): void;
export declare function affects<T extends object>(target: Store<T>, key: keyof T): void;
~~~

so *every* shape the rules proved invalid is a type error, in both passes:

~~~
p3.tsx(11,11) TS2345: '() => number' is not assignable to 'Refreshable<() => number>'.
                        Type '() => number' is not assignable to '{ readonly [$REFRESH]: any; }'.
p3.tsx(12,11) TS2345: '{}' ... Property '[$REFRESH]' is missing
p3.tsx(13,11) TS2345: a plain accessor `target` ... '[$REFRESH]' is missing
p3.tsx(14,11) TS2345: `state.user`, a store child record ... '[$REFRESH]' is missing
p3.tsx(15,11) TS2345: `affects(signalGet())` — 'number' is not assignable to
                        'object | Accessor<unknown>'
p4.tsx(14,11) TS2345: `refresh(valueFormStore)` — only the derived forms are branded
p4.tsx(6,17)  TS2345: `affects(memo, "name")` — '"name"' is not assignable to 'unique symbol'
~~~

And the valid targets — `refresh(memo)`, `refresh(signalGet)`, `affects(memo)`,
`affects(state)`, `affects(state, "user")`, `affects(state.user, "name")` — all
type-check. Same line, both directions. Zero-argument and over-long calls are
TS2554 by arity.

**What was kept.** `static_api.rs` still records the `refresh(...)` *write* that
SC2001 `reactive-write-in-owned-scope` consumes; only the target diagnostics
went, and the control flow that skips a malformed call still skips it, so a call
`tsc` rejects records no write. SC2001 is unchanged at 36 findings.

**Do not amputate the runtime half.** The same applies to cleanup: SC3004's
consumer is gone, but `cleanup.rs::function_returns_cleanup` and the
`CleanupReturnStatus` classification behind it are load-bearing for SC4002 and
SC4004, which assert *ownership and disposal* — facts no type expresses. The
`callResultDomain` and member-return work still serves them; `diagnostics_process.rs`
pins that a returned call producing a `number` hands over no cleanup while one
producing a function does.

### Correction 2026-08-17: three rules were mis-classified as fully redundant

An earlier pass of this ledger listed `v1/event-handlers`, `v1/no-react-specific-props`,
and `v1/style-prop` as **proven redundant, removal specified**. That was wrong,
and the mistake is worth recording because it is the mirror image of the
fixture-stub trap: each was probed on *one* arm — an unknown attribute name on
an intrinsic element — and the verdict was generalised to the whole rule. Read
against each rule's complete former upstream domain (now reconciled in
`fixtures/ownership-cases/migration-ledger.json`),
all three have an arm TypeScript does not cover, so all three are **partially
redundant** and belong in the table below. None was deleted.

What the full probe found (real `1.9.14` typings, both passes):

| Spelling | `tsc` | Whose claim |
| --- | --- | --- |
| `<div onclick={fn} />` | silent — `onclick` *is* a declared prop | SC8001's canonical-casing advice |
| `<div ondblclick={fn} />`, `<div onDblClick={fn} />` | silent — both declared | SC8001's ambiguous-name advice |
| `<div {...{ onClick: fn }} />` | silent | SC8001's `warnOnSpread` arm |
| `<div onClIcK={fn} />`, `<div oncLICK={fn} />`, `<div onDoubleClick={fn} />`, `<div ondoubleclick={fn} />`, `<div only={fn} />`, `<div onLy="s" />` | TS2322 "Property does not exist" | TypeScript's |
| `<Pascal className="x" />`, `<Pascal htmlFor="x" />`, `<Pascal key={1} />` with permissive props | silent | SC8011 — and upstream's cases 4, 8, 9 are exactly these |
| `<Strict className="x" />` where the component declares `{ class?: string }` | TS2322 | TypeScript's |
| `<div className="x" />`, `<div htmlFor="x" />`, `<div key={1} />` | TS2322 | TypeScript's |
| `<div style="font-size: 10px; missing-value: ;" />` and every other string-valued `style` | silent — string styles are legal in 1.x | SC8017's string arms, including the malformed-CSS claim |
| `<div style={{ "-webkitAlignContent": "center" }} />` | silent — the `` [key: `-${string}`] `` index signature absorbs it | SC8017 |
| `<div style={{ fontSize: 10 }} />`, `{{ COLOR: "x" }}`, `{{ unknownStyleProp: "x" }}` | TS2561/TS2353 | TypeScript's |
| `<div style={{ "margin-top": -10 }} />` | TS2322 against `MarginTop<…>` | TypeScript's |
| `<div css={{ … }} />` (a configured extra style prop) | TS2322 | TypeScript's |

The narrowing each needs is the same question — *is this attribute name declared
on this element's attribute type* — which is a type fact the checker does not
have. The implementable approximation, per rule, is in the table below.

### Narrowed 2026-08-17: five rules, partially redundant, now scoped

Each keeps the arm no type answers and drops the arm TypeScript already
reports. Every one is pinned in **both** directions by
`fixtures/tsc-oracle/rule-cases.json` — a `removed-because-redundant` case for
the dropped arm and a `silent` case for the surviving one — so neither half can
move without failing CI. Each retained spelling is now a direct positive or
negative case in `fixtures/ownership-cases/cases.json`.

| Code | Rule | Dropped, and why | Kept, and why |
| --- | --- | --- | --- |
| SC8011 | `v1/no-react-specific-props` | `className`/`htmlFor`/`key` on an **intrinsic** element — TS2322 each. The `key` arm was intrinsic-only and went entirely. | The same spellings on a **component**, upstream's own cases 04 and 08. A component's props are whatever it declares, so the key is permitted on a permissive one and a type error on `{ class?: string }` — the answer genuinely depends on the component. |
| SC8017 | `v1/style-prop` | The object-key arms on an intrinsic element: camelCase (TS2561 with the kebab suggestion), an unknown key (TS2353), a unitless number for a length (TS2322 against `MarginTop<…>`), and a configured extra style prop (TS2322 on the attribute). | Every **string-valued** `style`, legal in 1.x, including the two claims no type can make — a declaration with a missing value, and a value that is not CSS. Plus any `-`-prefixed key on any element: `` [key: `-${string}`] `` absorbs it, so `-webkitAlignContent` is silent (upstream's case 02). Plus any key on a component. |
| SC8001 | `v1/event-handlers` | Every **type-checked unknown** `on*` name in every value form including the boolean shorthand (TS2322), and every mis-cased or non-standard spelling — `onClIcK`, `oncLICK`, `onDoubleClick`, `ondoubleclick` are not declared under any casing. Also the static-value arm on a standard declared handler: no static value is assignable to `EventHandlerUnion`. | The readability rename for a **declared** spelling: 1.x declares each handler as both `onClick` and `onclick`, so `onclick` and `ondblclick` type-check (upstream's cases 02 and 12). Every arm on a **hyphenated tag**: `<my-widget />` is TS2339 against stock typings, so a project using one declared it itself, commonly permissively. Hyphenated attribute names such as `on-foo`, which TypeScript deliberately skips but the compiler still lowers. And `warnOnSpread`, which type-checks while Solid does not attach the handler. |
| SC8003 | `v1/jsx-no-duplicate-props` | **Identically spelled** duplicates, by origin pair: two attributes are TS17001, an attribute then a spread is TS2783 (`strict` pass only, which the rule does not accept as an exception), two keys in one spread object are TS1117. | Two **differently spelled** props the compiler folds into one slot — `onClick`/`onclick` both become the delegated `el.$$click` write, `attr:title`/`title` share the template attribute slot. Plus the two identical-name orders TypeScript leaves alone: a spread then an attribute (upstream's case 02) and two different spread objects. Plus every child-content conflict — no type relates `children`, JSX children, `innerHTML`, and `textContent`. |
| SC8012 | `v1/no-unknown-namespaces` | Every namespaced prop on an **intrinsic** element — TS2322. Solid resolves namespaces through mapped types over user-augmentable interfaces plus individually declared `on:*` events, so an unrecognised prefix has nothing to land on. This covered the `style:`/`class:` steer too: **neither prefix is declared at all**, a real gap in Solid's published typings given the 1.x compiler supports both. | The same on a **component**, upstream's cases 06 and 07. Props are a plain object, TypeScript is silent, and the claim — the compiler special-cases namespaces only on DOM elements it lowers directly, so the prop arrives inert — is one no type makes. |
| SC1007 | `expected-function-got-expression` | The **call-result** arm on a normal declared handler. Both its triggers land on TS2322 at the same attribute: an expression *proven non-callable* is exactly what TypeScript rejects, and a *proven-accessor call* is rejected whenever the accessor's value is not callable (`onClick={count()}` with `count: Accessor<number>`). Deliberately **not** kept for the one spelling TypeScript permits — an accessor holding a function, `onClick={handler()}` — because there the finding would be wrong: a JSX attribute expression is a tracked read, so that handler does update. | The **reactive-handler-read** arm: a callable handler read out of reactive props or store state. TypeScript is silent, and the claim is a timing one — a native listener receives its function value once during DOM setup, so reading it through reactive props freezes the initial handler. Also the hyphenated native `on*` arm TypeScript deliberately declines to check: proven non-callable/non-array values are violations and mixed runtime shapes are uncertifiable. |
| SC1005 | `uncalled-accessor` (both catalogs) | Three of its six value positions, in both dialects: a native JSX attribute (TS2322 — an accessor is never assignable to a DOM attribute type), a class object value (TS2322 against 2.0's `Record<string, boolean>`), a computed property access (TS2538). This removed the last consumers of the dialect's `class_object_values_are_truthiness_coerced` and `native_children_attribute_invokes_functions` predicates, which went with them. | The positions TypeScript **permits**, and the most common real spellings of the bug: a string-concatenation binary operand (`"hello " + label` renders the accessor's source text), a unary operand (logical-not and the numeric coercions `-`/`+`/`~`, all clean against the published typings), and a template-literal interpolation. |

### Narrowed 2026-08-17: `no-direct-mutation`, in the 2.0 catalog only

2.0's `createStore` returns a shallowly `Readonly` proxy, so a write to a
**root** record property is already a type error against
`@solidjs/signals@2.0.0-rc.0`, for both spellings:

~~~
mut.tsx(4,29) TS2540: Cannot assign to 'count' because it is a read-only property.  // state.count = 1
mut.tsx(5,29) TS2540: Cannot assign to 'count' because it is a read-only property.  // state.count++
~~~

**Solid 1.x is the opposite**, and this is why the predicate is asked of the
dialect rather than assumed: its `createStore` returns a *mutable* store type,
and the same four writes produce **no diagnostic at all** against
`solid-js@1.9.14`. The 1.x rule is fully independent and untouched — carrying the
2.0 answer across the seam would have silenced it wrongly, which is exactly the
failure AGENTS.md warns about.

Three shapes survive in 2.0, each a write TypeScript accepts and the runtime
drops:

- **A nested record's property.** The readonly-ness stops at the top level, so
  `profile.user.name = "Grace"` type-checks.
- **A cast.** `(profile as { count: number }).count = 1` erases the readonly.
  This one constrains the implementation: `member_root` resolves *through* the
  cast, so comparing the written member's object against the resolved root span
  alone would have handed this case to a diagnostic that does not exist. The
  narrowing therefore requires the object to be a bare **identifier**.
- **A props member**, which is not readonly at all.

Coverage 526 → 524; the three root writes went and two of the surviving shapes
were added as fixture cases, because the fixture had none. The finding *count*
happens to land on four either way, so `diagnostics_process.rs` asserts each
surviving spelling and each dropped one **by span** — a count alone cannot tell
this narrowing from a regression that dropped the wrong three.

### Closed 2026-08-18: SC1005 no longer overlaps arithmetic diagnostics

The structural fact now retains binary `+` expressions with a string literal on
one side, unary logical-not, and the unary numeric coercions. Numeric and
bitwise **binary** operators reject a function operand in TypeScript — `count +
1` is TS2365, `count - 1`, `count * 2`, and `count | 0` are TS2362 — and are no
longer SC1005 positions, which removes the former `count + 1` duplicate.
Concatenations whose string behavior would require a resolved operator
signature remain outside the violation claim rather than being guessed from
source text.

The **unary** operators were dropped in the same pass on the assumption that
they behave like their binary counterparts. They do not: probed against
solid-js@1.9.14 through `scripts/tsc-oracle.mjs`, `-f`, `+f`, and `~f` on a
function value are clean in *both* the strict and loose passes, exactly like
`!f`. Dropping them removed a real, TypeScript-silent defect class (a coerced
accessor is silently `NaN`) and lost upstream parity case
`reactivity__invalid__21`, whose deviation could not be declared
TypeScript-owned because the span audit proved TypeScript reports nothing
there. They are restored under
`CoerciveOperandKind::NumericCoercion`, pinned by a `expect: "silent"` /
`checker: "reports"` oracle case and by
`fixtures/reactive-ir/uncalled-accessor-v2`, whose `TypeScriptOwnedOperators`
case now holds only the binary spellings.

### Independent — keep

Grouped by why no type can express the claim. `tsc` was confirmed silent on a
positive case for each entry marked ✓; the rest assert a runtime, timing, or
provenance fact with no type surface at all.

- **Reactivity and timing** — SC1001 `strict-read-untracked`, SC1002
  `reactive-read-after-await` ✓, SC1006 `untracked-derived-function`, SC9011
  `reactive-source-uncaptured`, SC5004 `v1/no-async-tracked-scope` ✓
  (`createMemo(async …)` type-checks: 1.x `EffectFunction` returns `Next`
  freely and 2.0's `ComputeFunction` admits `PromiseLike`), SC5001/SC5002/SC5003/SC5005
  (the pending-async and Loading-boundary family). *When* a read happens
  relative to a tracking scope is not a property of its type.
- **Ownership and disposal** — SC4001 `no-owner-effect`, SC4002
  `no-owner-cleanup` ✓ (`onCleanup(() => {})` is a well-typed `Disposable`),
  SC4003 `no-owner-boundary`, SC4004 `no-owner-settled-cleanup` ✓ (returning a
  real cleanup from an unowned `onSettled` is perfectly typed; the claim is that
  nothing will dispose it), SC3001/SC3002/SC3003 (the leaf-scope rules).
- **Write phase and transactions** — SC2001 `reactive-write-in-owned-scope`,
  SC2002 `action-called-in-owned-scope`, SC2003 `no-direct-mutation`, SC2004
  `resolve-in-reactive-scope`. Which scope is active at a call is not typed.
- **Compiler lowering** — SC8008 `v1/no-innerhtml` ✓ (`innerHTML` is a declared
  prop and a string is its declared type), SC8004 `v1/jsx-no-script-url` ✓
  (`href` is `string`; the claim is about the scheme the string carries), and
  SC8007 `v1/no-array-handlers` ✓ — the case that
  proves the JSX family is not uniformly redundant: `EventHandlerUnion` includes
  `BoundEventHandler`, so `onClick={[handler, 1]}` is **legal** per Solid's own
  types.
- **API shape that survives its own signature** — SC7001
  `missing-effect-function` ✓: the single-argument `createEffect(compute)`
  overload still exists in rc.0, deprecated and typed `never`, so the call
  type-checks and the claim "this effect never runs" is the checker's alone.
  Cast-hidden non-callable values survive in both dialects as well, including
  `.ts` angle-bracket assertions and a cast-hidden non-callable `effect` field
  in the required `{ effect, error }` bundle. Raw invalid arguments, including
  nullish values accepted only with `strictNullChecks` disabled, are excluded
  because the strict published-type pass reports them.
  Missing facts now remain explicit rather than becoming silent gaps. Exact
  required-only tuple length can prove that a spread-expanded Solid 2 call has
  no apply slot; the one-element case is therefore a violation and is proven
  not to allocate an owner-bound computation. A tuple slot hidden inside a
  spread still has no value fact, while optional/rest/array/unequal-union
  shapes have no exact length, so those paths remain uncertifiable. Unknown or
  `any` callback values are also uncertifiable, as is a nullable callback hidden by the
  runtime-transparent `!` wrapper, while compiler-proven callable identifiers retain a proven
  violation path. A `"use server"`
  directive is a framework and bundler convention that no core package reads;
  both published server entries neutralise client-runtime claims (1.9.14 uses a
  no-op; 2.0.0-rc.0 routes through `serverEffect`). Otherwise-reporting effect
  and ownership cases under the directive are therefore uncertifiable until a
  project/compiler fact proves which entry executes. Oracle cases pin both the
  uncertain directive path and undirected controls.
  SC7002 `sync-node-received-async`, SC7005 `http-response-after-flush`,
  SC7006/SC7007 (the server surface) likewise assert runtime behavior. SC5005
  distinguishes a visible server-render entry from an absent one. SC7005 is
  now always uncertifiable: even with SSR proven, source facts cannot decide
  whether a boundary settles before or after the shell flush. This
  2026-08-20 kind change means SC7005 now fails `--certify`.
- **Syntax and style, no type surface** — SC1003 `v1/no-destructure` ✓ /
  `component-props-destructure`, SC1004 `v1/components-return-once` /
  `component-returns-conditionally`, SC8002 `v1/imports`, SC8006
  `v1/jsx-uses-vars`, SC8009 `v1/no-proxy-apis` ✓ (a legal import; the claim is
  target-runtime Proxy support; explicit type imports are proven erased and
  runtime-referenced imports are proven executing, while unused value imports
  are uncertifiable because `verbatimModuleSyntax` changes their emit; direct
  Proxy calls require the exact standard-library declaration; and `mergeProps`
  reports a violation only for a proven function source, certifies only exact
  plain literals, and keeps every possible callable/`$PROXY` source
  uncertifiable without identifier-name heuristics), SC8010
  `v1/no-react-deps` ✓
  (`createEffect(fn, [dep])` type-checks — the array is 1.x's `Init` value),
  SC8013 `v1/prefer-classlist`, SC8014 `v1/prefer-for`, SC8015
  `v1/prefer-show`, SC8016 `v1/self-closing-comp` ✓, SC8018
  `prefer-component-syntax`, SC6001 `primitive-in-directive-application`.
- **Provenance and contracts** — SC9001, SC9005, SC9006 (the package-contract
  family). A missing contract is a statement about analyzability, not about a
  type.
- **SC8005 `v1/jsx-no-undef`** — kept, with a caveat worth recording. Its
  surviving domain is an unknown `use:` name (unresolved JSX tags are
  TypeScript-owned and remain checker-silent). Against the published typings *alone*,
  `use:autofocus` is TS2322, because `JSX.Directives` ships empty and is meant
  to be augmented. In a project that has augmented it — the documented, intended
  usage — `tsc` is silent, and the checker's claim (no lexical *value* binding
  exists for that name) is a different question from whether the *type* was
  declared. Independent, but a narrowing candidate if the two ever collapse.

### Fixed 2026-08-17: `solid-1x-sources` had been running the 2.0 dialect

The documented `.gitignore` trap, live in the repository.
`fixtures/reactive-ir/solid-1x-sources/node_modules/solid-js/` existed as an
**empty directory** — no `package.json`, no `.gitignore` exception, nothing
tracked — so dialect selection found no 1.x version and fell back to the 2.0
default. The fixture whose entire stated purpose is "the reactive-source
factories 1.x has and 2.0 does not" had never exercised the 1.x catalog.

What it was actually asserting: six `package-contract-export-missing`
obligations, because `createComputed`, `createDeferred`, `createSelector`, and
`createResource` are not in the 2.0 contract; plus a spurious
`missing-effect-function` and `no-owner-effect` on 1.x's single-argument
`createEffect`; plus one of the 18 SC9002 obligations the cleanup-return removal
dropped, which was this artifact rather than a real obligation.

What it asserts now: thirteen findings, every one a 1.x source factory's
untracked read — `createSignal`, `createMemo`, `createResource`,
`createDeferred`, `createSelector`, `createMutable`, `For`, `Index` — exactly the
"evidence that the source was discovered at all" its comment claims, plus
`v1/no-proxy-apis` on the store import, `v1/no-async-tracked-scope`, and
`v1/reactive-read-after-await`.

The stub and its `.gitignore` exception lines are now tracked together, which is
the only form of this fix that survives CI.

### Withdrawn: `import-location` is not a fixture defect

An earlier pass of this ledger recorded
`fixtures/reactive-ir/import-location`'s `import { createSignal, createMemo }
from "solid-js/store"` as a defect, on the grounds that it is TS2305 and no real
project compiles it. That reading was wrong: importing a name from the wrong
module **is** `v1/imports`'s subject, so the case is deliberate and correct.

It did raise a live question, and auditing it removed an eighth rule.
`v1/imports` (SC8002) fired on exactly one condition — the module named in the
import does not export the name — which is exactly TS2305's condition:

~~~
imp.tsx(1,10) TS2305: Module '"solid-js/web"' has no exported member 'createEffect'.
imp.tsx(2,10) TS2305: Module '"solid-js"' has no exported member 'render'.
imp.tsx(3,15) TS2305: Module '"solid-js/store"' has no exported member 'Component'.
~~~

Both passes, and value and type positions alike. The second arm I assumed it had
does not exist: a name exported by *both* modules returns early, so the style
preference upstream expresses for `import { Show } from "solid-js/web"` was never
reported here — verified silent, and pinned as such. Its module-rewrite autofix
was genuinely useful, and offering an autofix is explicitly not an exception.
**Removed**, 9 findings; `Dialect::export_modules` and the generated per-subpath
export index remain, still consumed by the contract layer.

## Compiler-faithful heuristics (verified against the 1.x compiler, do not "fix")

These were flagged as suspect eslint-plugin-solid ports and have now been
verified against the **pinned 1.x compiler**
(`solid-1x-compiler@79b9b637`, byte-faithful to
`babel-plugin-jsx-dom-expressions@0.40.7`) — the parity target is Solid's own
behavior, not upstream's quirks. Each entry below matches the compiler, which
is why it stays.

- **`on*` event-name detection** (`upstream_compat/shared_reactivity.rs`,
  `solid1x_attributes.rs`): the compiler's attribute lowering treats *every*
  `on`-prefixed DOM prop as an event (`plan.key.starts_with("on")`,
  `to_event_name` = the suffix lowercased), so `once`/`only` genuinely become
  listeners for events `ce`/`ly` when function-valued, and statically-valued
  ones are frozen into the template as plain attributes — exactly what
  `v1/event-handlers` reports. Upstream's `/^on[a-zA-Z]/` is *narrower* than
  the compiler; the checker deliberately uses the compiler's `startsWith`
  boundary instead, so a statically frozen `on-foo` receives SC8001 while a
  dynamic non-callable value is handled by SC1007's TypeScript-unchecked
  handler branch. A callable `on-foo` remains clean because it is a real
  listener for the distinct `-foo` event.
- **ASCII-only element-name case classification**
  (`upstream_compat/mod.rs::is_lowercase_led`): Babel's `isCompatTag` is
  `/^[a-z]/`, so a non-ASCII-led tag compiles as a component reference. The
  checker matches the compiler.
- **Static `innerHTML` without children is silent**
  (`no-innerhtml`, `allowStatic` default) and **single-line
  whitespace-only children block `self-closing-comp`** — configurable
  stylistic leniencies matching upstream's option defaults; neither can
  produce a false positive.

## Resolved: Solid 2.0 precision corrections 2026-08-17

**Read with the ledger above.** Where an entry below describes SC3004 or SC9002
as reporting something, that consumer is gone; the *classification* work it
describes survives because SC4002/SC4004 need it. The entries are kept as
written rather than rewritten, because they record how the runtime value domain
came to be trusted — which is still true — and rewriting them would erase the
reason the removal was safe.

- **SC7007 inline arguments and serializer identity** (`server_rules.rs`,
  `demand_plan.rs`): compiler library-type facts are now demanded for every
  non-spread server-function argument, so `save(new Date())` no longer falls
  through a variable-only gap. `configureServerFunctionsClient` must resolve
  to the exact `@solidjs/web/server-functions` declaration; a local same-named
  function with a `serializeArgs` property cannot silence the project. A valid
  exact configuration call with a dynamic options object produces an
  uncertifiable SC7007 until `serializeArgs` presence is closed. The remaining
  top-level fact boundary no longer creates silent nested false negatives.
  Alias-transparent primitive-domain facts now certify strings, booleans,
  null, and unions whose numeric constituents are all finite literals; domains
  containing only bigint, symbol, or undefined are proven violations. Broad
  numbers, mixed safe/unsafe primitive unions, object graphs, arrays, spreads,
  and missing facts remain uncertifiable. Invalid calls remain TypeScript-owned
  through an exact call-validity gate.

- **Synchronous standard callbacks after `await`** (`static_rules.rs` and
  `runtime_semantics.rs`): SC1002's accessor-call *and* member-read proofs now
  continue into a function written directly in an exact built-in
  `Array`/`ReadonlyArray.prototype.filter` call after a dominating await.
  Callability is sampled at the argument, not the callee, and the callback must
  be the literal argument for a proven SC1002 —
  `filter(makePredicate(fn))` and an `async` callback instead produce SC9012,
  preserving the unknown synchronous callback extent as an explicit proof
  obligation. Promise callbacks and project-defined or shadowed methods are
  outside this exact built-in behavior; unresolved package callbacks remain
  package-contract obligations.
- **Cleanup returns classified from the runtime value domain** (`cleanup.rs` and
  `demand_plan.rs`): identifier returns are demanded with TypeFacts'
  `runtimeValueDomain` and classified from it rather than from rendered type
  text, at exactly the peeled span the classifier resolves the entity at (so
  `return (value)` and `return value as Cleanup` work like the bare form).
  `CleanupReturnStatus` now separates "proven a function" from "proven legal but
  not a function", so a proven-`undefined` return can no longer make a callback
  look like one that returns a cleanup. Mixed legal domains, `unknown`, `any`,
  and generics are not legality findings (the removed SC9002 was TypeScript's
  job), but an unowned `onSettled` callback that may return a cleanup now keeps
  an uncertifiable SC4004 owner obligation instead of treating that cleanup as
  absent.
- **Static member cleanup returns** (`cleanup.rs` and `demand_plan.rs`): member
  return spans now receive the same exact `runtimeValueDomain` demand as
  identifier returns. A proven static function member is accepted as a
  cleanup and a proven primitive member was SC3004 (removed; now simply "not a
  cleanup"). A *mixed* union
  (`(() => void) | number`), `any`, and a computed member were SC9002 (removed),
  because their runtime property value is not closed by an exact dispatch
  proof. An **optional** member (`maybe?: () => void`) is legal but does not
  prove a cleanup on every execution; when owner safety depends on it, SC4004
  is uncertifiable. Verified against the pinned producer for all four
  spellings.
- **`runWithOwner` Owner identity** (`owners.rs` and `solid-dialect`): the
  supplied-owner proof now accepts only a compiler-resolved `Owner` type whose
  declaration and origin match the active dialect export table. Re-exported
  aliases are accepted; a user-local `Owner` lookalike and unresolved values
  remain conditional. This removes the rendered-type-name match without
  changing the nullable-owner fail-closed behavior.
- **Assignment target reads** (`solid-facts/src/ast` and
  `AstFacts::is_plain_assignment_target`): normalized facts distinguish plain
  assignment from compound/update reads, and only the member that *is* the
  written target is exempted, so a computed key or destructuring default inside
  a target stays an SC1001/SC1002 read.
- **Owner-backed settled cleanup** (`owners.rs`): the owner requirement pass now
  gates only the duplicate SC4002 for an inline owner-backed `onSettled` callback,
  and only when the callback is the literal argument; SC3001, genuinely unowned
  SC4002, and unowned returned-cleanup SC4004 remain distinct. Indirect,
  exported, and unresolved cases stay conservative.
- **The leaf pass requires an exact callback value and its synchronous extent**
  (`cleanup.rs`). The leaf-scope rules (SC3001/SC3002/SC3003) used to
  fire for a primitive written lexically *anywhere* inside the leaf-owner
  argument, so `onSettled(wrap(() => { onCleanup(fn) }))` reported SC3001 even
  though `wrap` may stash the callback and run it out-of-band, where no leaf
  scope exists and the call does not throw. The pass now demands the same two
  containment facts the dynamic-extent path already did: a literal or exact
  in-project callback exposes its body, and the call must sit in that
  callback's own synchronous extent (`direct_callback_contains`). An exact
  identifier callback and a closed local adapter with one unconditional
  function-literal or callback-parameter return are followed transitively, so
  forbidden operations keep their SC3xxx identity and an exact safe body is
  certified. Conditional, aliased, package, and otherwise opaque callbacks
  cannot support a specific violation claim and produce SC9012
  `uncertifiable` instead of silent failure. Known accessors, setters, actions,
  primitives, and exact standard-library calls discharge that walk, preventing
  false uncertainty on ordinary signal operations. The genuinely unowned
  SC4002 and the unowned returned-cleanup SC4004 are unaffected, as are the
  settled call-site gates. Pinned by `fixtures/reactive-ir/solid2-precision`
  and the closed/opaque pairs
  in `fixtures/reactive-ir/leaf-owner/`.

### Remaining approximations from this slice

- **Resolved 2026-08-17, returned calls are classified from the call result**
  (`cleanup.rs::returned_call_domain`, `demand_plan.rs`). The TypeFacts
  interface change this needed has landed: `callResultDomain`
  (`solid-ts-facts` `559c9031`, ADR 0013) matches a call-like node against the
  demand's exact start *and end* bytes and classifies the checker type there
  with the same runtime value-domain classifier, so the callee a call shares a
  start byte with can never be the subject. `cleanup_return_status` now feeds
  that domain to the existing `domain_cleanup_return_status`, which closes both
  directions the old callee probe produced: `return makeCount()` where
  `makeCount(): number` is SC3004 rather than silent (**FN** closed), and the
  unowned `onSettled(() => { return makeCount(); })` no longer reports SC4004
  as though a cleanup were handed over (**FP** closed). `handlers[i]()` is
  classified from its own signature rather than from a fact about `handlers`,
  which was the hazard that kept the value domain off call spans before.
  Both return spellings are covered: an expression-bodied arrow records its
  return on the function fact, so `returned_callees` now chains
  `functions[].expression_return` and `() => make()` is demanded exactly like
  `return make()`. Absent (no exact call-like node) and `unknown` (checker
  error or recovery type) remain fail-closed, as does a callee whose
  `resolvedCall` is not `Valid`. Pinned by
  `fixtures/reactive-ir/solid2-precision`'s `ReturnedCallCleanupReturns` and
  the two module-level `onSettled` returned-call cases.
  Across the corpus this discharged 30 SC9002 obligations and proved 13 SC3004
  returns — all of them a call producing a primitive where a cleanup is
  expected, such as `createEffect(() => 1, () => read())` and
  `createEffect(() => count(), () => untrack(() => count()))`.
  `callability` is no longer demanded at returned-call spans. Cleanup was its
  only consumer, and demanding it there is not merely dead: callability is read
  through `smallest_contained_callability`, which selects the smallest entity
  *contained* in the queried span, so a callability fact on an expression-bodied
  arrow's own returned call (`(post) => post.includes(id())`) sits inside the
  callback-argument span and outranks the arrow. That answered "is
  `post.includes(id())` callable" (no) where `inline_standard_callback` asked
  "is this argument a callable callback", which silently withdrew the
  `Array#filter` synchronous proof and with it SC1002 on the accessor read —
  visible only when the callback body *is* the returned call, since a binary
  body has no returned callee. The result domain is invisible to that lookup.
- **Evidence-backed divergence from upstream, `no_direct_mutation`**: with
  compound-assignment and update facts, the shared port now reports
  `store.count++` on a props/store member, which eslint-plugin-solid 0.14.5
  (commit `6d3bc311`) misses — its props branch tests for an ESTree
  `AssignmentExpression`, and `++` is an `UpdateExpression`. The compound form
  and an accessor binding's `++` are both parity-correct (upstream reports them
  via `AssignmentExpression` and `reference.isWrite()` respectively). No upstream
  case exercised any of these spellings; it is pinned by
  `fixtures/reactive-ir/v1-reactivity`'s `MutatesInPlace`.

## Resolved: false negatives closed 2026-08-16

- **Leaf-owner rules follow the dynamic extent through exact helpers**
  (`cleanup.rs::helper_forbidden_operations`). `onCleanup`/`flush`/primitive
  creation in a project function's *synchronous extent* (body minus nested
  function bodies) throws when the function is called from a leaf scope; the
  call site in the leaf callback is flagged, naming the helper
  (`LeafOwnerOperation::via`). Resolution is the exact TypeScript identity,
  transitive with a cycle guard. Remaining boundaries, deliberate: an
  unresolved/ambiguous/package callee contributes nothing (package behavior
  is the contract surface's), IIFEs inside a helper count as nested bodies
  (silent), and helper calls written inside nested functions within the leaf
  callback are not the leaf's synchronous extent (silent, correct).
  The leaf callback must be a **function literal, exact in-project function
  reference, or closed local callback return**. The last form is followed only
  when one exact local function unconditionally returns a function literal or
  its callback parameter; this proves the value received by the owner rather
  than treating the factory call itself as the callback. Conditional returns,
  local aliases, package calls, and opaque wrappers remain SC9012 obligations.
  `fixtures/reactive-ir/leaf-owner/` pins the `onCleanup`, `flush`, and
  primitive positives, the transitive hop, exact safe and defective
  references, both literal spellings, the nested-body and event-handler
  negatives, and the two closed local callback-return forms.
  Cost, accepted: the helper traversal is redone per call site rather than
  memoized by callee symbol. Depth is capped at 8 with a cycle guard and the
  walk only starts for a non-primitive call inside a leaf callback, so the
  fan-out is small; memoizing it is open work.
## Resolved: upstream quirks that contradicted the compiler

- **`on:`/`oncapture:` duplicate folding is gone** (2026-08-16,
  `solid1x_syntax.rs::duplicate_slot`). Upstream folds `onClick`/`onclick`/
  `on:click`/`oncapture:click` onto one name and reports runtime-legal pairs
  as duplicates. The compiler lowers `on:evt` to a bubble
  `addEventListener`, `oncapture:evt` to a capture `addEventListener`, and a
  non-delegated plain `on*` to one listener per occurrence — all attach, so
  none of those pairs is dead code. `v1/jsx-no-duplicate-props` now reports
  event-shaped names only for proven single-winner slots: the delegated
  `el.$$event = handler` property write (later-wins) and the statically
  valued template attribute (first-wins, shared with `attr:`). No upstream
  former upstream case pinned the folding; the product-owned cases now pin both
  directions directly;
  `fixtures/reactive-ir/eslint-compat` pins both directions.
  The slot model is **DOM lowering, so it applies to intrinsic elements
  only**. A component's props are a plain object the compiler never lowers:
  there the slot is the key as written, so `<MyComp onSave={a} onSave={b} />`
  and `<MyComp on:click={a} on:click={b} />` are real later-wins duplicates
  (the slot model would have silenced both), while `onClick`/`onclick` and
  `attr:title`/`title` are distinct keys.
  The static-value half is a *node-kind* test matching the compiler's inline
  branch (`StringLiteral`/`NumericLiteral`): `{0x10}` and `{1_000}` freeze,
  `{-1}`/`{+1}`/`{NaN}`/`{Infinity}` do not.

  `v1/event-handlers` (SC8001, `solid1x_attributes.rs`) now uses the same
  compiler node-kind predicate, so `{-1}`, `{+1}`, `{NaN}`, and `{Infinity}`
  are dynamic while radix and separator numeric literals remain static. The
  shared static-string resolver still covers upstream's proven string locals
  and literal concatenations. No former upstream case separated the two
  spellings; the focused fixture carries the regression instead.

  Adding the node-kind predicate was not sufficient on its own: a source-text
  arm (`text(span).parse::<f64>().is_ok() || static_string(..)`) survived in
  the same disjunction and decided the answer first, so `{-1}` and `{NaN}`
  still reported until it was removed. The diagnostic asserts Solid "will treat
  the value as an attribute", which is only true of the frozen forms, so the
  text arm was making a false claim rather than a conservative one. Pinned by
  `fixtures/reactive-ir/eslint-compat`'s `onClick={-1}`/`onClick={NaN}` pair
  (now clean) alongside the `onFoo="a"`/`onFoo="b"` static duplicates (still
  reported).

  **Closed 2026-08-18: non-frozen, non-callable handler values.** On a normal
  declared handler such as `onClick`, `onClick={-1}`, `onClick={NaN}`, and
  `onClick={someNumber}` are TS2322 and remain checker-silent. The uncovered
  case is a hyphenated attribute such as `on-event`, which TypeScript
  deliberately declines to check even though the compiler lowers every native
  `on` prefix as a listener. SC1007 now reads the exact runtime value domain and
  array shape there: a proven non-callable, non-array value is a violation; a
  callable/non-callable union, `any`, or an unresolved array/bound-pair shape is
  uncertifiable; a callable or absent handler is certified. Type assertions are
  peeled before classification. Real-typings oracle cases pin violation,
  uncertainty, and safe controls in both dialects.

## Audited remaining `TypeDescriptor.text` consumers 2026-08-17

**No consumer decides anything from `TypeDescriptor.text` any more** (verified
2026-08-18). Every remaining hit either labels a message or is a doc comment; the
two that made proof decisions were replaced by facts, below. The audit is kept
because it is what made the replacements findable:

- `interproc.rs` uses `text` only to label an unknown-callback diagnostic and
  generated contract stub; it does not make a proof decision.
- `solid1x_structure.rs` and the array branch of `solid1x_attributes.rs` asked
  a type-shape question (array/tuple versus a callable value) by matching
  descriptor text, because the Type Facts schema had no structural array-shape
  fact. **Resolved 2026-08-18** — see the `arrayShape` entry below.
- `shared_reactivity.rs` does not: its remaining `text` use is not a
  type-shape test.
- `server_rules.rs` asked whether a transport type has a rich serialization
  member (`Date`, `Map`, `Set`, typed arrays, and so on). **Resolved
  2026-08-18** — see the `libraryTypes` entry below. Its one remaining use of
  `text` quotes the author's type in the message; the decision never reads it.

Two consumers reintroduced a text decision while the SC1007 handler domain and
the SC7007 transport domain were being widened, and both are closed again:

- `shared_reactivity.rs::unchecked_handler_value_proof` certified an absent
  handler by matching `"null" | "false"`. `type Falsy = false` renders as
  `Falsy`, so the identical runtime value was a *proven violation* through an
  alias and silent as a literal. The runtime value domain cannot separate them
  — `null` and `false` both arrive as `may_be_other` — so the proof now comes
  from the AST: the literal written at the attribute, or the initializer of an
  immutable binding the reference resolves to.
- `server_rules.rs::argument_is_proven_json_safe` matched `"string" |
  "boolean" | "true" | "false" | "null"` for JSON safety, with the same alias
  asymmetry in the other direction — a spurious obligation on `type Name =
  string`. **Resolved 2026-08-21:** Type Facts' primitive value domain is
  structural and alias-transparent. Declared strings/booleans/null and
  safe unions now certify identically to literals; numeric domains certify
  only when every numeric constituent is a finite literal. No proof decision
  reads `TypeDescriptor.text`.

## Resolved: static attribute values are a fact, not a rendered type 2026-08-17

Bumping `typefacts` for the call-result domain also brought that revision's
node-selection change ("Classify complete demanded expressions"): a demand
resolves to the complete expression at its span rather than the deepest node at
its start byte. That is the correct subject, and it exposed a consumer
heuristic that had been right only by accident.

`upstream_compat::literal_string_type` recovered a static attribute string by
parsing `TypeDescriptor.text` for a rendered literal type, decoding JSON-style
escapes by hand. For `innerHTML={"a" + "b"}` the old selection typed the
leading `StringLiteral`, so a literal type appeared and the value read as
static; the complete `BinaryExpression` widens to `string`, so the same test
called a static value dynamic and `v1/no-innerhtml` reported it (upstream's own
case asserts it is valid under `allowStatic`).

The fix is the fact the migration guide asks for, not a repaired heuristic:
`constantValue` (`solid-ts-facts` `fc739a6c`, ADR 0014) is demanded at the
exact attribute-value span and accepted only as a present `kind: "string"`.
The producer folds literals, substitution-free templates, transparent
wrappers, unary signs, same-kind binary `+`, and compiler-resolved immutable
declarations (`const`, `readonly`, enum members), bounded by a depth limit and
a declaration cycle guard. Absence is "not proven constant", so a dynamic
value stays uncertifiable rather than guessed.

This is a precision *gain* in both directions, not only an FP fix:
`v1/jsx-no-script-url` now proves the scheme in
`href={"java" + "script:alert(1)"}`, which no literal type ever described, and
a `const`-referenced value is static wherever it was declared. Pinned by
`fixtures/reactive-ir/upstream-divergences`'s `FoldedMarkup` and `ScriptUrls`.
The later catalog reduction retired `jsx-no-script-url`; this paragraph records
the producer fact that remains useful to other semantic consumers.

Deliberately **not** folded into the producer: the *node-kind* tests. The 1.x
compiler inlines an attribute into the template on a `StringLiteral`/
`NumericLiteral` branch, so `jsx-no-duplicate-props` must keep asking what was
written rather than what it evaluates to — `{"a" + "b"}` is not inlined. The
`v1/event-handlers` inconsistency recorded above was that same syntactic
question and is now closed; see the note under the duplicate-props entry.

## Resolved: array/tuple shape is a fact, not a rendered type 2026-08-18

Two consumers decided "is this an array or a tuple?" by matching
`TypeDescriptor.text` against `[`, `readonly `, `Array<`, `ReadonlyArray<`, and a
trailing `[]`. Both were fail-closed, so both were false negatives, and text
could not settle the question in two independent ways:

- **An alias renders as its own name.** `type Handlers = [(data: number, event:
  MouseEvent) => void, number]` renders as `Handlers` and matched no prefix.
- **A trailing `[]` is ambiguous.** An array of functions (`((n) => void)[]`) and
  a function returning an array (`() => string[]`) render identically, which is
  why the screen also had to consult `callability` — and even then it could not
  see through the alias.

The fix is the fact, not a repaired heuristic: `arrayShape` (`solid-ts-facts`
`ce4c772`, ADR 0015) classifies the type at the exact demanded expression span
with the checker's own `isArrayOrTupleType` predicate over the real union
constituents. `array` requires every constituent to be an array or tuple;
`notArray` requires none to be, and is a positive claim so the negative is usable
as proof; `mixed` and `unknown` are proven states that prove neither side.
Absence stays fail-closed. `expression_descriptor` and `expression_callability`
had no other callers and were removed with the screen.

Closed false negatives, measured by A/B against the text screen on
`fixtures/reactive-ir/array-shape-v1`:

- `v1/no-array-handlers` now reports an aliased tuple, a doubly-aliased tuple,
  and a call returning one — all previously silent.
- `v1/prefer-for` now offers the `<For each>` autofix when the `.map` receiver is
  an *alias* for an array (`type Rows = string[]`). The alias hole had been
  withholding a correct rewrite, which is the same defect reaching a second rule.

### Narrowed in the same pass: `no-array-handlers`' `on:` arm

Writing the fixture against the real typings (`scripts/tsc-oracle.mjs`) showed
the fixture stub had been hiding a duplicate — the trap `fixtures/tsc-oracle`
exists for. `solid-js@1.9.14` types the two handler spellings differently:

~~~ts
type EventHandlerUnion<T, E, EHandler> = EHandler | BoundEventHandler<T, E, EHandler>;
interface BoundEventHandler<T, E, EHandler> { 0: (data: any, ...e: Parameters<EHandler>) => void; 1: any }

type EventHandlerWithOptionsUnion<T, E, EHandler> = EHandler | EventHandlerWithOptions<T, E, EHandler>;
interface EventHandlerWithOptions<T, E, EHandler> extends AddEventListenerOptions { handleEvent: EHandler }
~~~

`onEvent` accepts `BoundEventHandler`, an interface with members `0` and `1`, so
a `[handler, data]` tuple is legal and only this rule can object to it. `on:event`
has **no** bound arm, so every array and every tuple there is already `TS2322`:

~~~
Type '[(data: number, event: MouseEvent) => void, number]' is not assignable to
type 'EventHandlerWithOptionsUnion<HTMLDivElement, MouseEvent, ...>'
~~~

in both the strict and non-strict passes. The `on:` arm was removed under the
absolute rule; former upstream case `no-array-handlers__invalid__03` is recorded
as TypeScript-owned in the completed migration ledger.

### Closed 2026-08-18: the plain-array duplicate, via `tupleShape`

A **plain array** on `onEvent` has no `0`/`1` members either, so it was `TS2322`
too — confirmed for `((event: MouseEvent) => void)[]`, `ReadonlyArray<...>`,
`any[]`, and `unknown[]` — and the rule reported it anyway.

`arrayShape` could not settle it, by construction: it reports `array` for a plain
array and a tuple alike, because both of its consumers wanted the union of them.
The condition that is genuinely this rule's is not "array or tuple" but **"a
tuple with both numbered slots whose first can be called with `(data, event)`"**
— exactly what `BoundEventHandler` accepts and `tsc` therefore permits.

`tupleShape` (`solid-ts-facts` `b9d1a8e`, ADR 0016) supplies it: fixed slot count,
whether a rest tail follows, and the first slot's callability *and minimum
arity*, present only when the type at the exact span is itself a tuple. The rule
now fires on that and nothing else. `tsc` names each removed shape's reason
precisely, which is how the partition was checked:

~~~
((event: MouseEvent) => void)[]      missing the following properties: 0, 1
[number, number]                     Types of property '0' are incompatible
[(event: MouseEvent) => void]        Property '1' is missing
[1, 2, 3]                            Type 'number' is not assignable to (data: any, e) => void
[(a, b, c) => void, number]          Target signature provides too few arguments
~~~

The last row is the arity residual, closed by an amendment to ADR 0016.
`elementZero` says the slot is callable, which does not settle whether it can be
*invoked* with the two arguments Solid passes: `BoundEventHandler` types slot 0
as `(data: any, ...e: Parameters<EHandler>) => void`, and `EventHandler` takes
one parameter. A handler requiring three is callable, and not callable here.
Adding `elementZeroMinimumParameters` took the fixture's SC8007 count from 6 to 5.

Against the fixture, `handler-cases.tsx` now holds every SC8007 and produces
**zero** `tsc` diagnostics, while `clean-cases.tsx` produces eight and **zero**
findings. The rule and the type checker partition the space exactly.

**Contextual typing is what makes this work, and it is load-bearing.** An array
literal written where the expected type has numbered members acquires fixed
slots; the same literal in an unconstrained position stays a plain array. So
`tupleShape` absent *plus* an array literal written here means no bound-handler
type applies at this attribute — the project's JSX typings are not checking it,
and the rule is the only thing that can speak, which is the boundary
`jsx_name_is_type_checked` already draws. The syntactic fallback is kept for
exactly that case.

The consequence is that `fixtures/reactive-ir/array-shape-v1` had to stop using a
permissive `IntrinsicElements` index signature: its stub now carries the real
`EventHandlerUnion`/`BoundEventHandler` signatures, because a looser stub erases
the contextual typing the rule depends on and the fixture would stop exercising
its own path. The retired upstream corpus used a permissive harness, so three
former cases (`no-array-handlers__invalid__03`, `__05`, `__07`)
are recorded as TypeScript-owned in the completed migration ledger, each
verified against the real typings before the corpus was retired.

### Closed 2026-08-18: unions of tuples

A union had no `tupleShape` at all — the fact answered only for a type that was
itself a tuple — so `Handlers | OtherHandlers` and the very common optional
`Handlers | undefined` both failed closed. `arrayShape` reported `mixed` for the
optional form, which proves neither side, so the rule went silent on values that
are a bound pair whenever they are anything.

The fact now answers for a union with its constituents' **meet**: the slots they
all have, a rest tail only if all carry one, callable only if all are, and the
largest argument requirement among them. What it reports therefore holds
whichever constituent the value turns out to be. A single non-tuple constituent
voids the answer, and nullish constituents are skipped because they carry no
structure — presence stays `runtimeValueDomain`'s question. The payload and Wire
table schema are unchanged; this widened when the fact is emitted, not its shape.

`tsc` agrees on every boundary, which is how it was checked: `H1 | H2` and
`H1 | undefined` are silent (ours, now reported), while `H1 | number[]`,
`H1 | [number, number]`, and `H1 | [(a,b,c) => void, number]` are each TS2322
(the type checker's, still silent here).

### Closed 2026-08-18: mixed handler shapes and runtime presence

A union that mixes a bound pair with a **plain function** (`Handlers |
((e: MouseEvent) => void)`) has no `tupleShape`: one constituent is not a tuple,
so no violation holds for every runtime value. It no longer disappears. The
handler expression now also demands `runtimeValueDomain`; callable/non-callable
unions, `any`, and generic shapes produce an explicit **uncertifiable** SC8007.
An unresolved import remains silent because TS2307 owns that source.

The same audit found two adjacent proof errors:

- `Handlers | undefined` has a common tuple shape, but a tuple is not present on
  every execution. With `strictNullChecks` disabled TypeScript erases the
  undefined constituent before the runtime-domain fact is computed, and the IR
  does not receive that compiler option. A violation is therefore reserved for
  a structurally proven runtime array (an inline literal or immutable local
  array initializer); type-only pair evidence remains uncertifiable.
- A TypeScript assertion was treated as a safety voucher. JSX facts now record
  whether a wrapper is a runtime type escape, and the rule demands/classifies
  the peeled runtime expression. An asserted array is a violation, an asserted
  function is safe, and an unresolved runtime shape is uncertifiable.

The focused fixture now pins proven violations, proven-safe controls,
TypeScript-owned invalid tuples/arrays, and every uncertifiable branch. The
real-typings oracle carries strict and non-strict cases for a pair/function
union, an optional pair, an asserted array, and an asserted-function control.

### Closed 2026-08-18: `rich_transport_member`, via `libraryTypes`

`server_rules.rs` asked whether a server-function argument is one of a few
runtime types JSON cannot carry (`Date`, `Map`, `Set`, `RegExp`, a typed array).
It answered by splitting `TypeDescriptor.text` on top-level `|`/`&`, stripping a
`[]` suffix, and matching the head against a name list. It was the second
`TypeDescriptor.text` consumer named alongside `array_like_type`, and it was
deliberately left alone when `arrayShape` landed on the grounds that its question
was open-ended.

That framing was wrong, and worth recording. The open-ended question is "does
this object graph contain a non-JSON-safe member" — a recursive walk needing a
cycle guard, a depth bound, and a nesting policy. But that is not the question
this rule asks. It asks only about the **top level**, by deliberate design ("an
unproven rich type is not a proven throw"), and a bounded top-level question is
exactly the kind a fact can answer.

`libraryTypes` (`solid-ts-facts` `3d51c40`, ADR 0017) answers it: the sorted set
of standard-library type names the type at the exact span is built from at its
top level — itself, its union and intersection members, and one array-element
unwrap. A name is recorded only when the resolved symbol is declared in a
default-library file. The rule keeps its own list of which names matter, and that
a lone `Uint8Array` has a natural HTTP encoding; the fact carries no policy.

Three defects text could not avoid, all closed:

- **An alias renders as its own name.** `type Stamps = Date[]` matched nothing,
  declared locally or imported from another module. Measured on
  `fixtures/reactive-ir/server-function-rich-args`: the text walk found 6
  findings, the fact finds 8.
- **`Array<Date>` and `Date[]` are the same runtime value** and only the second
  matched.
- **A user-declared type could match a global's name.** `Map` from the project's
  own code is now excluded by its declaration file, not hoped away by spelling.

The fact's nesting boundary remains unchanged, but it no longer becomes a
checker blind spot: `Boxed = { title: string; when: Date }` produces an explicit
uncertifiable SC7007 because the complete object graph is not proven JSON-safe.
`tsc` cannot duplicate any of this — the argument's type matches its parameter's
type by construction, so no diagnostic is possible; the claim is entirely about
runtime transport.

## Closed 2026-08-18: unresolved generic member dispatch is explicit

Generic member calls now have three outcomes instead of a silent fail-closed
branch. One exact implementation contributes its summary; a finite set of
implementations contributes a summary only when every candidate has the same
reactive-read behavior; missing or divergent behavior produces SC9012
`reactive-dispatch-unresolved` as an `uncertifiable` finding.

The explicit obligation covers parameter-member substitution at each call
site, computed calls whose TypeScript call is valid, and direct member dispatch
on parameters of exported helpers with unseen callers. A compiler-proven
tracked JSX call is safe regardless of which implementation reads, and an
exact standard-library method is not open dispatch, so neither produces an
export obligation. Member calls nested in returned, assigned, or scheduler
callbacks are not mistaken for direct export behavior: the existing callback
execution contract owns those paths. These distinctions keep JSX collection
helpers and higher-order adapters certifiable while refusing a falsely empty
summary for a helper whose own execution directly depends on an unseen
implementation.

`fixtures/reactive-ir/interprocedural-methods-v2/` pins all three finite-set
outcomes: an exact reactive object is a proven SC1001 read, reactive candidates
with equivalent summaries remain a proven read, and reactive/inert candidates
produce SC9012. It also pins valid computed dispatch as SC9012 and a direct
export boundary. `fixtures/reactive-ir/v1-reactivity/` pins the same shared
obligation under the v1 identity. The real-typings oracle carries keystones for
both dialects and TS2349 negative controls; invalid calls remain TypeScript's
job and never receive SC9012.

## Closed 2026-08-22: package contracts preserve parameter-member reads

Schema version 1 now has the additive `reactiveReads` form
`{ "kind": "parameter-member", "parameter": N }`. The producer already knew
the exact parameter symbol behind a direct member receiver; it now exports that
provenance instead of refusing every JavaScript runtime artifact at the open
package boundary. Local and module-local receivers are unchanged and do not
become public effects.

Consumers instantiate the row per call site. Proven reactive store/path
arguments contribute a read, inline primitive/array literals are clean, and an
opaque argument is SC9012 rather than guessed plain. Local wrapper summaries preserve
the same parameter provenance. The package generator fixture
`fixtures/package-contracts/parameter-member-read/` pins the runtime-artifact
claim and its negative local-receiver controls;
`fixtures/reactive-ir/package-parameter-member-consumer/` pins the reactive,
plain, and uncertifiable consumer outcomes.

Argument-value/identity-dependent dispatch remains deliberately open. A
contract variant keyed by an arbitrary runtime argument would export the
callee's dispatch table, has no bounded exhaustiveness proof, and is not the
same thing as environment `variants` with ordered export-map conditions.
`solid-recharts`-style sentinel dispatch therefore remains correctly
uncertifiable. Parameter-attributed writes are also not claimed: their
operation and ownership semantics need a separate design rather than symmetry
by spelling.

## Closed 2026-08-22: legacy ESM roots reach contract generation

The package generator no longer requires `package.json#exports` when the
runtime artifact still has one exact legacy ESM root. It recognizes `module`,
an ESM-safe `main`, and an unambiguous ESM index fallback, all through the same
entrypoint-resolution module used before semantic analysis. The negative CJS
fixture pins that a conventional `main: index.js` without ESM package semantics
is refused rather than interpreted under the generator's TypeScript settings.
Missing, absolute, escaping, declaration-only, and CJS targets remain
unsupported. This changes package-shape reachability only; it does not add
trust or weaken any reactive proof obligation.

On the fixed 417-probe ecosystem manifest, 7 of the 11 former
`unsupported-package-shape` probes now generate reviewable drafts. Two more
reach semantic analysis and expose their real unresolved read/dispatch
obligations; one is correctly classified CJS-only, and one declares a missing
module artifact and remains no-ESM. Whole-corpus success moves from 336/417
(80.58%) to 343/417 (82.25%) with zero timeouts.

## Closed 2026-08-22: contracts distinguish proven none from unknown

Schema version 1 now accepts `{ "status": "unknown" }` in each existing
effect-claim field. Omission retains its previous reviewed meaning of proven
none. The marker occupies the existing field rather than a new sibling, so an
old loader rejects its incompatible type instead of ignoring a new property
and failing open. The Rust contract module normalizes both wire forms behind a
single `ContractClaim<T>` interface.

An exact exported callback obligation emits a partial reviewable draft with
`callbacks` unknown and keeps independently proven reads, returns, owner
requirements, and async behavior.
Consumers demand that uncertainty only when a call supplies a potentially
callable value; a no-argument call remains clean. Read, return, owner, and async
obligations now become unknown only in their affected domain. Exact containing
function identity keeps clean sibling exports intact; an import-level or
transitive obligation that cannot be joined to one export falls back to all
applicable function exports, which is conservative but permits a reviewable
entrypoint draft.

Callback rows also carry bounded `arguments` descriptors. A producer records a
fresh accessor passed to a callback parameter, and a consumer marks only the
matching callback-function parameter reactive. The
`callback-reactive-arguments` package fixture pins the producer behavior and
ensures the handoff itself is not reported as an uncaptured read.

`fixtures/reactive-ir/package-unknown-callback-consumer/` pins the demanded and
non-demanded consumer cases, and
`fixtures/reactive-ir/package-unknown-returns-consumer/` pins the other half:
a non-callback domain, which is opened where the claim enters the project
rather than where a call demands it, with a sibling export whose summary
withholds nothing staying clean.
`cli_reports_the_exact_unknown_claim_domain` holds the finding to naming the
one domain left unknown, since a summary that states four domains and
withholds one is not the same evidence as a summary that states nothing. The
existing unknown-callback producer process
fixtures pin partial emission, a known sibling callback summary, and cyclic
forwarding termination. Unresolved dispatch, unknown package identity, and
unreviewed evidence remain correctly uncertifiable at consumption even when
generation can write a partial draft.

The fixed 305-row/417-probe ecosystem manifest measures the result directly.
All 21 probes previously classified as `unresolved-parameter-behavior` now
generate contracts, so that class falls from 21 to zero. Across the complete
worktree (including the companion parameter-member and runtime-identity
slices and legacy ESM resolution), success rises from 291/417 (69.78%) to
343/417 (82.25%), a gain of 52 probes or 12.47 percentage points. The corrected full-run target uses the
documented 600-second budget and completed with zero timeouts; the generated
JSON and Markdown reports live under `benchmarks/ecosystem/`.

## Closed 2026-08-22: package generation reaches the artifact ceiling

The remaining semantic generation classes on the pinned ecosystem corpus are
now zero. The generator represents ordered conditional branches whose export
`kind` differs, recursively generates and caches exact installed dependency
contracts, scopes unresolved obligations to the affected export and claim
domain, carries accessor-valued callback arguments, and treats exact
standard-library declarations as platform behavior rather than an unresolved
package dispatch. None of these inferred drafts become reviewed evidence.

Generation projects now contain the exact static relative runtime-module
closure of an entrypoint instead of every JavaScript file below its distribution
directory. This keeps published `.js` barrels ahead of adjacent declarations
without repeatedly loading unrelated bundles. Return facts are also assigned to
their innermost summary and AST owners once, instead of rescanning every return
for every function. Returned-factory lookup likewise indexes exact binding,
factory, return-owner, symbol, and function-span relationships once per file,
instead of rescanning the bundle for every direct call. On the former
`@tanstack/ai-devtools-core` timeout, these changes reduce generation of both
entrypoints from 128.32 to 10.37 seconds in the debug-binary reproducer. Release
cold analysis falls from 4.03 seconds to 0.474 seconds; return-summary
attribution itself falls from 3.13 seconds to 34 milliseconds, and
interprocedural graph construction falls from 314 to 33 milliseconds after the
return-attribution optimization.

The measured result is **407/417 (97.60%)**, up from **343/417 (82.25%)**:
64 additional successful probes and 15.35 percentage points. The remaining ten
failures are six npm peer-resolution failures, three packages with no usable
ESM runtime artifact, and one CJS-only tsup bundle. There are zero timeouts and
zero semantic contract-generation failures. CJS stays fail-closed: the one
remaining bundle exposes generated `__export`/`__toCommonJS` machinery rather
than a statically auditable `module.exports` surface, and declarations are not
runtime proof. The exact report is `benchmarks/ecosystem/report.json` with the
human-readable companion `report.md`.

The full-corpus target now measures the optimized product binary rather than a
debug checker and schedules `min(available CPUs, 8)` probes concurrently. On
the same 417-probe manifest, wall time is **104.394 seconds**, down from
**238.502 seconds** for the four-worker debug run (56.23% less, 2.28x faster),
with the same 407 successes and ten artifact/install failures. Reports now
record installation and generation separately: this run spent 542.020 seconds
of aggregate worker time installing, 211.963 seconds generating, and 0.879
seconds in harness bookkeeping. The remaining full-run floor is therefore
primarily isolated npm resolution rather than semantic contract analysis.

**Superseded as the current figure, 2026-08-22.** The numbers above are the
measurement of that change on the then-current 417-probe manifest and stay as
history. The manifest is now 305 rows / 416 probes, and the regenerated
reports read **403 complete contracts, 6 partial, 7 failures** in 94.286 s —
not a regression from 407/417 but a stricter count, since `partial-success`
had not yet been split out when 407 was measured. See "The ecosystem benchmark
counted partial contracts as successes" below for the full old-versus-new
accounting.

## Closed 2026-08-22: contract review no longer certifies unobserved callbacks

Five defects in the package-contract slice shared one shape: generation
succeeded where it could only have said "unknown", and the review plan no
longer surfaced the negative claim that resulted. They are recorded together
because fixing any one of them alone leaves the same class open.

**Callbacks forwarded into an unresolvable callee.** A call whose callee had
neither a dispatch candidate nor a resolvable identity was dropped from the
graph entirely. That is `list.map(fn)` where `list` is one of the exported
function's own parameters -- `any` in every published JavaScript runtime
artifact, since the generation project deliberately keeps the runtime `.js`
ahead of its adjacent declarations. The forwarded callback escaped with no
recorded behavior, and an omitted `callbacks` field is a *negative* claim, so
silence certified "never invoked". `main` refused these packages outright; the
parameter-member slice replaced that refusal without covering the callback
path underneath it. Measured on the real registry:
`@solid-primitives/utils@6.3.2` claimed `map`, `filter`, and `sort` invoke
nothing and `tryOnCleanup` needs no owner, and
`@solid-primitives/event-listener@2.4.4` claimed `makeEventListener` never runs
its handler. A consumer of the promoted contract reported SC1001 on a signal
read inside a DOM click handler -- a proven violation asserted from a claim the
contract never had. Such calls now open the existing unknown-callback
obligation, scoped to arguments that are parameters of the enclosing exported
function and whose own syntax does not already prove them inert.

Two consumer-side halves follow from it. A literal argument is now proof of
non-callability in its own right (`slice(list, 0, 2)` demands nothing from an
unknown callback claim, where every argument previously did, because the type
system reports "potentially callable" whenever it has no type at all). And a
read inside a callback whose contract timing is unknown is no longer reported
as a proven untracked read: the call already carries an SC9005 obligation, and
claiming the timing on top of it asserts exactly what the contract says it does
not have.

**`default` branches were unmatchable.** Generation encodes an export map's
fallback as the literal condition `default`, but `selected_conditions()` never
produces that string, so `matches_conditions` could never satisfy it. Every
consumer with a real environment selected fell through to an
environment-dependent uncertifiable result -- including the one the fallback was
generated for. `default` is now satisfied by any selected environment and by no
selection at all, and `precedence` decides among several matching branches.
Handwritten contracts, which carry no `precedence`, resolve only the case that
needs no invented order: a named branch beats the unconditional fallback, while
two named branches stay fail-closed.

**`--conditions` erased the environment it was scoped to.** A contract
generated with an explicit selection recorded nothing about it, so a consumer
in any other environment applied it. The selection is an assertion about the
resolving environment rather than an observation of the export map, so a
branching entrypoint now carries it and a differing consumer fails closed. An
entrypoint with one unconditional target still records nothing.

**Conditional export-name absence was silently unconditional.** A name observed
in only some branches was published as a complete unconditional summary,
handing a consumer in the other environment a claim about an export that does
not exist there. The proven branches are now retained as `variants` even when
they agree, and normalization no longer collapses a variant set that fails to
cover its entrypoint's conditions.

**Legacy `module`/`main` provenance was invisible.** A legacy dual package's
contract describes only the analyzable ESM build. Refusing the package would
reject every legacy dual package, including the common case where `main` is the
CJS transpile of the same source, so the review plan now names the field the
root came from and says when `main` points elsewhere.

**The checklist section that would have caught all of this was removed.** The
"callbacks with no execution row" section is restored; `docs/package-contracts.md`
had continued to promise it.

Regression pins: `fixtures/package-contracts/unresolved-callee-callback`,
`conditional-export-absence`, and `legacy-dual-root` in the contract corpus;
`selected_variant` and `RuntimeEnvironment::matches_conditions` unit tests; a
review-plan test for legacy provenance; and
`fixtures/reactive-ir/package-variant-precedence-consumer`, which carries the
selection through to a consumer's proof. Its two exports declare the same two
overlapping branches and differ only in `precedence`: the unique lowest one
resolves the branch whose accessor return makes an untracked read provable,
and the tie leaves the import binding uncertifiable with the identical read
unreported. The unit tests alone could not distinguish a working selection
function from one whose answer never reached a consumer.

Remaining approximation, deliberately: an argument to a `parameter-member` read
whose origin the project cannot see -- a parameter, a prop, an import, a bare
`declare const` -- stays SC9012. A Solid store is a proxy typed as the object it
wraps, so no declared type proves the negative; only inline literal syntax or an
analyzed initializer with a standard-library origin does.

Generation reachability did not move, which is the expected result: a contract
carrying an explicit unknown marker is still a generated, reviewable draft. The
run that first measured these fixes read 407/417 on the manifest they were
reviewed against, matching the run before them exactly, with zero timeouts and
zero semantic contract-generation failures.

What moves is review surface, which is the point. On two pinned versions,
holding the package release constant so the comparison is not confounded by a
package update: `@solid-primitives/utils@6.3.2` goes from 23 to 73 checklist
items (43 of them the restored "callbacks with no execution row" section, and
unknown export claims rising from 2 to 9), and
`@solid-primitives/event-listener@2.4.4` from 12 to 14 (unknown export claims 3
to 7, including `makeEventListener`). A run that does not show that rise has
not applied these fixes.

The corpus denominator has since changed twice, so those two numbers are not
directly comparable to the current report -- see the entry below.

## Closed 2026-08-22: the ecosystem corpus measures a real environment

Three selection and reporting defects made benchmark numbers describe the
harness rather than the ecosystem. All three were found by reading the failure
list rather than the success rate, which is the general lesson: 407/417 was
stable across a run that shipped materially wrong contracts and a run that
fixed them.

**Solid 2 floors selected environments nobody supports.** The 2.x line spent a
long time in `experimental` and `beta`, so a package published this month can
still declare a range whose formal lower bound is an old beta while its own
dependencies have moved on. Flooring at that bound produced peer conflicts that
described nothing. The floor is now anchored at `2.0.0-rc.0`, and only ever
raised: a range accepting no `rc` keeps its own oldest accepted beta, the same
rule that keeps a beta-only package off a newer release candidate at the head.
`compatibleSolidVersions` still records the complete accepted set, so the range
fact is preserved and only the probe moves. Seventeen genuinely beta-only
probes remain in the corpus.

**Floor tuples were assembled per package and could not coexist.** Flooring
`solid-js`, `@solidjs/web`, and `@solidjs/signals` independently can synthesize
an environment that has never existed. `@tanstack/solid-router@2.0.0-rc.1` pins
`@solidjs/web@^2.0.0-rc.1`, and that release peers `solid-js ^2.0.0-rc.1`, so a
floor pairing `solid-js@2.0.0-rc.0` with web rc.1 was refused by npm before the
checker ran. The selector now raises a floor to a fixed point until the runtime
packages accept each other, collapsing the row to one `only` probe when floor
and head coincide. The catalog carries each runtime release's declared ranges on
its siblings to make that decidable.

**Two failure classes were conflated.** `no-esm-runtime-target` meant both "the
package declared a runtime target that does not exist" -- a publishing mistake --
and "the ESM target resolved, parsed, and exports nothing", which is a
well-formed side-effect-only module with no reactive surface to describe. The
second is now `no-exported-surface`. It is still a failure rather than a
success: promoting it would require the generator to emit an empty contract,
which is a semantics change and not a reporting one.

Two reporting defects travelled with them. A filtered run wrote the canonical
`report.json`, so a 23-probe sentinel silently replaced the full-corpus
artifact, and the report recorded no scope, so its header described the
manifest's 417 probes while its body held 23 results -- a clobbered report was
undetectable. Reports are now named for their scope, record it, and refuse a
`--baseline` from a different one. Separately, `diffManifests` compared version
and integrity but not probes, so a policy change printed "(no changes)" directly
above `--check`'s "file is out of date" verdict. `--check` itself was never
wrong: it compares the whole serialized document and correctly refused such a
manifest. The diff now reports probe changes, on the rule the same file already
states for exclusions and limitations.

The measured result is **409/416**. The denominator fell by one because the
incoherent `@tanstack/solid-router` floor collapsed into its head rather than
being probed as an environment that cannot install. All seven remaining
failures are outside this repository: two packages whose published manifest
names a file absent from the tarball (`@kobalte/themes`,
`@solid-primitives/composites`), two with self-contradictory peer ranges
(`@kobalte/solidbase`, and `@tanstack/solid-router-ssr-query` on both probes,
whose `@tanstack/solid-query >=5.90.0` peer cannot select any Solid 2 build
because every such release is a prerelease and a non-prerelease range never
matches one), one correct CJS refusal, and one side-effect-only module. Zero
timeouts, zero `type-facts-failure`, zero semantic contract-generation failures.

## Open: generation success is not contract correctness

The ecosystem benchmark counts whether `contract generate` produced a document,
not whether the document is true. That distinction is not academic: the run
immediately before the unknown-callback fixes measured 407/417 while
`@solid-primitives/utils` shipped a contract asserting that `map`, `filter`, and
`sort` never invoke their callbacks, and `@solid-primitives/event-listener`
asserting that `makeEventListener` never runs its handler. The metric was within
a percent of its cap and materially wrong at the same time.

The current run emits **14,309 checklist items across 409 invocations**, a mean
of 34. Every one of those contracts is `inferred` evidence held below the SC9005
trust ceiling, and none of those items has been reviewed against the packages'
published sources. So the corpus establishes that the generator reaches
essentially every installable package, and establishes nothing about whether
what it writes is correct.

Closing this needs a different measurement: review a sample of generated
contracts by hand against the real sources and count how many checklist items
resolve to "the generator was right" versus "the generator claimed something
false". Until that exists, a high success rate should be read as reachability
only, and the contract corpus under `scripts/contract-corpus.mjs` plus the
fixture snapshots remain the only checked-in evidence about correctness.

## Design-change candidates (open)

### `execution-map-incomplete` (SC9004) moved to producer integrity

Both dialect compilers emit every `jsx-expression` operation together with a
same-span region or callback role in every decision arm, and
`CompilerFacts::classifies` matches by span containment — so
`uncovered_jsx_expressions()` is empty by construction. The former SC9004
project rule could therefore describe only externally produced or partial
compiler facts, not a defect in analyzed source. It was removed from both
catalogs on 2026-08-20 and retained as a producer-integrity requirement. If a
third compiler adapter lands, its adapter tests must prove the same totality.

### Resolved: shorthand property values follow compiler runtime identity

TypeScript projects a shorthand property's *own* symbol at `{ pathname }` --
never the referenced value binding's -- so no TypeFacts entity, reference, or
declaration fact at that span identifies the value. The binder that builds the
Oxc AST facts does resolve that exact reference, and its answer is now carried
on `ObjectPropertyFact::shorthand_binding`; `interproc.rs`
(`binding_initializer`, `named_accessor`) reads the declaration from it instead
of matching the spelling within the enclosing function. That is scope-exact, so
the previous block-scoping hole is closed in both directions.

The cross-file gap is now closed from TypeScript's exact runtime identity and
symbol chain, not from a second module resolver. The imported binding must
carry a non-empty identity and its alias chain must end at a declaration in an
analyzed project source file. That directly incorporates the project's
selected module mode, extension priority, `baseUrl`/`paths`, package exports,
and re-export traversal without mistaking a project re-export of an external
value for project ownership. `interproc.rs::imported_accessor` joins the same
identity to the exact accessor/source export; the former textual relative
resolver remains only an accessor fallback for older or missing facts.

Two boundaries remain unavailable for an exact structured-property claim.
Each exported shorthand produces SC9012 instead of disappearing, and the
generated summary omits the unproven property rather than inventing a leaf:

- **external packages.** Their symbol chain has no declaration in an analyzed
  project source, even when a relative project module re-exports the value.
  Exact behavior belongs in an audited package contract.
- **globals/unresolved bindings.** A namespace import is an exact non-reactive
  namespace object and remains certified without SC9012. An unresolved export
  cycle is TypeScript-owned (TS2303), so it receives no checker finding.

What the fixture pins today is the same-file resolution set
(`scopedShorthand`, `unprovenShorthand`, `shadowedShorthand`,
`writtenShorthand`), the cross-file named-import join
(`importedAccessorShorthand`), compiler-selected ambiguous and path-mapped
joins (`ambiguousShorthand`, `pathMappedShorthand`), a nondeterministic import
set (`importedShorthand`, `namespaceShorthand`, `bareImportShorthand`,
`cyclicReexportShorthand`),
the default/named/export-all joins (`defaultReexportShorthand`,
`namedReexportShorthand`, `exportAllShorthand`), and a global
(`globalShorthand`).

The focused unresolved fixture now asserts three obligations explicitly: a
bare external import, the same value behind a relative project re-export, and
a global binding. Its path-mapped and ambiguous-relative controls certify
through runtime identity and project declarations. Exact local non-reactive
values and namespace objects likewise remain certified without SC9012.

The shared `solid_facts::resolve_relative_module_path` helper now answers
"which file does this relative specifier name" for both
`interproc.rs::relative_module_file` and the backend's
`resolve_relative_export`. It is lexical, project-local, and returns no
answer when extension/index candidates are ambiguous.

## Partially resolved design changes

- **`v1/jsx-no-undef` now fails closed on missing semantic facts.** It reports
  unresolved `use:` names only when the structural binder proves that no
  lexical binding exists. Unresolved JSX tags, including dotted roots, are
  TypeScript-owned (TS2304) and checker-silent rather than a second diagnostic.
  The old auto-import helpers remain test coverage for the upstream formatting
  logic, not a blanket semantic allowlist.
- **Unknown callback helpers remain contract obligations.** Exact TypeScript
  call facts now enrich the obligation and diagnostic with package,
  entrypoint, export/function, callback parameter index/type, required
  execution mode, and an editable schema-v1 contract stub. Standard-library
  behavior and project/package contracts can discharge it; unknown execution
  remains refused until an explicit contract proves it.

## Resolved design changes

- **Shorthand property values are resolved by the binder, not by spelling.**
  The value binding at `{ pathname }` is named by
  `ObjectPropertyFact::shorthand_binding` -- the declaration Oxc's scope tree
  chose for that exact reference -- so a same-spelled binding in a sibling
  block neither substitutes for the intended one nor makes it ambiguous. This
  replaced a spelling match scoped to the enclosing function, which both lost
  a provable structured return whenever any sibling block reused the spelling
  and, worse, could certify an accessor the shorthand never named. A shorthand
  the binder leaves unresolved carries no fact and proves nothing. The
  remaining cross-file gap is listed above.

- **Invoking a parameter's member is resolved per call site.** A function that
  calls `reader.read(value)` on its own parameter makes no claim about which
  implementation runs — that belongs to each caller. The owner records the
  obligation (`invoked_parameter_members`: parameter index and property), and
  `interprocedural_reads` resolves it against the argument actually passed at
  each site, the way `invoked_parameters` already substitutes a directly
  invoked parameter. One exact object contributes its reads; several exact
  objects contribute their common summary only when those summaries are
  equivalent. A missing or divergent implementation produces SC9012 rather
  than contributing no read. This replaces both the old pooled answer (which
  contaminated an exact site with an ambiguous sibling) and the later silent
  omission at the ambiguous site.

- **Callee resolution is exact and conservative.** Parenthesized, `as`,
  `satisfies`, and non-null wrappers are peeled through a shared AST fact
  helper. Resolved call declarations identify member callees when TypeScript
  provides them; static members can use their exact property entity, while a
  TypeScript-valid computed member such as `handlers[i]()` produces SC9012
  instead of inheriting `i`, inheriting `handlers`, or disappearing. An invalid
  computed call remains silent because TypeScript owns it.
- **Summary discovery covers method, alias, and returned-value branches.**
  Class/object methods, returned closures, conditional aliases, destructured
  function properties, and exact object spreads retain their canonical
  symbols. Direct generic calls and resolved structural member calls propagate
  summaries only through the dispatch proof described above; a finite
  unresolved dispatch remains explicitly uncertifiable through SC9012.
- **Transparent TypeScript wrappers are peeled at equality gates.** The
  shared helper is used by map/callback discovery, Solid 1.x structure gates,
  and shared reactivity function matching, with AST and fixture coverage for
  parentheses, `as`, `satisfies`, and non-null assertions.
- **Namespace-imported JSX primitives use dialect vocabulary.** `<Solid.For>`,
  `<Solid.Show>`, and `<Solid.Repeat>` resolve only when the namespace import
  is from a dialect-owned module and the member is in that dialect's export
  vocabulary. The namespace and named-import twins are pinned by
  `fixtures/reactive-ir/namespace-import-v2/`.
- **Component identity conventions are dialect-owned.** JSX call sites,
  direct JSX returns, and exact compiler-resolved Solid component aliases prove
  component identity. Solid 1 does not promote upstream's uppercase-name
  shortcut to proof: capitalization makes component identity **uncertifiable**
  until a JSX call site or exact component type selects the execution model.
  This uncertainty propagates through ownership, props, reads, destructuring,
  conditional returns, handler reads, and mutations. Solid 2's
  direct-JSX-return convention remains dialect-owned. The shared reactive core
  contains no hard-coded proven-component casing rule. Intrinsic-tag case
  checks remain syntax-only.
- **Dialect-owned type origin is no longer enough to register a source.** The
  dialect classifies exact exported aliases as component, accessor/resource,
  signal, store, setter, or store setter; user-local lookalike aliases and
  unrelated Solid types do not become accessors.
- **Unclassified function spans are `Unknown`.** Explicit compiler-untracked
  regions and other semantic proofs become `UntrackedRendering`; AST-proven
  module evaluation is its own one-shot role. Unknown reads/writes are not
  projected as violations.
- **Owner-shape recognition is AST-backed.** Binding immutability, array
  slots, call initializers, returned functions, and arrow kind now come from
  facts rather than scanning source bytes.
- **Compiler-established ownership is trace-backed.** Default compiler effect
  reruns emit typed owned regions without changing generated code. Custom
  wrappers make no claim; component and runtime-callback ownership still comes
  from exact TypeFacts identity and package contracts.

## Closed 2026-08-22: two vendored Solid 1.x compiler census gaps

Found by the ecosystem benchmark (`docs/ecosystem-benchmark.md`), which ran
`contract generate` against 417 real package/Solid-version probes. Two probes
failed with the `type-facts-failure` class, and both are **defects in the
vendored Solid 1.x JSX compiler, not in this repository's `rust/` tree**. Both
are recorded here rather than worked around: the checker is failing closed for
a reason that is real, but the reason is a bookkeeping disagreement inside the
compiler, not genuine ambiguity in the analyzed package.

Both live in `packages/compiler/src/semantic_trace.rs` of
`github.com/yumemi-thomas/solid-1x-compiler`, pinned at rev
`79b9b63721c59b0acfd72348438bbb6e090ec81c` (`rust/Cargo.toml`'s
`solid1-dom-expressions-compiler`). That file reconciles a **census** (every
JSX site the compiler owes an answer about) against the **trace** (the answer
lowering actually gave); `TraceRecorder::finish()` fails when the two disagree.
`rust/dialects/solid-v1/compiler` only consumes the finished trace and has no
seam to intervene, so neither is fixable from here. Fixing them means an
upstream change plus a `rev` bump per `docs/monorepo.md` — not a floated
branch.

Both are **Solid 1.x only**; the identical constructs compile cleanly under the
Solid 2.0 dialect, whose fork does not share either mechanism.

- **Static `style`/`classList` object before a later spread, on a native
  element.** Reported as `semantic trace has unresolved execution sites:
  NativeAttribute@<span>`, where the span is the whole object literal.
  Observed on `@kobalte/core@0.13.13`; reproduced in eight lines as
  `<input tabIndex={-1} style={{ "font-size": "16px" }} name={props.name} {...props} />`.
  The census decides whether to decompose the object using an element-wide
  "does this element have any spread" flag, while lowering uses a
  per-attribute, position-aware test (`!seen_spread && !dynamic`) that peels a
  static pre-spread attribute back into the ordinary template planner. The
  census records one opaque site that nothing then resolves. Removing the
  spread, or making the style value dynamic, both avoid it.
- **JSX fragment nested inside a callback passed as a control-flow built-in's
  prop.** Reported as `semantic decision targets an uncensused JsxChild site at
  <span>`. Observed on `@tanstack/ai-solid-ui@0.7.17`; reproduced with a
  `<Show fallback={(() => { ... return cond ? <>{expr}</> : null })()}>`. The
  census tracks "am I under a component" in a mutable `parent_component` flag
  that survives the recursive walk into attribute-value expressions, so a
  fragment created inside a separately scoped closure is censused as
  `ComponentChild` while lowering correctly decides `JsxChild` for the same
  span. Same span, different kind, so reconciliation fails. Solid 2.0 replaced
  the flag with an explicit set of component-child fragment spans, which is why
  it cannot occur there.

Status: **fixed upstream and pinned**. `rust/Cargo.toml` moved
`solid1-dom-expressions-compiler` from `79b9b63721c59b0acfd72348438bbb6e090ec81c`
to `ad2c9452041c757138bb972416d8abc4798ea6b9`, which carries both fixes:
`style`/`classList` decomposition now follows the same positional spread
carve-out lowering uses, and fragment children are classified from a span set
instead of a flag that leaked through attribute values. Both are census-only,
so emitted output is byte-identical across the two revisions.

The corpus confirms it: `@kobalte/core@0.13.13` and `@tanstack/ai-solid-ui`
both generate cleanly, and the whole 416-probe run contains zero
`type-facts-failure` results. Neither was a `tsc` concern in any form — this is
JSX-lowering execution-fact bookkeeping, so the absolute rule in AGENTS.md was
never implicated either way.

## Generated contracts are byte-bound only in the single-artifact case (2026-08-22)

`contract generate` used to write `artifacts: {}` unconditionally, so nothing
tied a generated contract to the bytes it describes beyond a version string —
and a version string is not a pin: republished or locally patched contents keep
the version, and the contract would still claim to describe them. The consumer
has always verified artifact hashes whenever present
(`validate_contract_artifacts` in
`rust/crates/solid-facts-backend/src/diagnostics.rs`), so the gap was entirely
on the producer side.

Generation now emits a real `artifacts.implementation` `{ path, hash }` pair —
the `hash` value carries the `sha256:` prefix — whenever schema v1 can carry it:
the contract's emitted entrypoints resolve to exactly one runtime artifact and
that file is inside the contract's own directory (the in-package default
output). Several residues remain, all recorded on the review plan as
`contract artifact binding` items rather than papered over:

- **Multi-artifact packages stay unbound.** Schema v1's `artifacts` object has
  exactly one `implementation` slot, and a package whose entrypoints resolve to
  several runtime files cannot be described by it. Hashing one of them would
  claim byte identity for a contract whose other entrypoints describe files
  nothing pins, so nothing is emitted. Closing this needs a per-entrypoint (or
  list-valued) artifact claim, which is a new schema shape rather than an
  additive field — an old schema-v1 reader must not be able to ignore a new
  sibling and read the omission as "no artifacts to check". Owner: a future
  schema revision, reviewed on its own.
- **Even a bound contract is bound at its entry artifact only.** The hash covers
  the export-map target file, while the analysis behind the summaries consumes
  that target's whole relative runtime-module closure (`runtimeModuleClosure` in
  `packages/cli/scripts/generate-package-contract.mjs`, seeded as the analysis
  roots in `analyzeTarget`). A barrel entry —
  `export { x } from "./internal.mjs"` — therefore gets a "bound" contract whose
  semantics come from a file no hash pins: patch `internal.mjs` and the entry
  bytes, and the hash with them, are unchanged. The hash is still real evidence
  about the entry file and keeps being emitted; the review plan now states the
  entry artifact is hashed and counts the closure modules that are not
  byte-bound. Closing this needs the same new schema shape as the multi-artifact
  residue — a list-valued or per-module artifact claim — so it is owned by that
  future schema revision, not by the generator.
- **Out-of-package outputs stay unbound.** A project-owned contract under
  `.solid-checker/contracts/<package>/` sits outside the package, so its
  artifact path could only be spelled with `..`, which the loader rejects by
  design. Nothing to fix here on the producer side; the review plan says the
  contract is not byte-bound, and the reviewer checks the artifact by hand.
- **No declaration artifact is ever generated.** The package generator analyzes
  runtime targets and never resolves the `types` condition, so it has read no
  declaration file whose bytes it could claim. `artifacts.declaration` remains
  available to the lower-level `--declaration-artifact` workflow, which is
  handed the exact file.

None of this is a `tsc` concern in any form: artifact identity is a trust-
boundary fact about bytes on disk, which the type system says nothing about.

## The runtime-module closure is walked, not attested (2026-08-22)

The per-entrypoint closure in a review plan's `generation.entrypoints` block is
what `contract review --transfer-from` treats as *the bytes this review was
recorded against*: an entrypoint transfers only when its recorded module paths
and sha256s are identical on both sides. That record is produced by
`packages/cli/scripts/runtime-module-closure.mjs` — a scanner and resolver in
the Node process, walking the same specifier forms TypeScript would. It is not
the file list the analyzing program actually opened.

The gap is real and was exploitable. Three shapes were silently omitted, each
producing a closure record that named fewer files than the analysis read while
claiming to name all of them:

- an ESM-spelled `./impl.js` whose checkout ships `impl.ts` (TypeScript's
  extension substitution);
- a `#`-prefixed specifier resolved through the manifest's `imports` map;
- every import below a string literal containing `/*`, because the comment
  stripper was a regular expression that knew nothing about strings.

A byte-identical barrel entry over a fully rewritten implementation therefore
transferred an entire review and promoted to `reviewed` evidence with zero
human decisions.

The walker now handles all three, and — more importantly — is fail-closed
instead of best-effort. Every static specifier form is resolved to a recorded
file, classified as carrying no runtime semantics (a declaration file), or
classified as external (a bare specifier, which the package-contract boundary
owns and no closure hash could pin). Anything else adds a `notes` entry to that
entrypoint's closure record, and a note makes the entrypoint non-transferable
and surfaces on the review plan's `contract artifact binding` section. A dynamic
`import()` with a non-literal specifier is noted for the same reason.

**The residue**: a syntax walk can still disagree with the compiler in ways
neither side reports — a `paths` mapping, a resolution the bundler condition
resolves differently, a specifier form the scanner classifies as external that
the analyzed program in fact opened. Nothing in this process can observe that,
because the process that resolved the modules is the other one. The exact fix is
a TypeFacts protocol addition: the analyzing program already knows its own file
list, and emitting it would turn the closure from a reconstruction into an
attestation. Until that exists, unresolvable specifiers fail closed via notes
and the record stays a generator-side claim.

Two adjacent facts belong with it. `contract generate --missing` writes
project-owned contracts under `.solid-checker/contracts/<package>/`, which are
outside the package by construction and therefore never byte-bound at the loader
(see the out-of-package residue above) — so the artifact-binding residue is the
*default* shape for project-owned contracts, not an edge case, and the review
plan's binding section is the only thing standing in for a hash there. And the
closure record is not evidence the loader reads: it lives in the review plan, and
nothing outside `contract review` consults it.

`tsc` has nothing to say about any of this. Which files a contract's summaries
were derived from is a provenance fact about a generation run, not a typing
question.

## The ecosystem benchmark counted partial contracts as successes (2026-08-22)

`scripts/ecosystem-benchmark/` classified any exit-0 generation as `success`,
including a contract that refused entrypoints and said so on stdout. The
checked-in `benchmarks/ecosystem/report.md` therefore reports the Official
Solid family under Solid 1.x as "Declared entrypoints: 44 / Generated
entrypoints: 28 / Success: 6/6 (100%)", with no field anywhere in the report
that could attribute the gap. Classification now has a `partial-success` class
and a matching probe outcome, so `success` means a complete contract, and the
refused entrypoints are counted, summed per family, and listed by package.

Adding the third outcome also left every *comparison* in the report still
written as a two-valued test. `buildBaselineComparison` and `buildFloorHeadDiffs`
in `scripts/ecosystem-benchmark/lib/report.mjs` asked "was it `success`, is it
`success`", so a probe going `partial-success` → `failure` — the run where the
contract disappeared entirely — matched neither regression nor fix, and the
symmetric `failure` → `partial-success` gain matched neither either. Both now
compare direction on the ordered scale `success > partial-success > failure`,
carry both outcomes on each entry, and render the transition rather than a
hardcoded destination. The floor/head headings are named for direction
("Worse/Better at head than at floor") for the same reason.

**Regenerated 2026-08-22.** The checked-in reports now carry the split. On the
305-row/416-probe manifest with the release binary, the full corpus is
**403 complete contracts, 6 partial, 7 failures**, against **409 successes and
7 failures** on the same manifest before the split. The failure set is
unchanged package-for-package and class-for-class, and all 6 partials are
former successes (`@kobalte/core`, `@tanstack/charts`,
`@tanstack/solid-pacer`, `@tanstack/solid-router`, and `solid-js@2.0.0-rc.1`
on both floor and head): `409 = 403 + 6`. The typed generation-refusal change
moved no probe into a failure class. The sentinel subset moves the same way —
23 probes, 20 complete, 2 partial, 1 failure — and now runs against the same
manifest as the full report instead of an older 417-probe one.

The measurement also refutes half of the prediction above. The Official Solid
"44 declared / 28 generated" gap is **not** refusals: that family records zero
refused entrypoints while still generating 11 of `solid-js`'s 23 declared
entrypoints and 2 of `@solidjs/image`'s 5, all classified `success`. Declared
counts include export-map branches for which the generator emits no contract
entrypoint at all, and no field attributes that. It is recorded as unmeasured
in docs/ecosystem-benchmark.md rather than as closed by this class.

## Closed 2026-08-22: schema-valid callback argument claims are never dropped

Contract callback rows may carry `arguments` descriptors — "this helper hands
your callback a reactive value at parameter N". Source discovery materializes
one shape only: an inline function literal whose span *is* the call-site
argument, carrying an `accessor` descriptor. Every other schema-valid shape —
a callback passed by name, or a `store-path`/`tuple`/`object`/`argument`
descriptor — was dropped in silence, so the callback body was analyzed as if
the contract had said nothing about its arguments. Reads through the callback's
parameters then looked like ordinary data: fail-open, and no gate could see it,
because no checked-in contract uses `arguments` yet.

The consumer now keeps those call sites demand-sensitive, through the same
per-export SC9005 path the unknown-callback domains use
(`rust/crates/solid-reactive-ir/src/interproc.rs`). Precision is unchanged
where the claim binds: an inline literal carrying only `accessor` descriptors
still materializes the accessor and reports nothing. A descriptor beyond the
literal's declared parameters is not a gap either — but only when the literal is
a *restless arrow*, the one shape that provably cannot name the argument. A
non-arrow function expression reads the slot as `arguments[N]`, and a rest
parameter — which `FunctionFact.parameters` deliberately excludes, because it
has no single argument index — absorbs every argument from its index onward;
`mapPath((...args) => args[0].value)` was silently clean before that fact was
carried (`FunctionFact.rest_parameter`, added in
`rust/crates/solid-facts/src/ast/mod.rs`).
`fixtures/reactive-ir/package-callback-arguments-consumer/` pins all six
outcomes.

**Remaining approximation.** The non-`accessor` descriptor kinds are *reported*
rather than *modeled*: a `store-path` argument handed to a callback is a real,
expressible claim that the consumer could materialize as a store source. Until
it does, such a contract makes the call uncertifiable instead of certified.
That is fail-closed and honest, but it is a claim shape the schema allows and
the consumer does not yet use.

## Closed 2026-08-22: contract and dispatch obligations no longer suppress each other

`PackageContractExportMissing` consumer obligations and genuine
`ReactiveDispatchUnresolved` findings travel in one vector — they are found by
the same interprocedural walk — and both were deduplicated under the single
identity `reactive-dispatch-unresolved`. The dedup key is
`(identity, path, start_byte)`, so an SC9005 and an SC9012 that merely started
at the same byte silently suppressed one another, and which one survived
depended on push order. The identity now follows the defect kind
(`rust/crates/solid-reactive-ir/src/reactive_analysis.rs`), pinned by
`reactive_analysis::tests::contract_and_dispatch_obligations_do_not_deduplicate_each_other`.
No checked-in snapshot moved: no current fixture produces the colliding pair.

## Closed 2026-08-22: explicit contracts cannot bypass version classification

An explicit `--contract` file was version-classified only when its package
appeared in the *import-derived* module set. Contract resolution also applies a
contract to `export … from "pkg"` re-exports, which never contribute to that
set, so a stale explicit contract could be applied to a package the project
only re-exports. Classification is now unconditional
(`rust/crates/solid-facts-backend/src/diagnostics.rs`); a package that is not
installed has no manifest to disagree with, so an explicit contract for it
still applies exactly as before. Pinned by
`diagnostics::tests::explicit_contracts_are_version_checked_without_an_import`.

**Remaining gap.** `package_contract_statuses_with` still enumerates
`imported_package_roots`, so a re-export-only package with a refused contract
is fail-closed in analysis but invisible in `--check-contracts`. Closing that
means broadening the module set the report walks, which changes what the
report claims about every tier, not just the explicit one.

## Entrypoint conditions are alternatives; only the host target is scope (2026-08-22)

`RuntimeEnvironment::matches_entrypoint_conditions` combines an entrypoint's
recorded conditions with membership, not containment, and that is correct for
how contracts are generated: the list is the union of the export-map branches
the entrypoint resolves through. The bundled `solid-js` root entrypoint records
`browser, deno, development, import, node, worker` — no environment satisfies
all of it at once, and requiring containment would make the contract this
checker ships unmatchable. Pinned by
`entrypoint_conditions_are_alternatives_except_for_the_host_target`.

`--conditions` generation writes the *asserted scope* into the same union
field, where the list is not alternatives. The host target is the one dimension
where the two are distinguishable — at most one of `browser`/`node`/`deno`/
`worker` describes any environment — so an entrypoint naming host targets and
not the consumer's now fails closed rather than matching through a shared
resolver condition such as `import`. Recording `default` keeps it open, since
the unconditional branch really is reachable everywhere.

**Remaining approximation.** The other exclusive dimensions (`development` vs
`production`, the rendering modes) cannot be tightened the same way: real
export maps record only `development` as a branch, and a production consumer
legitimately resolves such an entrypoint through its fallback. A
`--conditions production` contract therefore still records its build scope into
a field a development consumer can match. Closing this needs schema v1 to
distinguish "branches observed" from "environment asserted" at the entrypoint
level, which it cannot express today. A conservative alternative that also
fails closed for the non-target case would produce false uncertifiable results
against every checked-in contract, so it is not the smaller evil.

## Spread arguments to parameter-member reads are reported at the spread (2026-08-22)

`argument_proves_non_reactive` treats an array/object literal as proven plain
data, spread included, and that was audited rather than assumed:
`drop([...storeArray])` copies out of the proxy at the call site, so the callee
really does receive snapshot data and its `parameter-member` claim proves
nothing about reactivity. The read that exists is the spread, and the spread
pass in `local_access.rs` already reports it in its own execution role —
`fixtures/reactive-ir/package-parameter-member-consumer` `SpreadArgument`
produces exactly one `SC1001` for `"state spread"` and no `SC9012`. Adding the
obligation as well would report one dependency twice.

**Remaining gap.** The copy is shallow, so a nested proxy surviving it
(`drop({ ...store }).nested.value`) is a second dependency that neither the
spread read nor the parameter-member claim describes. Closing it needs the
consumer to track proxy identity through a literal's element/property
positions, which no fact table carries today.

## Closed 2026-08-22: contracts are enforced against the lockfile integrity, where one exists

`package.integrity` — the npm sha512 of the tarball a contract was audited
against — was format-validated on load
(`rust/crates/solid-reactive-ir/src/lib.rs`) and then compared to nothing. A
published or project-owned contract bound to nothing but a version string, and
a version string is not a pin: a republished tarball, an `npm overrides` entry,
and a locally patched install all keep the version while replacing the bytes
the summaries describe. Every bundled contract carries an integrity, so this
was the strongest available identity fact going unused.

Loading now recovers the installed copy's integrity from the project's npm
lockfile and refuses a disagreeing contract exactly as it refuses a stale one:
status `stale`, an uncertifiable `SC9005` at the import, the run continues. The
message and the report `detail` name **both integrities**, because the versions
agree and naming them would read as a contradiction. Bundled and project-owned
contracts get their existing, different remedies, reworded for the case where
the audited version is already the installed one.

The integrity comes from `package-lock.json` or `node_modules/.package-lock.json`
at `lockfileVersion` 2 or 3, whose `packages` map is keyed by *install path* and
so names the specific installed copy rather than a package name. Pinned by
`cli_refuses_a_contract_whose_lockfile_integrity_moved_under_the_same_version`
(process) and `lockfile_integrity_is_recovered_only_when_it_is_unambiguous`
(unit).

**Remaining approximation, deliberately fail-open on the fact and fail-closed
on the verdict.** Enforcement needs both halves — an integrity in the contract
and a recoverable one on disk — and every way the second half is unavailable
yields *no fact*, which means the previous behavior (version matching alone),
never a refusal:

- **No npm lockfile.** pnpm and Yarn keep their own formats, and many projects
  have no lock at all. Reading them is tractable but each is a separate format
  with its own store layout and its own path-to-entry question; none of it can
  be guessed from the npm shape. Owner: one format at a time, each with its own
  fixture.
- **`lockfileVersion` 1.** Its tree is keyed by package name, so resolving an
  entry to *which* installed copy it describes under hoisting would be exactly
  the guess this must not make.
- **Link, workspace, `file:`, and git entries** have no registry tarball and so
  no integrity. A linked workspace package's bytes are unpinnable by
  construction; closing this needs a content hash of the linked directory,
  which is a different mechanism from npm integrity.
- **Two lockfiles disagreeing about the same installed directory.** Which one
  describes the bytes on disk is not answerable from the files, so no
  enforcement happens rather than a coin flip in either direction.
- **An unparseable lockfile** is the project's own file, not a malformed
  contract, so it never fails the run.

A contract with no `package.integrity` is unaffected in every case.

## Open: package contracts are bound to a module *name*, not a resolved module

Contract discovery and contract application both key on the import specifier's
package root and nothing else. `discover_package_directory`
(`rust/crates/solid-facts-backend/src/diagnostics.rs`) walks ancestors for
`node_modules/<name>`, and `PackageContract::for_module` — the only gate in
`resolve_contract_imports` (`rust/crates/solid-reactive-ir/src/contracts.rs`) —
compares `contract.package.name` against `import.module`'s root. Neither asks
where the specifier actually resolves.

**The failing scenario.** A tsconfig `paths` entry maps
`"reactive-package": ["src/local-impl"]`, while `node_modules/reactive-package`
is still installed (a common shape: a local reimplementation, a fork under
development, a test double). The published or project-owned contract for the
installed package is discovered by name, passes name and version
classification, and is applied to imports that resolve to project source that
the contract never described. Its summaries then drive reactive-read,
callback-timing, and owner-requirement conclusions about code the contract's
author never saw — a false certification, not merely a missed one. A workspace
`link:` is *not* an instance of this: `discover_package_directory` follows the
symlink and reads the linked package's own manifest, so name and version are
classified against the package that is really there.

**Investigated 2026-08-22: no narrow safe check exists with today's facts.**
The obvious verification — the imported module's resolved declaration must sit
inside the discovered package directory — fails on the facts the backend has:

- **`ImportFact` carries only the specifier text** (`module: CompactString` in
  `rust/crates/solid-facts/src/ast/mod.rs`). There is no resolved module path
  anywhere in the fact tables.
- **Declaration paths exist but are the wrong evidence.** `Declaration.location`
  does carry a path, but `alias_roots_and_source_declarations`
  (`rust/crates/solid-reactive-ir/src/symbols.rs`) deliberately *skips* `.d.ts`
  declarations, which are exactly the ones that would locate an external
  package. Reading them instead would then be wrong in three routine cases: a
  package typed through `@types/<name>` declares into
  `node_modules/@types/<name>` and would fail containment; a pnpm or
  workspace-symlinked install can report the realpath
  (`.pnpm/<name>@<version>/node_modules/<name>`) while discovery returns the
  link path; and an untyped JavaScript package has no declaration inside the
  package at all, which is precisely where a contract matters most. Each of
  those would turn a correct contract into a false `SC9005`.
- **The package directory is not on the IR side of the seam.** Only the backend
  computes it; `PackageContract` carries `source_path`, which locates the
  package directory for a *published* contract and not for a bundled, local, or
  explicit one. Threading it through would be new data across the fact
  interface, for all four tiers.
- **`paths` itself is not read anywhere.** Detecting the alias directly would
  mean parsing `compilerOptions.paths` with its `extends` chain, `baseUrl`, and
  wildcard patterns — a new fact source — and would still be ambiguous, because
  TypeScript falls back to `node_modules` resolution when a mapped path does
  not exist.

**What closing it needs.** One fact the producer already computes and does not
forward: the resolved module file for each import specifier, from the same
TypeScript resolution the checker's type facts come from. With that, contract
application can require the resolved file to lie inside the package directory
the contract was classified against, fail closed when it does not, and stay
silent (no fact, current behavior) when resolution is unavailable. That is a
Type Facts protocol addition and a `PackageContract` provenance field, and it
should be designed as one change rather than approximated by a path heuristic.
Half-implementing it — a containment check on declaration paths — would trade a
rare false certification for a routine false uncertifiable result on every
`@types`-typed and pnpm-installed package.

## Open: contracts have no distribution mechanism beyond four local tiers

A contract reaches a project through exactly four channels: this checker's own
**bundled** artifacts, a **published** `solid-reactivity.json` inside the
installed package, a **local** file under `.solid-checker/contracts/`, and an
**explicit** `--contract` path. There is no fifth. There is no registry, no
fetch, no shared corpus of reviewed contracts, and no way for one team's review
work to reach another project.

The consequence is the whole many-packages user story. A project importing a
dozen Solid-aware packages that this checker does not bundle has one path
available: generate a draft for each with `contract generate` and review each by
hand. Generation never promotes `inferred` evidence, so until that review
happens every one of those packages reports `unverified` and certifies nothing.
The ecosystem benchmark measures the generator against real packages, but its
output is not a corpus anyone can install — the reviewed contract for a popular
package has nowhere to live except inside the package or inside one project.

**A design now exists**: [rfcs/0001-contract-registry.md](rfcs/0001-contract-registry.md)
specifies a signed, content-addressed registry of reviewed contracts and one new
explicit command, `contract fetch`, that resolves against the installed artifact
and materializes the contract into the existing local tier for the consumer to
commit. It adds no discovery tier, no precedence rule, and no analysis-time
network access.

The two candidate directions recorded earlier — a registry, and shipping a
reviewed ecosystem corpus as additional bundled contracts — turn out to
**compose rather than compete**. The registry is where reviews live and are
governed; bundling is a release-time snapshot of its most-imported entries, for
zero-configuration coverage. The RFC's §8 covers the one hazard that creates: a
fetched contract lands in the local tier and would otherwise shadow a bundled
audited artifact.

What remains open is the implementation — none of `contract fetch`,
`contracts-lock.json`, the entry/index specification, signatures, or the
revocation path exists — plus the RFC's own unresolved questions, of which the
load-bearing ones are the trust-set bootstrap, reviewer key rotation and
revocation, whether verifier identity can be recorded in a schema-v1 contract at
all (the loader's unknown-field failure is the outright-malformed path, so the
field would hard-fail older clients). The artifact-keyed review transfer the RFC
named as a hard dependency is no longer open: `contract review --transfer-from`
carries a previous review's resolutions onto a regenerated contract for every
entrypoint whose runtime-module closure is byte-identical, so an upstream release
costs a re-review of the diff rather than of the package.

## How much of a real ecosystem contract is actually a claim (measured 2026-08-22)

> **Superseded 2026-08-23 for the unknown-claim figures.** The all-five
> whole-summary shape this section identifies as the dominant cause was a defect
> in the emitter's attribution, not a limit of the analysis. It is fixed; the
> re-measured numbers are in "[Closed 2026-08-23: the whole-summary unknown
> collapse](#closed-2026-08-23-the-whole-summary-unknown-collapse)" below. The
> per-family reasoning and the closure-note conclusions here still hold.

The ecosystem benchmark measured generation *reachability* only — whether a
contract was emitted — and "[Open: generation success is not contract
correctness](#open-generation-success-is-not-contract-correctness)" already
records that a 98% success rate says nothing about what the emitted documents
contain. A machine-verification scheme asks the question in between those two:
under a scheme where an unknown stays uncertifiable, **how clean is a typical
package's generated draft before anyone reviews it?**

That is now measured. `scripts/ecosystem-benchmark/lib/contract-content.mjs`
opens every emitted `solid-reactivity.json` and its sibling `.review.json`
before the probe's temporary directory is removed, and counts unknown claims by
domain, refused entrypoints, closure notes, and positive behavioral rows. The
outcome classes are untouched: the same 305-row/416-probe manifest, the same
403 complete / 6 partial / 7 failures, class-for-class identical to the previous
run (release binary, 600 s budget, 95.413 s wall). Full method and caveats in
[ecosystem-benchmark.md](ecosystem-benchmark.md#contract-content-how-much-of-an-emitted-contract-is-actually-a-claim).

**Headline, over the 409 probes that produced a contract (207 packages):**

- **300 / 409 probes (73.35%) are fully proven** — no unknown claim, no refused
  entrypoint, no closure note.
- **126 / 207 packages (60.87%)** are fully proven across every one of their
  probes.
- **5,415 / 8,113 exports (66.74%) are proven.** 2,698 carry an unknown.
- 11,013 unknown claims in total, but **2,077 of the 2,698 unknown exports are
  unknown in all five domains at once** — most of each domain column is the same
  exports counted five times.
- 7 entrypoints refused across 6 probes; 32 closure notes across 7 probes.
- Positive behavioral rows available to a future probe step: 1,636 callback
  executions, 1,200 return trees, 990 reactive reads, 275 owner requirements,
  98 async behaviors.

**Per-family highlights:**

- **Solid Primitives is genuinely clean, and it is most of the corpus.** 288 of
  the 409 contracts; **230 fully proven (79.86%)**, 88.37% of exports proven,
  zero refusals, zero closure notes. Corvu is comparable on a smaller base
  (23/28, 82.14%). The small-single-purpose-package shape is what the generator
  handles well, and it is also the shape most of the ecosystem actually is.
- **The dominant unknown cause is one summary shape, not one claim domain.** A
  function export the generator reaches but cannot analyze is emitted with all
  five domains as `{"status":"unknown"}`, and that single summary is then shared
  by every export matching it. `@kobalte/core@0.13.13` emits exactly one such
  summary and attaches it to 452 of its 610 export names — 2,260 of the corpus's
  11,013 unknown claims from one summary. `solid-recharts` (305 of 327 exports),
  `motion-solidjs` (329), `@tanstack/solid-db`, `@tanstack/solid-table` and
  `@solidjs/router` are the same shape. The one large exception is
  `@solidjs/web@2.0.0-rc.1`: 188 unknowns, all `reactiveReads`, the other four
  domains fully claimed.
- **TanStack's unknowns are NOT its options-object callback pattern.** This was
  the expected answer and the data refuses it: 318 of TanStack's 322 unknown
  exports are the all-five whole-summary shape, and only 3 exports in the entire
  family carry a `callbacks`-only unknown. Hand-checked against two real
  contracts: `@tanstack/solid-query@5.101.4` on `solid-js@1.9.14` emits 57
  exports with exactly 3 unknowns (`useQuery`, `useInfiniteQuery`,
  `replaceEqualDeep`, all `callbacks`), while `@tanstack/solid-query@6.0.0-rc.0`
  on `solid-js@2.0.0-rc.1` emits 57 exports of which 37 are unknown in all five.
  Both declare the same non-standard `"@tanstack/custom-condition":
  "./src/index.ts"` branch pointing at TypeScript source; in 5.x that branch
  still yields real `reactiveReads` rows, in the 6.x prerelease it yields the
  whole-summary sentinel. The family's own numbers are unremarkable once that
  shape is set aside: 33/50 contracts fully proven, 84.84% of exports proven.
- **Official Solid is the worst-looking family (6/23 fully proven) for a reason
  that is not unknowns**: it owns 29 of the corpus's 32 closure notes. Its
  contracts largely make claims; they just cannot be bound to the bytes they
  describe.

**What this implies for machine-verified contracts**
([rfcs/0002-machine-verified-contracts.md](rfcs/0002-machine-verified-contracts.md),
forthcoming):

- A scheme that keeps unknowns uncertifiable does **not** start from a blank
  page. Three quarters of generated contracts already carry no unknown at all,
  and two thirds of all exports are claimed — the verification surface is real
  work, not an empty set.
- The work is extremely unevenly distributed. Roughly ten package/target pairs
  produce most of the corpus's unknown claims, and each of them concentrates in
  a single all-five summary. Closing that one shape — not five separate domain
  analyses — is what would move the number.
- **Closure notes, not unknowns, are the harder blocker.** An unknown is an
  honest uncertifiable result a consumer can route around; a closure note means
  the contract cannot be byte-attested at all, so no amount of verification
  binds it to an artifact. 7 probes and 32 notes today, 29 of them in Official
  Solid. See "[The runtime-module closure is walked, not
  attested](#the-runtime-module-closure-is-walked-not-attested-2026-08-22)".
- These figures are the **demand-insensitive upper bound on the work**, and
  should never be quoted as a defect rate. An unknown becomes a finding only
  when a consumer touches that surface; a package with 452 unknown exports costs
  a project nothing if it imports two proven ones. The benchmark has no demand
  model, so the number of unknowns a real project would actually hit is
  unmeasured and is almost certainly far smaller.
- **"Proven" here means "claimed", not "verified".** Every claim counted as
  proven is still `inferred` evidence below the SC9005 trust ceiling. A contract
  asserting that `map` never invokes its callback is counted fully proven by
  this measurement and is false — which is exactly the gap RFC 0002 exists to
  close, and exactly why this measurement is a floor on the verification work
  rather than an estimate of it.
- **Probe drivability is not measured.** The 4,199 positive behavioral rows are
  what a probe step would have to drive; no attempt was made to drive any of
  them, so how many are actually executable is the next open question.

## Closed 2026-08-23: the whole-summary unknown collapse

An unresolved proof obligation used to erase claims the analysis had already
proven, on exports that could not reach it. Two independent defects in
`emit_package_contract`
(`rust/crates/solid-facts-backend/src/main.rs`) compounded:

- **Every domain.** `ReactiveDispatchUnresolved` fell through
  `unresolved_claim_domains`' catch-all to all five claim domains. The
  obligation proves that the possible runtime implementations of a dispatch do
  not share one *reactive-read* summary. It says nothing about the callbacks the
  export invokes, its owner requirements, or its async behavior.
- **Every export.** Attribution read only the *innermost* function containing
  the obligation. An obligation inside an anonymous arrow, a named local helper,
  or a private cross-file helper matched no export, and the fallback marked
  every export of the entrypoint — including exports with no path to it at all.
  A third rung scanned every call in the project whose callee *text* equalled a
  missing contract export's name, or ended in `.` plus that name, which is the
  name-only matching the precision contract forbids.

**What replaced them.** `ReactiveDispatchUnresolved` now marks
`reactiveReads` and `returns` only. Attribution is a ladder — `joined`,
`enclosing-chain`, `identity-widening`, `reachability`, `fallback-all` — spelled
out in
[package-contracts.md](package-contracts.md#which-exports-an-unresolved-obligation-belongs-to).
The name-text scan is gone; the reachability rung asks the call graph, in
`rust/crates/solid-reactive-ir/src/attribution.rs`, and is used only when the
enumeration is provably complete.

**Why `returns` and not `reactiveReads` alone.** The returns description is
derived from the same resolved callee summary the dispatch invalidates, and it
does **not** fail closed on its own: a value produced by an unresolved dispatch
and placed in a returned object is described from the local accessor index,
which knows nothing about it, so a possibly-reactive property is published as a
certified-negative omission. `StructuredReturnUnresolved` is not the guard — it
fires only for a shorthand property bound to an import with no project
declaration, an orthogonal condition. The
`unresolved-dispatch-domains-control` fixture is the proof: with the dispatch
resolved the generator claims `returns.properties.value` is an accessor, which
is exactly the claim the unresolved variant cannot make. `callbacks`,
`ownerRequirements` and `asyncBehavior` are proven by passes that never consult
the dispatch and are kept.

**A third defect the fix exposed.** With the collapse gone, two conditional
branches that each *prove* a different `returns` stopped being merge-compatible,
and `mergeSummaries` refused the whole entrypoint over it. solid-js 1.9.14's
`Show` is that shape — it returns its `props` argument in the server build and a
memo accessor in the client build — and refusing discarded the other 147 exports
of its `.` entrypoint. The base now carries the unknown sentinel for the
divergent domain and the exact per-branch behavior is emitted as `variants`,
which is the same discipline the function already applied when either side was
unknown.

**Measured recovery** (full ecosystem benchmark, release binary, 600 s budget,
same 305-row / 416-probe manifest; before = the 2026-08-22 run recorded above).

> **The `after` column was measured before the soundness fixes below, and has
> been re-measured.** The adversarial review recorded in "[Closed
> 2026-08-23: under-marking in the attribution
> ladder](#closed-2026-08-23-under-marking-in-the-attribution-ladder)" found
> six ways an export whose behavior depends on an unresolved obligation was
> published with the domain omitted — an arrow-bound export invisible to every
> rung, an escape test that never saw an escape, a name-text join, a blanket
> discharge, and two false callback rows. Every one of them *lowered* the
> unknown-claim counts and *raised* the "exports proven" figure by certifying
> something that was not proven. The improvement recorded here is real in
> direction; part of its magnitude was that inflation. The `corrected` column
> is the current state and the one to compare future work against.

| | before | after (superseded) | corrected |
| --- | --- | --- | --- |
| Probes fully proven | 300/409 (73.35%) | 304/409 (74.33%) | **288/409 (70.42%)** |
| Packages fully proven | 126/207 (60.87%) | 128/207 (61.84%) | **111/207 (53.62%)** |
| Exports proven | 5,415/8,113 (66.74%) | 6,520/8,320 (78.37%) | **6,095/8,358 (72.92%)** |
| Exports unknown in ALL five domains | 2,077 | 492 | **527** |
| Unknown claims, total | 11,013 | 4,898 | **5,903** |
| Probes with at least one unknown claim | 102 | 99 | **116** |
| Entrypoints emitted / refused | 847 / 7 | 850 / 4 | **850 / 4** |
| Positive behavioral rows | 4,199 | 5,545 | **5,005** |

Outcome classes moved once and have not moved since: 403 success / 6 partial /
7 failure became **406 / 3 / 7**, and the corrected run is package-for-package
identical to that. The three probes that moved (`@kobalte/core@0.13.13`, both
`solid-js@2.0.0-rc.1` probes) are entrypoints the conditional-merge refusal
used to discard and now emits. No probe has regressed in any of the three runs.
The export total grew from 8,113 to 8,320 because those three entrypoints now
contribute exports, and to 8,358 when the declaration-sibling gate changed
which modules an entrypoint enumerates.

The `after` → `corrected` movement was attributed by re-running the full corpus
twice against the current binary, once with and once without the
conditional-merge one-sided fix: the engine soundness rounds account for 15
fully-proven probes and 316 proven exports across 48 probes, and the merge fix
for 1 probe and exactly 109 exports (108 `returns`, 1 `asyncBehavior`) across 8.
The per-cause table is in
[ecosystem-benchmark.md](ecosystem-benchmark.md#headline-numbers-2026-08-23-third-measurement-state-release-binary-416-probes).

**Fixtures.** `fixtures/package-contracts/unresolved-dispatch-attribution` pins
the `joined` and `enclosing-chain` rungs and the surviving `callbacks` claim,
with a sibling export that must stay fully proven;
`unresolved-dispatch-domains-control` pins the claim the unknown replaces;
`unresolved-dispatch-reachability` pins the call-graph rung across files;
`unresolved-contract-export-attribution` pins that a missing contract export
still keeps all five domains, for that export only, through exact symbol
identity rather than a name scan. All four are in the
`scripts/contract-corpus.mjs` pin list.
`unknown_claim_attribution_markers_reach_the_review_plan` in
`rust/crates/solid-facts-backend/tests/contracts_process.rs` pins the stderr
marker seam on real bytes from both processes.

**Still fail-closed after this.**

- The `fallback-all` rung survives. An obligation whose containing function
  cannot be identified at all still marks every function export. It is now
  observable — the review plan says `fallback-all` — rather than silent.
- ~~The reachability rung is conservative in its escape test~~ — **false when
  written; corrected below.** The test accepted any reference inside an
  `ExportFact.span`, which for a declaration export covers the whole body, so
  none of the three escapes it claimed to catch were caught.
- ~~The `export_names_for_function` join reads `function.name` or
  `method_name`~~ — **the consequence stated here was wrong, and in the unsound
  direction.** An arrow export did not "reach a lower rung"; it made the
  reachability enumeration return an empty set, and nothing was marked.
  Corrected below.
- `ownerRequirements` is kept claimed across an unresolved dispatch. An
  implementation the analysis cannot select could call an owner-requiring
  primitive. That is the same gap every uncontracted external call already has,
  and narrowing it is a separate question from this one.

## Closed 2026-08-23: under-marking in the attribution ladder

An adversarial review of the entry above, driven by hand-written packages
rather than by the fixture corpus, found that the ladder's *fail-closed*
guarantees were not guarantees. Six shapes published an export whose behavior
depends on an unresolved obligation with the affected domain simply **omitted**
— a certified negative. Over-marking is imprecise; this direction is unsound,
and every fix below moves toward failing closed.

Each was reproduced against the debug binary before and after, and each has a
regression fixture in `scripts/contract-corpus.mjs`'s pin list.

| Shape | Before | After | Fixture |
| --- | --- | --- | --- |
| `export const X = props => Panel(props)` reaching a private helper | nothing marked | `X` marked | `arrow-export-attribution` |
| Private helper handed to a callee (`apply(Panel, …)`) | only the *caller* marked; the escaping export certified | every export marked (`fallback-all`) | `escaping-private-helper` (`./argument`) |
| Private helper returned (`return Panel`) | same | same | `escaping-private-helper` (`./returned`) |
| Private component rendered (`<Panel/>`) | nothing marked at all | every export marked | `escaping-private-helper` (`./rendered`) |
| Private `Render` beside an unrelated exported `Render` | the *unrelated* export marked; the reaching one certified | the reaching export marked, the unrelated one clean | `export-identity-join` |
| `export { Panel, Panel as Root }` | `Panel` marked, `Root` certified | both marked | `export-identity-join` |
| Export forwarding into an exported parameter-member helper | discharged wholesale; `reactiveReads` omitted | discharged only where the row is published | `parameter-member-forwarded` |
| Callback invoked behind a closure handed to a helper, or behind a returned closure | `execution: "inline"` | no row; `callbacks` sentinel | `callback-execution-boundary` |

**What replaced them.**

- `export_names_for_function` names a declaration through
  `solid_reactive_ir::function_binding_name`, the same helper the IR uses, so
  arrow bindings resolve. It now distinguishes *undecidable* (`None`) from
  *decided: private* (`Some(vec![])`), and the reachability rung propagates the
  first instead of reading it as the second. `Some(vec![])` is itself claimed
  only when every export of the entrypoint joined to an identity or a symbol.
- The escape test accepts an export **specifier** span, never containment in an
  `ExportNamedDeclaration`'s span.
- The name-text branch is deleted. It survives only in the whole-project mode
  with no entry file, where `exports` is keyed by the project-wide export name
  and no identity channel exists.
- The call graph answers for an obligation filed *at* a declaration span, not
  only for one inside a body. Without that, every exported-helper obligation
  went to `fallback-all`.
- The `exported-parameter-member-dispatch` string comparison is replaced by
  `parameter_member_row_covers`, which asks whether the exports the ladder
  resolved the obligation to actually publish the `parameter-member` row. The
  covering channel is real (`parameter-member-read` /
  `package-parameter-member-consumer` pin it) but does not survive a hop.
- A zero-export decision emits its marker, and
  `generate-package-contract.mjs` renders it as a review-plan note. Silence was
  how a truncated reach enumeration looked from the outside.

**`ReactiveSourceUncaptured` now invalidates `returns` as well** (R7). The
reads-only claim was never tested: every shape that reaches the arm during
generation also raises the package's missing-contract-export obligation, which
erases all five domains, so the narrower claim was masked rather than proven.
`fixtures/package-contracts/uncaptured-source-return` records that, and the arm
fails closed by construction rather than by proof. Reads-only can be restored
only by a shape that fires the arm *alone*, which nobody has constructed.

**Still fail-closed, or still wrong, after this.**

- **A sibling `.d.ts` for an internal module truncates the reach enumeration
  (under-marking).** *Closed as unsound; see the 2026-08-23 entry below — it is
  now a widening, not a certified negative.* With `channel.d.ts` beside
  `channel.js`, the caller edge from `index.js` into `channelFor` is lost: the
  graph reports `complete` while having enumerated only the helper itself, and
  the obligation attributed to no export. Repro:
  `fixtures/package-contracts/parameter-member-forwarded` with a `channel.d.ts`
  added — `forwarded` went from unknown to certified.
- **A provably unused callback parameter still opens the sentinel instead of
  emitting the honest negative** (the remainder of R6). In
  `callback-execution-boundary`, `schedule` never uses its second parameter, so
  the truthful answer for `Escaping` is *no callback row*. Proving it needs an
  interprocedural "this parameter is never invoked" summary the generator does
  not compute; the fail-closed sentinel is what is emitted instead.
- ~~**A re-exported helper called from the same entry file joins to nothing.**~~
  *Not reproducible against this entry's own code.* Re-tested 2026-08-23 in all
  four spellings — `export { x } from "./m.js"` before or after
  `import { x } from "./m.js"`, `import` then bare `export { x }`, and
  `export * from "./m.js"` — and every one resolves the obligation to both
  published names. The identity join in this entry closed it; the residual was
  recorded from a run that predated it. It is pinned now by
  `fixtures/package-contracts/entry-reexport-identity`, so it cannot silently
  come back. The shape that *did* still fail was the same source with a
  `channel.d.ts` sibling, which is the `.d.ts` class above and not a second one.
- `fallback-all` survives, and the three escape shapes above now reach it. That
  is deliberate: nothing in the package proves the escaped helper is
  unreachable from a sibling export's caller.
- The `runtime_execution` rung can still return `inline` for a call nested
  inside a proven-inline scheduler that is itself inside an unproven closure.
  The fold looks at the enclosing argument chain, not at the schedule of the
  outermost call in it. Not observed in a repro; not narrowed here.

## Open: nominal class-method dispatch could discharge these obligations (2026-08-23)

The obligations the entry above learned to *attribute* precisely are mostly
obligations that should not exist. The dominant real-world shape, from the
ecosystem benchmark's own samples, is a parameter typed as a class or interface
whose method is then invoked: `getQueryCache()` on a `QueryClient` parameter,
`.toLowerCase()` on a `string`. `member_value_symbols_at` finds no *value*
implementation for a nominal type's method, so the dispatch is unresolved and
the enclosing export loses its reactive-read claim — even though the callee's
own resolved declaration names exactly one method body.

Resolving the method through the callee's declaration would discharge the
obligation outright rather than re-attributing it, and it composes with the
attribution ladder: fewer obligations reach the ladder at all, and the ones that
do are the genuinely ambiguous ones.

It is **not** implemented, because it needs a soundness argument this change did
not attempt:

- **Subclass existence reopens it.** A parameter typed `Base` can receive a
  `Derived` that overrides the method. Selecting `Base`'s body then certifies an
  implementation that does not run. The argument has to be closed over the
  analyzed program — an override declared anywhere in the closure must reopen
  the obligation — and "closure" here is the package plus its consumers, which
  for a published package is open by construction. `--program-boundary closed`
  is the existing lever, and whether it is enough is exactly the open question.
- **The standard library is the easy half.** `.toLowerCase()` on a `string` has
  no user-declarable override, and Type Facts already marks standard-library
  declarations (`resolved_callee_call(..).declaration.standard_library`). That
  subset may be dischargeable without the subclass argument at all.
- **A Type Facts signal may be required.** Deciding "this method has exactly one
  implementation reachable at this call site" is a type-system question, not an
  AST one, and the current fact set does not answer it.

## Open: probe discovery contradicts bundled Solid negative claims (2026-08-23)

**`solid-v2/solid-js` is resolved (2026-08-23); `solid-v2/@solidjs/web` and
`solid-v1/solid-js` are not.** The resolution is recorded at the end of this
entry, together with the exact worklist the two remaining contracts still carry.

The Stage-1 probe driver (`contract probe`, RFC 0002), run with discovery
against the bundled `solid-js@2.0.0-rc.0` contract, reports 65 incompleteness
findings: exports whose summaries state no `callbacks` row — which schema v1
reads as the certified negative "never invokes a caller-supplied callback" —
while the installed release observably invokes a function argument. A sample
was verified by hand against the real package: `untrack`, `flush`,
`createSignal`, `merge`, `latest`, `isPending`, `flatten`, and `children` all
invoke a caller-supplied function. The declared behavioral probes themselves
all pass (89/95 driven, 0 failed); only the negatives-by-omission are
contradicted.

The same run against the other bundled artifacts reports 97 incompleteness
findings for `@solidjs/web@2.0.0-rc.0` (40 distinct
`(entrypoint, export, parameter, execution)` rows over 13 export names), 33 for
`solid-js@1.9.14` (13 rows over four entrypoints), and **none** for
`@solid-primitives/scheduled@1.5.3`, whose exact-version review holds.

**Determined (2026-08-23, by reading the consumption path; no experiment run).**
Consumers are exposed. The dialect shadows a contract for exactly the pairs
`(dialect-owned module, name in the dialect's primitive table)` and nothing
else, so every other export of a `solid-js` contract is consumed normally —
negatives included.

- Loading applies no dialect filter at all. `load_package_contracts_reporting`
  (`rust/crates/solid-facts-backend/src/diagnostics.rs:915`) fills one
  `HashMap<package_name, PackageContract>` from four tiers — bundled
  (`:938-976`, via the `include_bytes!` table at `:804-835`), package-published
  (`:977-987`), project-local `.solid-checker/contracts/<pkg>/` (`:988-998`),
  explicit `--contract` (`:999-1016`) — and hands the certifiable subset to the
  IR at `:292-297`. The bundled `solid-js` contract is preloaded on the hot path
  (`rust/crates/solid-facts-backend/src/main.rs:174`) and applies even with no
  `node_modules`, because `contract_matches_manifest` is `is_none_or`
  (`diagnostics.rs:1292`).
- The evidence gate does not help here. `contract_evidence_is_certifiable`
  (`diagnostics.rs:1116`) plus `claims_are_certifiable`
  (`rust/crates/solid-reactive-ir/src/lib.rs:1297`) admit `verified`, and both
  bundled `solid-js` documents declare `"kind": "verified"`.
- The shadow is `native_vocabulary_outranks_contract`
  (`rust/crates/solid-reactive-ir/src/contracts.rs:192`) —
  `dialect.owns_module(module) && dialect.declares_primitive(imported)` —
  applied at the namespace-member (`contracts.rs:413`), named/default
  (`:506`) and re-export (`:591`) binding sites, each of which `continue`s
  without creating a `ResolvedContractBinding` at all. `owns_module` and
  `declares_primitive` are `rust/crates/solid-dialect/src/lib.rs:1054` and
  `:1049`; `modules()` is four specifiers for v1
  (`rust/crates/solid-dialect/src/solid_1x.rs:105`) and thirteen for v2
  (`rust/crates/solid-dialect/src/solid_2.rs:110`), and the primitive tables are
  the `primitive()` matches at `solid_1x.rs:122` and its v2 counterpart.
- The suppression itself is `interproc.rs:1216-1218` — `if
  contracts.callbacks.contains_key(symbol) { continue; }` — which skips the
  `contract_generation_obligations` push at `:1230-1240`. The map is filled
  with no emptiness guard (`source_discovery.rs:1365-1367`), and an omitted
  `callbacks` field deserializes to `Known(vec![])`, not `Unknown`
  (`lib.rs:1108` `#[serde(default)]` + `lib.rs:1022` `Default for
  ContractClaim<T>`). An empty list is therefore `contains_key == true`, and the
  obligation is skipped. That *is* the negative claim taking effect.
- Reachable today: ~11 negative-callback exports in v1's `solid-js.json` that
  the v1 table does not name (`enableExternalSource`, `requestCallback`,
  `createComponent`, `observable`, `cancelCallback`, …), all 48 in
  `solid-js/web`, `createRenderer` in `solid-js/universal`, and every export
  under a subpath `modules()` omits — `solid-js/web/storage`
  (`provideRequestEvent`), `solid-js/jsx-runtime`, `./jsx-dev-runtime` — plus
  ~24 in v2's (`createComponent`, `flatten`, `runInServerComponentScope`,
  `ssrScope`, `isWrappable`, `storePath`, …). Several of these demonstrably do
  invoke a caller-supplied function.
- A second channel bypasses the shadow entirely: `bundled_returns`
  (`source_discovery.rs:1286-1305`, read at `:208`, `:706`, `:764`, `:907`) is
  keyed on the **export name**, not a resolved symbol, so a `solid-js`
  contract's `returns` claims reach dialect primitives. It reads only `returns`,
  so it does not widen the negative-callback exposure, but "the dialect fully
  shadows solid-js contracts" is false in general.

Consequence for RFC 0002 Stage 2: the incompleteness blocker in `contract
verify` is *not* the only impact. It does fail closed — a regenerated
`solid-js` contract cannot be mechanically promoted while discovery contradicts
it — but the already-shipped bundled artifacts are `verified` and live, so the
wrong negatives are consumed now, independently of Stage 2. Resolution is
unchanged in shape and now clearly not optional: add the missing callback rows
to the bundled contracts (and their probes), or state the negative honestly as
`{"status": "unknown"}` for the exports discovery contradicts. A carve-out for
"dialect-owned modules" is not available as an answer, because the contradicted
exports are precisely the ones the dialect does *not* own.

### Resolved for `solid-v2/solid-js` (2026-08-23)

Every one of the 65 findings is now either a row proven from the installed
release's own implementation or the unknown sentinel. `contract probe` with
discovery reports **0 incompleteness and 0 failures** against the artifact
(124 claims, 113 passed, 11 undriven), and `make contract-conformance` is green
with every new claim behaviourally probed in each mode it is stated for. The
per-export audit — source citation to row — is in the commit that carries this
change; the shape of the answer is:

- **Identical in both builds, `callbacks[0]=inline`:** `untrack`, `latest`,
  `isPending`, `flatten`, `createComponent`, `createRevealOrder`,
  `runInServerComponentScope`.
- **Browser-only callback:** `flush`. The server build is
  `function flush() {}` — no declared parameter, empty body — so its variants
  keep a *proven* negative rather than inheriting the browser row.
- **Client tracks, server runs it once:** `children`, `createSignal`,
  `createOptimistic` (`0=tracked` / `0=inline`), plus `callbacks[0]=inline`
  added to the server variants of `createMemo`, `createEffect` and
  `createRenderEffect`, whose browser rows were already there.
- **Two slots:** `repeat` (`0=tracked` on the browser, `1=inline` in both — a
  row callback runs with the listener cleared and a signal it reads never
  re-runs it), `createLoadingBoundary` (`0=tracked, 1=tracked` on the browser;
  `0=inline, 1=deferred` on the server), `createErrorBoundary`
  (`0=tracked, 1=tracked` on the browser; both deferred on the server, where
  neither argument is referenced outside the thunk the export returns).
- **Sentinel:** `merge`. It is variadic and wraps *every* function argument in
  a memo, so any finite `callbacks[]` certifies a false negative at the first
  parameter past it. `{"status": "unknown"}` is the only honest schema-v1
  encoding, and `scripts/check-bundled-contracts.mjs` now reads that value
  instead of throwing on it.

Two **stated** claims were falsified on the way, not merely incomplete, and are
corrected with them: the server variants of `solid-js`'s `createRenderEffect`
and of `@solidjs/web`'s `effect` said `callbacks[1]=deferred`, while
`serverEffect` invokes `effectFn` synchronously inside the call
(`solid-js` `dist/server.js:668-729`; `@solidjs/web`'s server `effect` is
literally `(fn, effectFn, options) => createRenderEffect(fn, effectFn, options)`).
Both now say `inline`. The old conformance body could not see the difference —
it asserted only that the apply did not *re-run* — so the bodies for those two
slots now assert that it ran inside the call.

`fixtures/reactive-ir/bundled-contract-callback-consumer` settles the consumer
half end to end, which reading the code could not: with the certified-negative
contract, a `doubled()` read inside a callback passed to `flatten` produced no
finding at the call site; with the row it produces `SC1001` there, and the same
call from compiler-tracked JSX stays clean. `createEffect` beside it is the
dialect-shadowed control and does not move. Only that fixture's snapshot
changed — no existing finding moved, because every other contradicted export a
fixture touches is in the dialect's primitive table.

### Still open

- **`solid-v2/@solidjs/web`.** 40 rows over `applyRef`, `createComponent`,
  `dynamic`, `effect`, `getNextElement`, `memo`, `mergeProps` (parameters 0 and
  1), `renderToString`, `ssrElement`, `untrack`, `frameTransformResult`,
  `serverComponentResponse` and `provideRequestEvent`, most repeated across
  `.`, `./jsx-runtime` and `./jsx-dev-runtime`, which re-export the same
  functions.
- **`solid-v1/solid-js`.** 13 rows: `createComponent 0=inline` and
  `createResource 0=tracked` under `.`, `./jsx-runtime` and
  `./jsx-dev-runtime`; `mergeProps 0=tracked, 1=tracked` under `.` and
  `./web`; and `getNextElement 0=inline`, `use 0=inline` under `./web`.
- **`requestCallback` (1.x) cannot be measured by discovery at all.** Its probe
  schedules a task whose callback is not a function, and 1.x's `workLoop`
  throws from a `MessagePort` handler *after* the worker has answered, killing
  the process. `runSessionWithRestarts` treats a whole-process failure as a
  mode-wide fact and records the remaining claims undriven rather than
  retrying, so one export truncates the whole run: the 1.x worklist above had
  to be rebuilt by probing the contract in eight-export chunks. It is a
  callback taker by construction (`workLoop` invokes `task.fn`) and its
  negative is wrong, but no automated observation of it exists.
- **`scripts/check-bundled-contracts.mjs --write` cannot be used on the
  composed 1.x contract.** The row evidence it writes is not something
  `scripts/generate-bundled-solid1-contract.mjs` reproduces from its inputs, so
  the write immediately makes `check-composed-contracts` report the artifact
  stale. Recording probed evidence for 1.x needs the composer to carry it.
- **The review contract's tables disagree with the runtime in two places.**
  `rust/crates/solid-facts-backend/src/bin/solid-contract-gen.rs` states
  `repeat` `Callback(1, "tracked")` and the boundary fallbacks as `deferred`;
  the runtime observations above make the first `inline` and the second
  `tracked` on the browser. Those tables feed
  `every_callback_taking_export_is_modelled_or_excluded` and the dialect
  vocabulary rather than any consumer, so nothing certifies from them today,
  but they are the same claim written twice and only one of the two was
  audited.

## Closed 2026-08-23: a declaration sibling no longer certifies what it hid

The `.d.ts` residual recorded under *Closed 2026-08-23: under-marking in the
attribution ladder* was a certified negative, and it fired on the shape almost
every published package has. It is closed in the only direction the facts
allow: the enumeration now reports itself incomplete, and emission widens.

**Mechanism.** `index.js` writes `import { channelFor } from "./channel.js"`.
TypeScript resolves that specifier to `channel.d.ts` whenever one exists beside
`channel.js` — a declaration file wins over an adjacent implementation in every
resolution mode `analyzeTarget` configures, and `closureOf` still seeds the
runtime `channel.js` as a root, so the program holds *both* files as unrelated
modules. `runtimeIdentity` is minted from the symbol's `ValueDeclaration`
(`durableRuntimeRefFor` → `runtimeID` in the pinned solid-ts-facts), so the
call in `forwarded` carries `channel.d.ts`'s identity and `channel.js`'s
`channelFor` has no reference outside its own file. Three lookups then fail in
the same direction, all downstream of that one split:

- `all_function_call_sites`
  (rust/crates/solid-reactive-ir/src/indexes.rs:2077) resolves the callee symbol
  to the declaration, `functions_by_symbol` has no function for it — a
  `declare function` has no body and so no `FunctionFact` — and the caller edge
  is dropped.
- `compute_entered_only_through_calls`
  (rust/crates/solid-reactive-ir/src/attribution.rs:190) walks the same symbol's
  references, finds only the declaration name and the export specifier, and
  reports the entry set fully enumerated.
- `CallGraph::reach` therefore returned `complete: true` with `reaching` holding
  the helper alone, and `export_names_from_reachability` mapped that to
  `Some(vec![])` — *decided: no export reaches this* — so `forwarded` and
  `Isolated` were both published certified with `complete=true`, no marker
  degradation, and (before this) a zero-export review-plan note as the only
  trace.

**Why not an exact fix.** Nothing pairs a declaration file with the runtime
module it describes. `ImportFact` carries only specifier text (the same finding
as *Open: package contracts are bound to a module name*), and the compiler
holds no link between the two files — they are separate modules that happen to
share a name on disk. Recovering the edge would mean matching `channel.d.ts` to
`channel.js` by path, which is exactly the substitution the precision contract
forbids. The generator's own runtime resolver *does* know the pairing
(`closureOf` resolved `./channel.js` to the runtime file), so an exact fix
exists in principle: thread the resolved runtime module graph through
`--emit-contract` and join a declaration-bound import to the runtime module's
export by module identity plus ESM export name. That is new data across the
backend/IR seam for all four tiers and is not attempted here.

**The fix (fail closed).** `module_surface_is_unaccounted`
(rust/crates/solid-facts-backend/src/main.rs) gates the reachability rung. A
reaching function that is *decided: not an export of this entrypoint*, is
published by its own module's export surface, and has no reference anywhere
else in the project by exact runtime identity or canonical symbol, cannot have
had its entry set enumerated: either its importers are outside the analyzed
file set, or they are inside it and bound to a different declaration of the
same module. `export_names_from_reachability` returns `None` for it, the ladder
falls to `fallback-all`, and the marker records `mechanism: "fallback-all"`.

The gate is deliberately not asked of an entrypoint export (its consumers are
answered by marking its own name — this is what keeps `forwarded` exact in
`parameter-member-forwarded` and `channelFor` exact on `./direct`) nor of a
module-private function (its entries are exactly what the graph enumerates —
this is what keeps `unreached-private-obligation`'s zero-export answer).

**Before/after**, `fixtures/package-contracts/parameter-member-forwarded` with
a `channel.d.ts` added:

| | before | after |
| --- | --- | --- |
| mechanism | `reachability` | `fallback-all` |
| `.:forwarded` | certified | `reactiveReads`, `returns` unknown |
| `.:Isolated` | certified | `reactiveReads`, `returns` unknown |
| `./direct:channelFor` | exact `parameter-member` row | unchanged |

**The over-marking cost, honestly.** `Isolated` reaches nothing and is marked
anyway; the widening is to *every* export of the entrypoint, because that is
what `fallback-all` means. For a package that ships a `.d.ts` beside every
runtime module — the normal published shape — every internal-module obligation
now widens this way, so a generated contract's unknown surface grows by roughly
the number of exports per affected entrypoint. Those exports were previously
published as certified negatives about behavior the analyzer had not seen, so
the trade is real precision lost for real soundness gained, not noise for
nothing. Recovering the precision needs the exact fix above, not a narrower
heuristic.

Pinned by `fixtures/package-contracts/declaration-sibling-reach` (the split,
including the `./direct` control that must stay exact) and
`fixtures/package-contracts/entry-reexport-identity` (the same source with
identity intact, which must keep its three-way answer). Both are in
`scripts/contract-corpus.mjs`; the corpus is 24 packages.

**Not closed by this.** A declaration sibling still costs the whole entrypoint
its claims rather than just the reaching exports, and every other consumer of
the split identity — anything that resolves a call through
`functions_by_symbol` — still silently sees no edge. The gate protects the
attribution ladder's answer; it does not restore the call graph.

## Closed 2026-08-23: `contract verify` certified what no run had observed

A second adversarial review, this time of the RFC 0002 pipeline rather than of
the engine, found that `solid-checker contract verify` could reach
`evidence.kind: "verified"` on a contract **none of whose claims any probe had
observed**. Nine defects, in the Node commands under `packages/cli/scripts/`.
Each is closed; the design decisions the closures required are recorded in
[RFC 0002's Amendments section](rfcs/0002-machine-verified-contracts.md#amendments)
rather than silently applied.

**1. Stale `probed` markers (critical).** A `probed` row marker is a durable
property of the *document*. `writeProbeEvidence` never refreshed or removed
one, and `collectBlockers` never asked whether the *consumed* report witnessed
it. So probe-healthy → probe-observes-nothing → verify certified every marker
the healthy run had left behind. Closed on both sides: a `--write` now
supersedes the marker of any claim it re-drove that did not pass (reported as
`superseded`), and verification converts any marker its own report does not
witness — a passing claim of the same identity covering at least the marker's
modes — recording it under `staleProbedMarkers`. Conversion rather than a
blocker, because an unwitnessed marker and an absent one are the same state
from this run's point of view, and because blocking would make a legitimate
`--modes` narrowing unable to verify anything rather than able to verify less.

**2. `kind` certified from zero observations (critical).** `kind` is the one
claim schema v1 has no sentinel for, so `convertUnconfirmedClaims` exempted it
— relying on "a runtime kind that disagrees is a failed probe", which is
vacuous when the probe observed nothing. An import that threw, a missing
export, a crashed session, or a `--modes` narrowing all produced zero
observations and a verified contract. A `kind` claim not probed-passed in every
stated mode is now a **blocker**, with the deliberate consequence that a
package this checker cannot import cannot be machine-verified at all. Also in
this slice: discovery probes now run for `value` summaries, which are the
maximal negative claim and were exempt from their own falsifier; and the probe
report's family labels were realigned with what verification does — see 3.

**3. `because` destroyed by the plan rewrite.** A contract document carries no
generation-time attribution, so re-deriving the review plan from the verified
bytes threw away the only record of why each claim is unknown. Items now
inherit the prior plan's `because` by id, and every sentinel the verification
created gets a `because.conversion` mirrored from the sidecar.

**4. `--no-discovery` was invisible.** The probe report did not record it, so
`<contract>.verify.json` listed the incompleteness blocker as checked when
nothing had looked. The report records `discovery: {enabled, parameters}`, and
verification refuses a report with discovery disabled — or with no discovery
state at all.

**5. `returns=accessor` was transitively satisfiable.** The observation plants
its signal read inside the claimed callback, so `(cb) => () => cb()` passed.
The observation now also measures caching within one tracked read; a
forwarding closure is `undriven`, and a real `createMemo` accessor still passes
(proven against an installed `solid-js@1.9.x`). **An uncached derived accessor
— 1.x `mapArray`'s plain tracked function is the real example — now lands
undriven too**, and its `returns` domain converts. That is the safe direction
and it is a real precision loss: recovering it needs a distinguisher that
separates "recomputes per read because it is a plain tracked function" from
"recomputes per read because it is not an accessor at all", which no counter
available to a generic driver does.

**6. Fabricated call counts.** The worker stamped a per-probe-type constant, so
`evidence.calls` was a table lookup and a `deferred` claim recorded two calls
for one invocation. Counted now.

**7. Inherited-summary variants dodged conversion.** The walk converted an
inherited summary's five top-level domains and then descended into `variants`
on their own evidence, so the exact per-environment claims — the ones a
consumer selects — passed through certified. The inheritance travels with the
walk now.

**8. Summary-level markers outlived their claims.** An export summary's own
`probed` marker is computed from its `callbacks[]` rows and top-level
`returns`. Once those are converted (verify) or deleted (a review certifying
them absent), the marker asserted an observation of claims the document no
longer contained, and any row without evidence of its own inherited it. Both
paths recompute it.

**9. `mergeSummaries` one-sided divergence.** `left.returns ?? right.returns`
handed the environment-unaware base one branch's proven claim when the other
branch proved *none* — and in a proven summary an absence is a certified
negative, not an absence of knowledge. One-sided presence is a divergence now,
so the base is the sentinel and the exact per-branch claims stay in `variants`.
Merge-produced sentinels also carry a `because.divergences` on their review-plan
item, naming the branches and the shape of the disagreement; a merge was the
second emitter of the sentinel and the silent one.

**Fixtures.** `fixtures/package-contracts/conditional-returns-divergence` pins
the one-sided shape and `conditional-returns-divergence-both` the
both-present one, with a `Steady` negative control in each that must stay
unconditional. Both are in `scripts/contract-corpus.mjs`, which closes the
corpus-coverage gap the review flagged: `mergeSummaries` and `mergeClaimRows`
were not executed by the corpus at all. Everything else in this list is pinned
by unit cases in `scripts/contract-probe.test.mjs`,
`scripts/contract-verify.test.mjs`, and
`packages/cli/test/contract-attribution-notes.test.mjs`.

**Still fail-closed or unresolved after this.**

- **An inherited summary's *omitted* domains still pass through as certified
  negatives.** Conversion covers every domain the summary carries; a domain it
  omits is another package's proven negative, and schema v1 has no way to say
  "this negative is inherited". The reviewed tier is the only answer today.
- **An uncached derived accessor's `returns` claim is now unprovable** (5
  above).
- **A claim this run did not attempt keeps whatever marker it had** on
  `--write`. That is deliberate — the command reports what it drove — and it is
  safe only because verification independently refuses to certify an
  unwitnessed marker. The two checks are load-bearing together.
- **`--modes` narrowing can never verify.** Every stated mode must carry a
  passing `kind` observation, so a narrowed run blocks rather than converting.
  Deriving a package's genuinely applicable modes is RFC 0002 unresolved
  question 8.
- **`mergeClaimRows` still unions the multi-row domains** (`callbacks`,
  `reactiveReads`, `ownerRequirements`) across branches, so a row proven in one
  branch is published in the base even where the other branch proves the export
  invokes nothing. Unlike `returns`, a union there is not obviously the
  dangerous direction — it over-claims that a callback runs rather than that it
  does not — but it is the same one-sided shape and has not been argued
  through. Left open deliberately; fixing it without measuring the ecosystem
  cost would be the same guess in the other direction.

## Generated contracts contradicted by the runtime probe (2026-08-23)

The corpus-wide machine-verification measurement
(`benchmarks/ecosystem/verification-report.md`) attributed a root cause to each
of its 210 refusals. Two of them were defects in `contract generate` itself
rather than in what a probe could reach, and both are fixed here.

**1. An exported class was `kind: "value"`.** `Callability` is derived from
`GetSignaturesOfType(…, SignatureKindCall)`, and a class type has construct
signatures and *no* call signature, so every exported class answered
`nonCallable` and `promote_callable_export` /`promote_entry_callable` left it a
value. At runtime `typeof C === "function"`, which is what the probe's kind
probe measures, so each such export was a failed claim — 102 of them in
`@tanstack/solid-db@0.2.37` alone, all error classes, and `kind` is the one
claim schema v1 has no sentinel for, so one wrong answer blocks its whole
entrypoint. Class-ness now comes from the compiler's own declaration kind plus
the syntax facts' class-name spans, walked through alias and
`const Alias = SomeClass` hops by exact symbol identity.

**2. A retained callback parameter published the negative claim.** Local calls
are summarized transitively and the caller inherits the callee's callback
answer, but an *empty* answer is the claim "invokes no caller-supplied
function". `createComputation(fn, init) { const c = { fn, value: init, … }; }`
retains rather than calls, so solid-js 1.9.14's `createMemo`, `createEffect`,
`children`, `createSelector`, `createDeferred`, `createRenderEffect` and
`createComputed` each certified inertness the package contradicts on every use.
Retention is now tracked per parameter and opens
`callbacks: {"status":"unknown"}` on the declaring export, propagating along the
forwarding edges the callback rows already travel.

Measured against a `HEAD` baseline binary, per package, exports whose
`callbacks` domain moved to the sentinel: `@solidjs/web@2.0.0-rc.1` 38/388
(9.8%; 9 of its 48 exports with proven rows folded into the sentinel because a
sibling parameter of the same export escaped), and **zero** in
`@solid-primitives/analytics@2.0.0-next.2`,
`@solid-primitives/context@2.0.0-next.2`,
`@solid-primitives/connectivity@1.0.0-next.2` and `@corvu-next/dialog@0.1.5` —
the four corpus rows that reached `verified` before. On solid-js 1.9.14 the
probe's incompleteness findings fell 35 → 23 rows and 10 → 6 distinct exports.

**Still fail-closed or unresolved after this.**

- **A class's behavioral domains other than `callbacks` are still omitted.**
  The generator summarizes function declarations, not construct signatures, so
  a constructor that reads a signal or calls `onCleanup` publishes no
  `reactiveReads` or `ownerRequirements` row, and an omitted row is a certified
  negative. Only `callbacks` fails closed today, because only that domain has a
  demand-sensitive consumer and therefore a bounded cost. Resolving it properly
  means resolving a class export to its constructor's summary node — including
  the inherited constructor when the class declares none — which is a separate
  slice.
- **Retention is a closed list of positions**: an object-literal property
  value, an assignment value whose target is not rooted at a caller-supplied
  parameter, and a computed read of a rest parameter. A parameter that leaves
  through a conditional branch into a local binding does not open the sentinel,
  which is why solid-js 1.9.14 `createRoot` (`updateFn = unowned ? fn : () =>
  fn(…)`, then `runUpdates(updateFn, true)`) still publishes an empty callbacks
  claim in the client build and is still contradicted by the probe. Widening the
  list to conditional branches was measured and rejected: it converts a third of
  `@solidjs/web`'s exports while proving nothing.
- **Sub-entrypoint variants lag the root.** `./web:createComponent`,
  `./web:mergeProps` and `./jsx-dev-runtime:createEffect`/`createDeferred`
  remain contradicted for solid-js 1.9.14 where their root-entrypoint
  counterparts no longer are; those summaries are inherited through a
  dependency contract rather than analyzed in the sub-entrypoint's own runtime
  target.
- **Eight `callbacks[].execution` claims are simply wrong** and are now
  reachable because the probe no longer dies early: `.:onMount` states
  `tracked` where the runtime is `deferred`, `./jsx-dev-runtime`'s
  `createComputed`/`createMemo`/`createRenderEffect` state `inline` where the
  resolved artifact is `tracked`, `./jsx-dev-runtime:createSelector` states
  `deferred` where it is `tracked`, and `./web:use` states `deferred` where it
  is `inline`. These are a *different* generator defect — the wrong execution
  kind, not a missing row — and were present before this change; they were
  invisible only because the earlier contract crashed the probe worker first.

  **Re-measured 2026-08-23, after the execution-kind pass: four closed, four
  still fail.** The three `onMount` claims (`.`, `./jsx-dev-runtime`,
  `./jsx-runtime`) and `./web:use` no longer appear as failing claims at all —
  the fold over the enclosing callback chain answers `deferred` for
  `onMount(fn) { createEffect(() => untrack(fn)) }`, which is what the runtime
  does; `./web:use` stops failing in the same stage, and what the emitted row now
  says there was not separately read back. The four `./jsx-dev-runtime`
  sub-entrypoint variants —
  `./jsx-dev-runtime:createComputed`, `createMemo`, `createRenderEffect` and
  `createSelector` — still fail in `server` mode with the identical claim text and
  observed `tracked`, which is the "sub-entrypoint variants lag the root" bullet
  above: their root-entrypoint counterparts are fixed and these summaries are
  inherited through a dependency contract rather than analyzed in the
  sub-entrypoint's own runtime target. They are four of the ten `callbacks`
  failures left in the whole corpus. Note that these four are reached in a
  `server` session and are *not* withdrawn by the new inert-runtime rule, because
  `./jsx-dev-runtime` resolves unconditionally to `dist/solid.js` and re-runs
  normally — the withdrawal is per runtime, not per mode, and this is the shape
  that distinction exists for.

## Closed 2026-08-23: the probe environment was measuring itself

The corpus-wide machine-verification measurement attributed a root cause to each
of its 210 refusals, and the largest single one — `kind-observed`, 82 rows — was
not a claim anybody disagreed with. It was the absence of any observation at
all: roughly fifty of those rows had an entrypoint whose module **throws on
import** in at least one mode, so no `kind` reading existed, and `kind` is the
one claim schema v1 has no unknown sentinel for. A further 2,248 claims went
undriven because the throwaway install did not contain packages the probed code
imports, and three wide-surface rows exceeded a flat wall budget and produced no
result at all.

None of that is a fact about a package's reactivity. It is the probe's own
environment being reported as the package's behavior, and four things are fixed
here.

**1. A minimal, mode-scoped, recorded import environment.** The probe worker now
defines fifteen browser globals — `window`, `document`, `self`, `location`,
`screen`, `history`, `localStorage`, `sessionStorage`, `matchMedia`,
`requestAnimationFrame`, `cancelAnimationFrame`, `getComputedStyle`,
`MutationObserver`, `ResizeObserver`, `IntersectionObserver` — before it imports
anything, in the `client`, `development` and `production` sessions only. The
list, and the members of each fake object, are derived from what the corpus's
failing packages actually reach for, not from what a browser happens to have.

The premise is stated rather than assumed: **a claim observed under the shim is
a weaker observation than one observed in a browser.** So `<contract>.probe.json`
gains an `environment` block naming, per mode, the globals the process invented
and the ones Node already provided, and `<contract>.verify.json` carries it
forward. Four rules bound it: server modes are never shimmed (an import that
throws on `window` under `--conditions node` is a *truthful* observation);
generation is untouched, since `contract generate` imports nothing; every faked
value is stamped with a non-enumerable `__solidCheckerProbeShim` accessor and
the process carries `__solidCheckerProbeEnvironment`, so a probe body can ask;
and an import that still throws is unchanged — undriven, `import-failed`, with
the throw as its reason.

The sharpest reason the record exists: a `typeof window === "undefined"` guard
never threw, so for those modules the shim *redirects* rather than rescues. A
package that took its server path in every earlier measurement now takes its
browser path.

**2. Peer-complete installs.** The manifest's install environment was built for
static generation and installs what a row *pins*. For Solid 2 that runtime is
two packages, and rows whose package declares only `solid-js` as a peer got only
`solid-js` — 248 claims of the previous measurement were an
`ERR_MODULE_NOT_FOUND` for `@solidjs/web` attributed to the package. The harness
now completes the pinned runtime with the parallel `@solidjs/web` version, and
separately installs the non-optional peers the *installed artifact's own*
`package.json` declares, in a second npm invocation so no peer range can take
part in resolving a pinned version. If a peer install moves a pin anyway, the
pinned-only tree is restored and the row records that.

The line held: **a missing peer is the harness's gap; a missing undeclared
import is the package's.** `@solid-primitives/utils` (94 claims), `server-only`
(60) and the `react`/`vue`/`svelte`/`vite`/`@angular` group are imported by
packages that declare them nowhere, and completing those would mean the harness
choosing a version the package never named. They remain import throws.

**3. A probe budget that scales with the claim count.** 90 s + 500 ms per
planned claim, capped at 900 s, computed from the exact plan `contract probe` is
about to run rather than from an export count. A flat 120 s was a budget for the
median package and a guaranteed timeout for the wide-surface ones. All four rows
that previously timed out now complete in 83–208 s. A timeout remains its own
outcome class; this changes how many rows hit one, never what hitting one means.

**4. An asynchronous package throw no longer costs a whole mode.** Package code
the probe set running — a deferred callback, a promise left rejected — throws
outside every `try` the worker has. The process died with status 1 and an empty
stdout, so the parent had *no* results for that mode: every probe already
answered was discarded, and because a whole-process failure names no probe to
retry past, the mode ended there. The worker now answers with what it observed,
`completed: false`, and the abort reason, so the parent restarts for the
remainder exactly as it does after a synchronous throw. The reason is reported
and never attributed to a claim.

**Measured, by running the corpus four times against the same two snapshotted
binaries with one group of changes enabled at a time.** Each step is a full
416-row run and each attribution is a per-row set difference, not a
classification of deltas:

| State | Verified | Δ |
| --- | --- | --- |
| 2026-08-22 baseline | 194/416 (46.63%) | — |
| + engine fixes (class kind, retained-callback sentinels) | 214 | +20 / −0 |
| + the abort guard (4 above) | 217 | +3 / −0 |
| + shim, peer-complete install, scaled budget (1–3 above) | **222/416 (53.37%)** | +12 / −7 |

The environment half is a net **+1** on the headline, and that is the honest
result. What it bought is *observation*: claims driven 6,257 → 7,809, rows with
an entrypoint import throw 55 → 34, exports certified by a verified contract
672 → 752, probe timeouts 3 → 0. More observation surfaces more contradictions
as well as more confirmations, and one contradiction refuses a whole contract —
so `probe-failed` rises 65 → 75 as a root cause while `kind-observed` falls
82 → 71.

**Still fail-closed or unresolved after this.**

- **An inert fake can change an answer, and one row shows it.**
  `@solid-primitives/pagination@0.5.2` now fails `createInfiniteScroll
  callbacks[0]=deferred` with `observed inline`, because the fake
  `IntersectionObserver` never fires and a callback a browser would run on
  intersection ran only at setup. The driver already has the right precedent —
  a mismatch its own read scope could explain is recorded `undriven`, not
  `failed` — and the same reasoning applies to a mismatch a faked DOM API could
  explain. It is not implemented, because "which claims depend on which faked
  API" is not knowable from the contract, and the blunt version ("any failure in
  a shimmed mode is undriven") would discard the 99 genuine `tracked → inline`
  findings. The two `@solid-primitives/resize-observer` rows sit on the same
  line, one step less clearly.
- **The synthesized-argument boundary is now the binding limit for DOM
  primitives.** `@solid-primitives/interaction` reads `el.ownerDocument` on the
  element the *caller* passes, and the driver synthesizes `{}` there; the shim
  only let execution get far enough to reach the limit. RFC 0002 refuses a
  ladder of retries deliberately, so this stays undriven rather than being
  guessed at with a fake node.
- **Four globals were reached and deliberately not added.** `EventSource` (12
  claims), `Element` (4), `HTMLAudioElement` (2), `HTMLVideoElement` (2). Each
  needs constructor or `instanceof` identity rather than a value, and faking
  that invents behavior rather than removing an obstacle. 20 claims across the
  corpus.
- **93% of verified contracts still certify no observed behavior.** Verified
  rows carrying a probed behavioral row went 6 → 15 of 222, and the markers kept
  12 → 25. The rate roughly doubled on a base that is still almost nothing; the
  binding constraint is drivability, not the environment.
- **2,745 claims have no probe form at all** and never will —
  `reactiveReads` 1,354, `ownerRequirements` 565, parameter identity 421, nested
  return leaves 257, `asyncBehavior` 100, callback arguments 25, store paths 23.
  They are static claims, or claims schema v1 has no evidence slot for.
- **Wrong execution kind is now the dominant visible defect class**: **159** of
  the 218 failing claims, once the 53 `kind: value → function` failures the class
  fix removes are gone (the six `returns: accessor → array` failures are the
  remaining balance; an earlier revision of this entry said 155, which does not
  add up against the report's own shape table). `callbacks[n]: claimed tracked,
  observed inline` alone is 99. That is a generator *and* a probe defect and is
  tracked in "[Generated contracts contradicted by the runtime
  probe](#generated-contracts-contradicted-by-the-runtime-probe-2026-08-23)"
  above and in "[Closed 2026-08-23: execution-kind vocabularies, tracked-wrapper
  schedules, and one parameter with two
  executions](#closed-2026-08-23-execution-kind-vocabularies-tracked-wrapper-schedules-and-one-parameter-with-two-executions)"
  below, not here. **Re-measured 2026-08-23: 159 → 10 of 63.**

## Closed 2026-08-23: `contract verify` refused without writing anything down

The refusal path built no sidecar at all — `buildVerifyReport` was reachable
only after the promotion succeeded, and its `blockers.raised` was always `[]` —
so the most common outcome of the command was the least legible one. The only
record of *why* a contract was not promoted was stderr: a CI run kept a log or
kept nothing, and the corpus measurement had to recover the RFC 0002 blocker
taxonomy by pattern-matching English sentences against lines carrying absolute
paths.

A refusal now writes `<contract>.verify.json` with `outcome: "refused"`,
`blockers.raised` carrying every line the command printed, `blockers.checked`
carrying the same taxonomy the success path lists, and the consumed probe
report's own figures. Every refusal path goes through it — the blocker list, the
stronger-existing-evidence refusal, and the document-does-not-validate refusal —
so the sidecar exists for the same set of outcomes the stderr lines describe.

The two shapes are told apart by `outcome`, never by which counts are zero:
every field that would imply a promotion is **absent** rather than zeroed — no
`evidence`, no `conversions`, no `probed`, no `summary`, and a `contract` block
with `before` and no `after`, because nothing was written. Success behavior is
unchanged, and the docs sentence that promised "the blockers checked" without
saying the file only existed on one path is corrected.

Two consequences had to be handled rather than discovered later:

- **A refusal never overwrites a record of a promotion.** A sidecar carrying
  `evidence` is the audit trail of a verification that actually happened — of
  some other bytes, if it survived a regeneration, and self-invalidating either
  way — and replacing history with the record of a failed attempt is a strictly
  worse artifact. A refusal record replaces a refusal record; that is the only
  overwrite.
- **`contract generate` read the file's *existence* as proof of a verification.**
  `snapshotPreviousReview` moved a contract to `.previous` and printed "the
  previous machine-verified contract … were kept" whenever the sidecar was on
  disk. With a refusal sidecar that message was false and the snapshot was
  pointless, so the check is now on the record's content: `outcome !== "refused"`
  and an `evidence` block present.

The corpus harness reads `blockers.raised` in preference to stderr, and keeps
the text classifier for journals written before this change.

**Still unresolved after this.** The refusal sidecar is not schema-validated —
nothing loads it, exactly as with the promotion sidecar — and `blockers.raised`
is free text rather than a taxonomy field, so a consumer still classifies by
matching the line. Emitting the blocker *class* alongside each line would remove
the last reason the corpus harness owns a text classifier at all; it is not done
here because the classes live in `contract-verification.mjs` as a flat list and
`collectBlockers` builds sentences rather than tagged records.

## Closed 2026-08-23: execution-kind vocabularies, tracked-wrapper schedules, and one parameter with two executions

Generator defects behind the largest visible class in the corpus verification
measurement — 159 of the 218 failing claims are a wrong
`callbacks[].execution` — the ones that are the generator's own fault rather
than the probe's.

**Measured 2026-08-23, staged.** The class is now **10 of 63** failing claims and
the corpus verifies at **261/416 (62.74%)**, from 222/416. The two halves of the
change set were measured separately, each a full 416-row run against a
snapshotted release binary, with stage 1 built from `origin/main` (95270bee) plus
only the three probe-side files: probe-side fixes **222 → 243** (+21 / −0,
failing claims 218 → 106, execution-kind 159 → 47), generator-side fixes
**243 → 261** (+18 / −0, failing claims 106 → 63, execution-kind 47 → 10). The
full account, including what it cost, is in
[ecosystem-benchmark.md](ecosystem-benchmark.md#the-staged-decomposition-2026-08-23).
The cost is stated there and here: 445 `callbackExecution` rows and 67 proven
exports withdrawn from the generated corpus, and 12 of the 15 verified rows that
carried probed behavioral evidence lost it, because 22 of those 25 markers had
been promoted from observations made in a runtime that re-runs nothing.

### A clearing wrapper stays `inline`

`interproc.rs`'s `primitive_callback_execution` labelled `untrack` and 2.0's
`flush` `"deferred"`, and said so in a comment: a contract consumer reads
`"deferred"` as "not tracked here", which is the meaning the summaries needed.
But `"deferred"` also promises the callback does **not** run before the export
returns, and all four of `untrack`, `createRoot`, `runWithOwner` and `flush` run
it during the call. `docs/package-contracts.md` already stated the vocabulary
the other way round — these primitives stay `inline` while clearing the
listener, and the clearing travels separately through the dialect — so the two
halves of the tree disagreed, invisibly, until `contract probe` began measuring
timing.

The reconciliation is three pieces:

- `Dialect::runs_callback_synchronously`, a **derived** trait method rather than
  a per-dialect table: exactly the members of `runs_callback_deferred` whose own
  `callback_executions` rows are all `Execution::Inline`, so the two answers
  cannot drift. `the_synchronous_clearing_set_is_the_inline_half_of_the_deferred_set`
  pins the concrete sets — 1.x `{createRoot, runWithOwner, untrack}`, 2.0
  `{createRevealOrder, createRoot, flush, runWithOwner, untrack}`.
- `flush` earns its place on the rc runtime's bytes, not on its name:
  `@solidjs/signals` `flush(fn)` is `syncDepth++; try { return fn() } finally {
  … }`, so the callback is invoked and its value returned during the call
  (2.0.0-rc dev bundle). The reviewed bundled contract for `solid-js@2.0.0-rc.0`
  independently states `flush` `callbacks[0] = inline`.
- a composition over the chain of enclosing callback positions, innermost
  outward, replacing "the innermost classifiable wrapper" at both seams that
  needed it: the direct-invocation ladder and the local-callee forwarding
  ambient.

### The ambient tracking scope is not the export-relative schedule

The same composition fixes the opposite error. `onMount(fn) { createEffect(() =>
untrack(fn)) }` published `tracked` because the derivation read the enclosing
`createEffect` callback's lexical *tracking scope* and published it as the
callback's *schedule*. The clearing wrapper means the callback is not tracked;
the effect means it has not run when `onMount` returns. The fold answers
`deferred`, which is what the repo's own reviewed semantics map states for
`onMount` and what the runtime does.

Order is load-bearing and the fold keeps it: `untrack(() => createMemo(fn))`
stays `tracked`, because the memo subscribes what runs inside it and an outer
`untrack` cannot undo that. A rule phrased as "any clearing wrapper anywhere
means not tracked" answers `deferred` there and is wrong.

### `Tracked` does not mean "later", and the dialect now says which

The first version of the fold read a tracked wrapper above a clearing one as
`deferred`, on the assumption that a tracked computation has not run when the
creating call returns. **In 1.x that is false for four of the five tracked
wrappers the schedule table can produce.** Against `solid-js@1.9.14`
`dist/solid.js`: `createMemo` (`:244-256`), `createRenderEffect` (`:218-221`)
and `createComputed` (`:214-217`) all call `updateComputation(c)` on the
creating call; `mergeProps` (`:1329`) wraps every function-valued source in a
`createMemo`, so it is eager at every index; `createResource`'s tracked source
(`:283`) is a `createMemo` too. Only `createEffect` (`:222-229`) defers, via
`Effects ? Effects.push(c) : updateComputation(c)` — and it defers exactly
because a package export runs under an owner, where `createRoot`'s
`runUpdates(updateFn, true)` (`:192`) has installed `Effects = []` (`:820`).

Measured with the probe worker's own observation shape against the oracle
install (`rust/target/tsc-oracle/v1`, `--conditions browser`), five shapes
claimed `deferred` where the runtime and the probe answer `inline`:
`createMemo(() => untrack(cb))`, `createMemo(() => createRoot(() => cb()))`,
`createRenderEffect(() => untrack(cb))`,
`createRenderEffect(() => createRoot(() => cb()))` and
`mergeProps({a: 1}, () => untrack(cb))` — plus the same shape through the
local-callee forwarding seam that solid-js's own `dist` goes through. These were
not accidental leftovers: they were a *derived* `deferred`, which is worse.

The fix is a third dialect fact, `Dialect::tracked_callback_timing(primitive,
argument, argument_count) -> Option<TrackedCallbackTiming>`, established from
the audited runtimes rather than from names:

- 1.x eager (`DuringCall`): `createMemo`, `createRenderEffect`, `effect`
  (`solid-js/web`'s alias for it), `createComputed`, `createResource`'s
  two-argument source, `mergeProps`. Deferring (`AfterCall`): `createEffect`.
- 2.0 eager: `createEffect` and `createRenderEffect` — both go through
  `effect()`, which calls `recompute(node, true)` unconditionally
  (`@solidjs/signals@2.0.0-rc.0` `dist/dev.js:4107-4121`) — plus `createMemo`
  (`:4558-4560`), `createSignal(fn)` (`:4548-4552`), `createOptimistic`
  (`:4778-4790`) and `createProjection` (`:5634-5675`), all of which build a
  non-lazy `computed` and so hit `setupComputedNode`'s
  `!options?.lazy && recompute(self, true)` (`:2845`). Deferring:
  `createTrackedEffect`, which builds a `lazy` computed and only
  `enqueue`s it (`:4253-4309`). **The two dialects disagree on
  `createEffect`**, which is why this cannot be one shared table.
- Unestablished, and therefore the unknown sentinel: 1.x `createSignal`/
  `createStore` (the argument is stored, never invoked), 2.0 `createStore` and
  `createOptimisticStore` (their derived overloads did not accept the probe's
  call shape, so no measurement backs a claim), and every tracked primitive with
  no schedule row in `primitive_callback_execution` at all.

`the_tracked_callback_schedule_partitions_each_dialect` pins all three sets per
dialect. The fold composes a detached callback under an eager wrapper to
`inline`, under a deferring one to `deferred`, and under an unestablished one to
the unknown sentinel — it returns `Option<&str>` now, and both ladder seams plus
the forwarding seam treat its `None` as authoritative rather than falling back
to the lexical answer. Where tracking is *not* cleared the answer stays
`tracked` regardless of schedule: attribution is the claim there, and the
wrapper's timing is not asked for.

### The cross-target merge unioned contradictory callback rows

The per-export contradiction sentinel below runs inside Rust, **once per
analyzed target**. `mergeSummaries` in
`packages/cli/scripts/generate-package-contract.mjs` then unions the targets'
callback rows, with a comparator that broke ties on `execution` precisely
because two executions per parameter were expected there. So the sentinel was
bypassed for every conditional export, and `fixtures/package-contracts/
conditional-callback-conflict` shipped a base carrying `parameter: 0` as
`deferred` *and* `inline`. `returns` and `asyncBehavior` had been given the
sentinel for this exact shape (`claimDomainsDiverge`); `callbacks` had not, in
the same function.

`callbackRowsContradict` now applies the same rule to the merged callback rows,
reports the divergence through `onDiverge` so the review plan's
`unknown-sentinel` item names both branches, and leaves the exact per-branch
claims in `variants`. The fixture is registered in
`scripts/contract-corpus.mjs`, so a regression of the union specifically fails a
gate; before, no gate saw the base at all — the process test asserts only the
variants.

One-sided *presence* is deliberately not closed: a parameter with a row in one
branch and none in the other is a positive against a certified negative, the
same hole `claimDomainsDiverge` closed for `returns`. It needs its own
measurement and is listed as unresolved below.

### One parameter with two executions is one false claim

One row is pushed per invocation site and `push_contract_callback` dedups only
exactly-equal rows, so a parameter invoked twice with two schedules published
both — `@solid-primitives/range`'s `mapRange` carried `callbacks[2]` as
`deferred` *and* as `tracked`, and the report lists both as failing. Schema v1
has one execution axis per parameter and the runtime has one behavior, so at
least one row was false and a consumer picking either was guessing.
`contract_export_function` now opens the per-export `callbacks` sentinel for a
parameter carrying two different executions, in the same three lines the
retained-callback fix extended. Rows that agree on `execution` and differ
elsewhere are deliberately not contradictory.

The documents that defined the vocabulary moved with the code, because the
change makes contract emission a consumer that asks *when* a callback ran and
both of them said nothing downstream does. `docs/package-contracts.md` and the
`Execution` comment in `solid-dialect/src/lib.rs` now state the two axes
explicitly — `tracked` is attribution, `inline`/`deferred` are the schedule of a
callback the export does not subscribe — and name `startTransition` and
`createResource` as the two places the readings diverge, along with the reason
emission refuses them rather than restating their attribution as a schedule.

Fixtures: `callback-untracked-wrapper` (a clearing wrapper is `inline`, with the
tracked and deferred negatives), `callback-deferred-untracked-chain` (nesting
and its order-sensitivity, the eager/deferring/unestablished partition, and the
forwarding seam through a bootstrap-resolved local `untrack`),
`multi-role-callback-parameter` (the intra-target sentinel and its width, with
four negatives including two same-schedule sites),
`conditional-callback-conflict` (the cross-target union). All four are in
`scripts/contract-corpus.mjs`.

**Still fail-closed or unresolved after this.**

Two of these ship an *affirmative wrong claim* — not a lost fact, not a
sentinel. Both are pre-existing and both are rows `contract probe` will fail;
they are stated that way because "recorded" is not the same as "harmless".

- **A package-local transparent wrapper around the real `untrack` publishes
  `tracked`, and the truth is `deferred`.** A schema-v1 `callbacks` row carries
  the execution word and no clearing column, so once a local callee's summary
  crosses the forwarding edge an `untrack`-shaped wrapper and a transparent one
  are indistinguishable and the enclosing tracked wrapper wins. Measured shape:

  ```ts
  import { createEffect, untrack } from "solid-js";
  function runUntracked<T>(fn: () => T): T { return untrack(fn); }
  export function mountThroughWrapper(handle: () => void): void {
    createEffect(() => runUntracked(handle));   // published: "tracked"
  }
  ```

  Against solid-js@1.9.14 the callback does not run during the call and its
  reads subscribe nothing, so `classifyExecution` answers `deferred` and the
  `tracked` claim fails — which is what the reviewed bundled contract states for
  the identical `onMount` shape. Inside solid-js itself the wrapper *is* a
  primitive and the composition sees it; an arbitrary package's own detaching
  helper does not benefit. This is the one-line wrapper spelling most of the
  ecosystem uses. `trackedThroughLocalHelper` in the chain fixture is the
  *correct-answer* control for it (its `runNow` genuinely does not clear), so no
  fixture pins the wrong case; adding one is cheap and would make the gap
  visible instead of prose-only. Closing it needs a clearing column the schema
  does not have, or transitive propagation of the clearing fact along forwarding
  edges.
- **A wrapper the fold cannot classify at all falls back to the lexical answer,
  which can be a positive wrong claim.** `enclosing_callback_chain` refuses the
  whole chain on the first position `callback_wrapper_at` cannot classify, and
  the row then comes from `contract_callback_execution(semantic)` — the same
  lexical answer this fold exists to replace. Only
  `primitive_callback_execution`'s table classifies wrappers during generation
  (the bundled solid-js contract is not reachable through
  `contracts.callbacks` in a generation run), so the unclassifiable set is
  large: `batch`, `startTransition`, `catchError`, `createComputed`, `onMount`,
  `onError`, `createSelector`, `children`, `createDeferred`, `produce`, `from`,
  `render`, `hydrate`. Measured, with runtime truth:

  | export body | published | runtime |
  | --- | --- | --- |
  | `untrack(() => batch(() => cb()))` | `deferred` | **inline** |
  | `batch(() => untrack(() => cb()))` | `deferred` | **inline** |
  | `createComputed(() => createRoot(() => cb()))` | `deferred` | **inline** |
  | `createComputed(() => untrack(() => cb()))` | `deferred` | **inline** |

  All three are pre-existing RC3 residue and none is a regression. The honest
  behavior is the sentinel rather than the lexical fallback, and the forwarding
  seam shares it: `forwarded_callback_ambient_execution` now *names* the refusal
  instead of laundering it through `unwrap_or_default()`, but still lets the
  forwarding call's own position answer alone, which is deliberately
  best-effort. Note that `createComputed` reaches this residue and not the
  eagerness one — it has no schedule row, so the chain is refused a step earlier
  than the fold.
- **The contradiction sentinel carries no review-plan reason at the intra-target
  seam.** The `unknown-sentinel` item is derived from the contract's bytes, and
  the `because.attributions` block comes from an obligation marker whose label is
  hardcoded as `UnknownCallbackExecution` / `contract-generation-obligation` in
  `rust/crates/solid-facts-backend/src/main.rs`. A
  `contradictory-callback-execution` reason needs that label plumbed from the
  emitter; the sentinel itself is unconditional and does not depend on it. The
  *cross-target* twin does carry a reason, through `mergeDivergences`.
- **The contradiction sentinel is per export, which is wider than the
  contradiction.** One contradicted parameter discards the other parameters'
  undisputed rows (`contradictOnZeroOnly` in the multi-role fixture pins it).
  Schema v1 offers no narrower spelling: the only granularity below
  `{"status": "unknown"}` is a row's presence, and an absent row is a certified
  *negative*, so dropping only the contradicted parameter's rows would trade one
  contradiction for one affirmative false negative. Narrowing it needs a schema
  change, and the pre-existing `escaped_parameters` sentinel has the same width
  for the same reason.
- **One-sided callback-row presence across conditional targets is not a
  divergence yet.** `callbackRowsContradict` catches two executions for one
  parameter; a row proven in one branch against a *proven absence* in the other
  is the same class of hole `claimDomainsDiverge` closed for `returns`, and it
  still hands the proving branch's positive to the environment-unaware base. It
  needs its own measurement — the blast radius is every conditionally-exported
  callback-taking function — and its own fixture pair.
- **The wrong-execution-kind class is not closed, but it is now small and
  named.** Ten `callbacks[].execution` claims still fail across the whole corpus,
  measured 2026-08-23, and they are three groups:
  - **`@solid-primitives/pagination` `createInfiniteScroll`, three rows** (0.5.2
    `deferred → inline`; 1.0.0-next.6 floor and head `tracked → inline`). This is
    the row already flagged as **possibly the import shim's doing** under
    "[Closed 2026-08-23: the probe environment was measuring
    itself](#closed-2026-08-23-the-probe-environment-was-measuring-itself)": the
    fake `IntersectionObserver` never fires, so a callback a browser would run on
    intersection ran only at setup. Unchanged by this pass and still the leading
    candidate for a "a faked global could explain this" undriven rule.
  - **`solid-js@1.9.14` `./jsx-dev-runtime`, four claims** — `createComputed`,
    `createMemo`, `createRenderEffect`, `createSelector`. The sub-entrypoint
    variant lag described above; their root-entrypoint counterparts are fixed.
  - **Three single rows**: `@solid-primitives/memo@2.0.0-next.2`
    `createWritableMemo` (`deferred → tracked`, both Solid 2 probes) and
    `@solid-primitives/date-difference@1.0.2` `createDateNow`
    (`tracked → inline`). Neither has been investigated; both are new to the
    visible set only in the sense that they were previously buried under 149
    others.

  The `mergeProps` conservative-callable forwarding — a positive row for a
  parameter the export was never proven to invoke — is still untouched, and the
  probe-side noise it was waiting behind is now gone, so it is measurable.
- **A `callbacks` sentinel silences the `returns` probe of the same export.**
  Measured, and the one place this change set *lost* a finding instead of fixing
  it. `@solid-primitives/utils`'s `createHydratableSignal` and
  `createHydrateSignal` publish `returns: accessor` and really return a tuple;
  the probe caught that in all four modes and the corpus reported six such failing
  claims across three rows. After the contradiction sentinel opens `callbacks` on
  those exports, the returns probe reports *"no plantable reactive source: proving
  the returned value is an accessor needs a signal read inside a callback the
  contract states, and this export states none"* — the claim goes undriven,
  verification converts `returns` to unknown, and all three rows now verify.
  Nothing false reaches a consumer, because the wrong `accessor` claim is
  converted rather than promoted. But the generator defect is now invisible to the
  measurement, and three of the generator stage's eighteen gains rest on it. The
  driver plants a `returns: accessor` observation *through* a stated callback and
  has no other way in; giving it a second one (a synthesized reactive argument, or
  driving the accessor directly) is the fix, and it is not attempted here.
- **The forwarding seam's unknown arm is wider than it needs to be.** When the
  composed ambient execution is the sentinel, the emitter opens the
  unknown-callback obligation without knowing whether the callee publishes an
  `inline` row for the slot at all — so an export whose callee rows are all
  `deferred` loses its `callbacks` domain unnecessarily. Reaching it needs an
  unclassifiable-or-unestablished tracked wrapper above a clearing one above a
  local callee, and the cost is precision, never a wrong claim.

## Deferred: verification-suite speed work, robustness findings (2026-08-23)

An adversarial review of the `verify-speed-execution-kinds` speed work raised 26
findings. The stale-green and wrong-answer ones are fixed in that change set
(the worker pool's result attribution and death paths, the coverage key's
dialect-selection ancestor chain, the mid-run store guard, the registry memo's
input digest, `verify-delta`'s gitignored-input basis and its `pkg/contracts/`
row, the porcelain parse, the oracle base's symlink verification). These are the
remainder: none can produce a wrong verdict, each is a claim the code or a
document makes about itself that is narrower than it reads.

Ordered as the review ordered them, most severe first.

- **robustness — oracle case directories are a fixed path shared across
  processes.** `scripts/lib/tsc-oracle-case.mjs:29` — `rust/target/tsc-oracle-cases/<dialect>/case-<index>`
  has no per-process component, so two concurrent gate runs (`make tsc-oracle`
  in one shell and `make verify` in another, or a re-run started before the
  first finished) have worker threads rewriting the same `tsconfig.<pass>.json`
  and source file while the other run's checker reads them. The path was shared
  before the concurrency change too, but eight simultaneous writers make a bad
  interleaving far likelier. Failure shape is a JSON parse error or a bogus
  verdict, not a silent pass. A `process.pid` in `CASE_ROOT`, or a lock, closes
  it.
- **robustness — the provision short-circuit no longer heals a damaged
  install.** `scripts/tsc-oracle.mjs:105-135` — `assertProvisioned` checks only
  each top-level package's recorded `version`, so a missing transitive
  dependency (`csstype`, `seroval`), a deleted `.d.ts`, or a half-wiped tree
  passes and no `npm install` repairs it; the always-install path did. Direction
  of failure is loud (an incomplete install adds TS2307-class errors that land
  outside every case's `allow` set), and `--force` exists, but nothing tells a
  reader when to reach for it and the `already provisioned` line does not
  distinguish "verified complete" from "the two manifests I looked at agreed".
- **robustness — one unit crash now suppresses every drift report.**
  `scripts/coverage.mjs:286-334`, `scripts/contract-corpus.mjs:290-306` — compute
  and compare are two phases, so a crash in unit 40's `analyze`/`generate` makes
  `mapPool` throw before the comparison loop runs and drifts in units 0–39 are
  never printed. They were printed before the crash when the two were
  interleaved. Green/red is unaffected; the diagnostic value is lost on exactly
  the runs that need it. `mapPoolSettled` already exists, is unused, and is the
  right tool.
- **robustness — the gate cache has no eviction.**
  `scripts/lib/gate-cache.mjs:239, 291-298` — one `<key>.json` per (shared digest
  × unit), never pruned, and every checker rebuild invalidates all 83 coverage
  keys and writes 83 more files carrying full findings lists. `createdAt` is
  stored and nothing reads it. Only `make clean` reclaims it. The growth
  expectation is now stated in AGENTS.md; an age or count cap is not
  implemented.
- **robustness — no memory-aware cap on the oracle-gate fan-out.**
  `scripts/lib/pool.mjs:29-31`, `scripts/tsc-oracle-gate.mjs:312-316` — each
  worker thread carries its own `typescript` instance and runs two checker child
  processes, each spawning a TypeFacts producer; `min(cores, 8)` multiplies all
  of it. The cap bounds the process tree, nothing bounds resident memory. On a
  memory-tight runner the failure is an OOM-killed thread, which now surfaces as
  a `gate worker exited with code …` rejection rather than the hang it used to
  cause — loud, but it reads like a gate failure rather than a resource one.

The review's remaining test-coverage finding — the pool's death path being
untested, and one self-referential assertion — is **not** deferred: it is closed
by the four regression tests added to `scripts/pool.test.mjs` in the same change
set (unattributable message, idle death, queued-task settling on close, fatal
answer), and the self-referential `threadId` assertion is replaced. It is
recorded here only so the review's numbering has no silent gaps.
