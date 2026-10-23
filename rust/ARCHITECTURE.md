# Workspace architecture

The workspace separates what is true of *any* Solid checker from what is true
of *one Solid version*. Infrastructure lives under `crates/`; everything
specific to a Solid dialect — its rule catalog and its JSX compiler — lives
under `dialects/`.

```
rust/
├── crates/                      # dialect-independent infrastructure
│   ├── solid-facts/             # the fact model
│   │   ├── core                 # spans, source paths/hashes, generations
│   │   ├── ast                  # Oxc syntax facts
│   │   ├── compiler             # ExecutionMap + CompilerFactsProvider seam
│   │   └── (root)               # per-file/per-project joins with Type Facts
│   ├── solid-dialect/           # the vocabulary a dialect answers through
│   ├── solid-reactive-ir/       # reactive program IR + the Finding model
│   ├── solid-facts-backend/     # daemon, caches, snapshots, dependency source, CLI
│   │   └── wire.rs              # sources, edits, and Type Facts provider interface
│   └── solid-checker-wasm/      # process-free Node-API/WASI entry point
└── dialects/                    # one per carried dialect; solid-v1 was retired
    └── solid-v2/                # everything Solid 2.0-specific
        ├── dialect.json         # rule assembly manifest
        ├── rules/               # solid-v2-rules: rule catalog + solve()
        └── compiler/            # solid-v2-compiler: Solid compiler adapter
```

## Version ownership at a glance

**This build carries one dialect, Solid 2.0.** Solid 1.x was retired in 2026-09
([ADR 0110](../docs/decisions.md#adr-0110)); the seams
below are unchanged, because they are what a *future* dialect re-enters through
and what keeps shared code from switching on an API spelling itself. “Shared”
means the algorithm is common, not that it is version-agnostic: shared code asks
the selected vocabulary and receives that dialect's answer.

| Concern | Shared module / seam | Solid 2.0 ownership |
| --- | --- | --- |
| Syntax, TypeScript facts, reactive IR, caches | `crates/solid-facts`, `crates/solid-reactive-ir`, backend infrastructure | No separate implementation |
| Primitive names, callback semantics, ownership, boundaries, import modules | `crates/solid-dialect::Dialect`; consumers read one `CallbackSemantics` descriptor per call argument | `solid-dialect/src/solid_2.rs` (`Solid2`, `Version::V2`) |
| JSX compiler facts | `CompilerFactsProvider` | `dialects/solid-v2/compiler` |
| Rule projection and wording | `solid-reactive-ir::projection` owns typed finding seeds, shared selection, and finding assembly | `dialects/solid-v2/rules` declares capabilities and maps every supported seed to 2.0 identity, severity, message, hint, and evidence |
| ESLint-era file-local checks | fact helpers live in `solid-reactive-ir::upstream_compat` | Executed unconditionally; the modules that only 1.x ran went with it |
| Shared static and fine-grained defects | `StaticDefectKind`, populated by static analysis and `upstream_compat::shared_reactivity`; contains no rule prose | Projected and worded by the 2.0 catalog; the async-tracked-scope check is omitted |

At runtime the only stable dialect id this build answers to is `solid-v2`, and
`--dialect solid-v1` is refused as unknown. In Rust, `Version::V2` always means
Solid 2.0; `Version::V1` is deliberately retained with no vocabulary behind it,
because *classifying* an installed runtime and *analyzing* it are different
questions and the `SC9013` refusal has to recognize 1.x in order to refuse it.
Other protocol/schema versions are unrelated.

Stable dialect ids also identify shipped rule manifests. ESLint’s `v2` key
is a compatibility name, not another dialect identity.

## The three dialect seams

**Vocabulary.** `solid-dialect` owns everything version-specific about
Solid's *vocabulary* — which names are primitives, which argument of a call
is its callback, which JSX tags open a boundary. The reactive engine takes a
`&dyn solid_dialect::Dialect` and asks; it does not know. This is the seam
ADR 0006 reopened so one engine could serve two dialects rather than one
engine per branch.

A vocabulary also judges the installation it analyzes, not only the major
(`Dialect::review_installation`). It names the packages whose releases decide
its release-dependent answers and where each resolves from
(`Dialect::release_owners`: for Solid 2, `solid-js` and `@solidjs/web` from the
project, `@solidjs/signals` from the installed `solid-js`'s real directory,
because `solid-js` re-exports it). Detection resolves those manifests once it
has chosen the language and hands the versions over. The answer is either
refused (`SC9013`) or a vocabulary plus a list of open gaps: empty is audited
(analyze, say nothing), anything else analyzes and adds one `SC9014` notice
naming each gap and the versions found. The vocabulary is built per answer from
the package that owns it (the store typing, `omit`'s predicate, the store
setter's root guard and `flush`'s action-step throw from signals, `dynamic`'s
options from web, `until` from `solid-js` and signals together), so a fresh
install of `solid-js@2.0.0-rc.3` over signals rc.9 gets rc.9's store answers.
Every distinct vocabulary other than the language's own is a release variant,
`dialect::RELEASE_VARIANTS`: the same catalog under an id of its own,
`solid-v2@<variant key>` (for the rc.9 triple
`solid-v2@store-root-mutable+omit-predicate+until+flush-in-action+flush-in-effect-callback+dynamic-static+store-setter-guards-roots`),
so caches and daemon sockets never mix two. A variant is registered when first
reached, and an id decodes back through `Dialect::variant_by_key`, so no table
of every variant is built; shared code never classifies a release or names what
a package owns. See `solid-dialect/src/solid_2/releases.rs`.

**Compiler.** `solid_facts::compiler::CompilerFactsProvider` is the checker's
whole view of a Solid JSX compiler: `AnalysisRequest` in, validated
`ExecutionMap` out. `solid-v2-compiler` implements it over the pinned
`solidjs-compiler` semantic trace; no other crate speaks a compiler's own
types. The analysis pipeline in `solid-facts-backend`
is generic over the trait. Callers obtain the selected adapter from the
composition bundle's compiler factory; the backend root does not re-export a
preferred dialect's concrete compiler. Their traces report execution sites
totally and compiler-established ownership regions conservatively. Component
identity composes compiler JSX call/use facts, TypeFacts aliases, AST return
shape, and an explicit 1.x compatibility convention; runtime callback
ownership requires source facts or dialect premises.

**Rules.** `solid-reactive-ir` builds a `Program` and defines the
dialect-neutral diagnostic model (`Finding`, `EvidenceStep`, `RuleMetadata`).
Its `projection` module is the single translation seam: it selects reportable
rows, turns them into a closed `FindingSeed` vocabulary, and assembles the
final `Finding`. Each rules crate is a `CatalogWording` adapter that declares
which optional tables it supports and exhaustively maps supported seeds to
rule identity, severity, message, hint, and evidence. `solid-v2-rules` owns
the 27-rule Solid 2.0 catalog, the only catalog this build ships (ADR 0110)
(`v1/<rule>` names, spanning the engine slices under 1.x vocabulary plus the
eslint-plugin-solid file-local surface). The wording duplication between
them is deliberate: a 1.x diagnostic never tells its reader to call an API
their Solid version does not have.

**Composition.** `solid_facts_backend::dialect::Dialect` bundles everything a
Solid version contributes: the compiler-provider factory, the catalog's
`solve`, rule documentation, package-boundary finding projection, and
the built-in runtime model, plus a stable `id`. Dialects register in
`dialect::ALL` and are resolved at the entry points — the CLI's `--dialect`
flag and the wasm request's optional `dialect` field, defaulting to detection
from the project's resolved `solid-js` version with `solid-v2` as the
fallback when nothing resolves — then threaded as a value through the whole
pipeline: build
functions, retained sessions, diagnostics, and the daemon. No backend code
names a dialect crate outside the registry. The dialect `id` is folded into
the compiler cache key, the retained diagnostic identity, and the daemon
socket identity, so artifacts from two dialects can never answer for each
other. `alternate_dialect_flows_through_native_pipeline` in the backend's
tests proves a non-default dialect's compiler and catalog flow end to end.
Semantic-demand planning follows the same path: imports are resolved through
the selected vocabulary's owned modules, export tables, primitives, and
reactive-source classification rather than backend-owned API spellings.

**Assembly manifest.** Each dialect’s `dialect.json` names its shipped rule
manifest. `scripts/dialect-manifests.mjs validate` checks the assembly; the
ESLint adapter reads the resulting rule manifests.

**Payload features.** `solid-facts-backend` and `solid-checker-wasm` expose
`dialect-v2`, which is the default and currently the only one. The feature owns
its registry entry, compiler adapter, and catalog dependency. The mechanism is
kept rather than inlined: it is how a second dialect is added back without the
composition boundary having to be rediscovered, and `dialect-v1` is what used to
sit beside it.

**Backend inputs.** `solid_facts_backend::wire` owns the small interface that
orchestration receives from callers and Type Facts adapters: `SourceFile`,
`SourceChange`, and `TypeFactsProvider` (plus their grouped-demand and timing
value types). The crate root re-exports that interface for compatibility;
session lifecycle, caches, incremental rebuilding, and joining stay in the
orchestration implementation.

## Type Facts producer and symbol identity

The producer is `apps/solid-typefacts`, a Rust process (and, for WASM, a
`reactor` cdylib) over the tsrs TypeScript checker, pinned by fork revision
(ADR 0365). It sits behind the versioned process protocol, so no checker node,
type or symbol crosses into solid-facts; `docs/typefacts.md` owns the protocol.

A table is a function of the program and its demands. Before it answers any
demand, the producer checks every demanded file in path order (check-first,
ADR 0367); a type descriptor is its location's type, and no checker counter
reaches a fact. Each update and each table discards the demand contributions,
async facts, source-fact memo and id maps (ADR 0364); only the compiler's
incremental program and the binder-level answers survive, the latter under
recorded `Text`, `Exports`, `Ambient` and `Options` dependencies (ADR 0369).
A dependency-source program lists the project's roots first, in `tsc`'s order,
because TypeScript's answers can depend on it (ADR 0368).

A symbol id is canonical or generation-scoped. A binder-level symbol's
`symbol:h:` id is `H(domain, key)` over a checker singleton, a global name, a
module export or its minimum declaration (ADR 0366); a union, intersection or
spread property's id is its combination and origins when they fix its type
(ADR 0371). Every other symbol (an instantiation, a `this` clone, a mapped
property) keeps a `symbol:<generation>:<n>` id and never shares a canonical
one: derived symbols are not erased to their root, because index-keyed
consumers (typed sources, descriptors and call sites by symbol) would then
apply one instantiation's type to another's (ADR 0372). The reactive IR may
therefore treat an id as naming one symbol and must not assume it names one
type.

## Dependency-source boundary

The backend selects installed runtime files for the declared target and build,
records exact source-edge refusals, and analyzes selected files with project
source. `package_requirements` supplies the boundary’s open obligations; a
missing fact is never proof of safe or unsafe package behavior. SC9005 reports
required behavior unavailable from source.

The IR owns interprocedural source summaries and reachability. Shared callback,
return and owner-requirement rows live in `summary_semantics`; they carry no
accepted-contract view. The dialect supplies reviewed runtime premises.
`CheckHost` carries runtime target/build and selected source-model state.
There is no catalog, authored tier, bundled package contract, receipt,
emission path, certification pipeline or probe authority. Type Facts protocol
78 removes the certification-only reads while retaining ordinary sessions,
module inventories, exact symbols and source facts.

Source selection assumes Vite with the Solid plugin. Runtime models additionally
state standard runtime configuration, package integrity and browser Window-realm
premises. Unknown targets, unresolved dispatch, visible mutations/providers and
unsupported namespace/member paths keep their relevant obligations open.

## Shared proof mechanics

Several rules read these proofs, so they live here. Each rule page names the
ones it uses.

### Component census

Three cases join ADR 0331's census, where a function is a proven component when
every reference to it is accounted for:

- **Audited `@solidjs/router` (ADR 0335, and ADR 0324's params).** next.41 was
  read on its own bytes against next.26's evidence (`Solid2` doc comments cite
  both): params are still decoded strings, histories and `wrapParams` as
  before, definitions copied into records, rendered by `createComponent` and
  read only for `preload` and the unexported `SERVER_ROUTE` brand; its new
  exports hand out no record. It adds `createRouter({ links })`, called with
  the router context during setup (`claims.js:42-44`), so `names_links`
  (`route_components.rs`) and `history_not_stock` (`router_params.rs`) refuse
  any config naming `links`, on every release. Two older paths also hand a
  project callable the context as `this`, in all four releases:
  `useAction(action)` runs `action.apply({ r: useRouter() }, args)`
  (`data/action.js:232-235` next.41), and Root calls `props.preload(...)` on
  props holding `routerState` (`components.jsx:13`). `router_context_as_this`
  (`router_params.rs`) withholds both premises unless every `useAction`
  argument is an arrow or the router's own action wrapper (a `const`, local
  or directly imported, needs every program reference safe), and
  `history_not_stock` unless a config `preload` is an arrow.

- **Audited TanStack routers (ADR 0334).** The rc.8 release was checked by an
  `npm pack` diff against rc.6, and rc.9 against rc.8: every route-option read
  is byte-identical, the changed files read none (rc.9's new `observe` module
  reads a matched route's `fullPath` and params), and all three load the same
  `@tanstack/router-core` (`1.171.22`). `route_install_roots` and `route_replacement_recorded`
  (`rust/crates/solid-facts-backend/src/installed_bytes.rs`) read the installed
  bytes of the router and of the router-core it loads, from the router's real
  directory. A `createRouter` key-read veto covers its four default components,
  and an anonymous inline function at one of those keys is a component
  (`execution_role/callee_roles.rs` no longer treats it as the call's callback).
- **Compound holders (ADR 0342).** `rust/crates/solid-reactive-ir/src/indexes/compound.rs`
  refuses a holder when: a stored identifier or `Object.assign` target is not a
  hoisted function declaration, an import, or a binding declared earlier at top
  level (a function running before the holder's statement would reach an
  uninitialized `const`); an `Object.assign` literal names `name`, `length`,
  `prototype`, `arguments` or `caller` (own non-writable function properties, so
  the call throws); top-level code before the holder's statement calls,
  constructs, renders JSX or evaluates a tagged template outside every function
  (a dialect primitive given only data literals, such as `createSignal(0)`, is
  the exception); its module is on an import cycle, imports one, or the project
  has an unresolved load; or a computed member write or a reflective call
  (`Reflect.set`, `Reflect.defineProperty`, `Reflect.defineProperties`,
  `Reflect.deleteProperty`, `Object.defineProperty`) may target `Object` (an
  unresolved target counts; `Object.assign(target, …)` refuses only a target
  that resolves to `Object`). Not followed: `Object.assign(F, {...})` written as
  a statement or `F.List = …` (the holder is the function itself), identifier-valued
  nested namespaces, and a holder reached through a namespace import.

### Function-valued bindings (ADR 0339)

`rust/crates/solid-reactive-ir/src/indexes/initialization.rs` decides whether a
mention runs after its declarator.

- **Same module.** A mention follows when it is in the declarator's own body
  after it; inside the function itself; in a function expression or arrow created
  after it; in an arrow or nested declaration written before it whose every
  mention follows (from the compiler's reference list, which withholds when
  missing); or in a hoisted module-level declaration that nothing running ahead of
  the declarator can call: no module-level function written before it, no
  declaration mentioned by such code or by module-level code before it, and no
  import cycle.
- **Across modules.** The census orders static imports and re-exports,
  `import x = require(…)` and `require(…)` as edges; a dynamic `import(…)` runs in
  a later job and orders nothing. These loads leave the importer possibly cyclic:
  an unresolved or ambient module (`declare module "virtual:b"`); a relative
  import that resolves to a declaration file with no runtime redirect; a package
  that resolves outside `node_modules` once realpath'd (a workspace link); a
  nonliteral load; a `require` the facts did not record (a `declare function require`
  shim); an `import.meta.glob` or `globEager` that is not provably lazy
  (lazy means no options, or an options literal whose `eager` is absent or the
  literal `false`); and a project file no analysed file carries (JavaScript
  outside `allowJs`). `require` is peeled of parentheses and TypeScript wrappers
  (`(require as any)(x)`, `require!(x)`, `require satisfies F`) before it is read
  as a load; any other mention (`(0, require)(x)`, an alias, `typeof require`)
  makes the importer possibly cyclic.
- **Other withholdings.** A `require` of a module that has, or transitively
  imports, a top-level `await` throws `ERR_REQUIRE_ASYNC_MODULE`. A direct `eval`
  in the declaring or the calling file withholds every mention, and a binding
  whose name is written (`eval` included) reaches nothing by name. A member
  mention (`A.helper()`) is withheld by its property. A function whose parameter
  list is not inert (a default that is more than a literal, a computed pattern key,
  a decorator) has its mentions withheld, and its defaults count as early mentions
  of what they reference. A name a load binds (an import-equals alias, a `require`
  result) is in its own dead zone until the load runs. The component, export and
  callback reachability roots are withheld when the root's binding may be
  written. The dialect's own packages and `node:` modules count as installed.
  Never credited: `var`, a `require()` call, a dynamic `import()`, and
  `createRequire`.

### Callback identity (ADR 0346)

- **Covering symbol.** A typefacts entity names the symbol at its demanded span's
  first byte. That symbol is the value's only when it covers the whole span:
  `symbol_covers_span` (`rust/crates/typefacts/src/lib.rs`), set by
  `symbol_span_covers` in `apps/solid-typefacts/src/nodes.rs`,
  carried by Type Facts wire schema v21 (handshake protocol 80). `EntitySymbols::exact`
  (`rust/crates/solid-reactive-ir/src/indexes.rs`) answers only for covering
  entities. `EntitySymbols::value_symbol` peels TypeScript wrappers and, when the
  peeled span is one identifier starting at the wrapper's first byte, reads the
  symbol at the wrapper's own span.
- **Callees.** `function_called_at`, `callee_symbol` and `callee_symbols` name the
  callee's identifier and nothing else: the entity at a call-valued callee
  describes the inner call. The tuple-iteration veto asks a call's
  TypeScript-resolved declaration (`resolves_to_project_function`), not the
  object-member index. A member read of a server function is uncertain only when
  TypeScript resolves no declaration for it (`member_of_server_function`).
- **Identity wrappers** (`rust/crates/solid-reactive-ir/src/indexes/identity_wrappers.rs`).
  A project function, declared or bound by an immutable `const` to a function
  literal, that returns one of its parameters on every completion by a
  straight-line return (or an arrow's expression body), returns no other value, is
  neither async nor a generator, and never otherwise references that parameter,
  its name, `arguments` or `eval`. A wrapper whose declaration is written later in
  its module names nothing, and a member written through a TypeScript wrapper is no
  write of the binding (`binding_written_through_wrappers`). The index is digested
  into the cross-file owner cache, so a wrapper changed in one file invalidates its
  callers in others.

### Invoked literals and namespace hand-outs (ADR 0347)

- **Invoked literal edge.** `invoked_function_literal` and `invocation_runs_under_owner`
  (`rust/crates/solid-reactive-ir/src/owners/owner_context.rs`) make a direct call of a
  function literal an owner edge, after the callee's parentheses and type assertions are
  peeled. `.call`/`.apply` on a literal, `new` on a plain function expression and a comma
  expression's last operand (`SequenceFact`, `rust/crates/solid-facts/src/ast/types.rs`)
  count too. Both owner passes read them: `find_missing_owners` (`owners/missing_owners.rs`)
  and `discover_owner_file` (`owners/owner_discovery.rs`). The edge is withheld for an
  instance field initializer and for JSX in the owner's own body.
- **Hand-out census.** `namespace_hand_outs` (`rust/crates/solid-reactive-ir/src/attribution.rs`)
  is built once per build by the pipeline (`SemanticLookup::set_namespace_hand_outs`). A
  hand-out is a static member read `ns.fn` used as a value, or a namespace object used
  other than by a static member (`Object.entries(ns)`, an alias, an element access,
  `export * as`). A specifier that does not resolve to the module is not counted.
- **Named reads.** `ModuleLoadConsumer` (`types.rs`) records the names a dynamic import's
  result reads: `Members` for `const { a } = await import(x)` and `(await import(x)).a`,
  and `Lazy` for `lazy(() => import(x), options)`, whose callee must be a named import of
  `lazy` from a module the dialect's export table names. Typefacts does not name the
  overloaded `lazy`, so the import binding decides. rc.13's `lazy` reads
  `mod[options.export]`, or `mod.default` (`solid-js` `dist/solid.dev.js`).
- **Seeds.** `seed_namespace_hand_outs` (`owners/owner_graph.rs`) applies the named
  hand-outs at graph assembly, so per-file fragments stay cached; it seeds
  `OWNER_CONTEXT_UNOWNED` without a proof. `seed_unnamed_hand_outs` seeds an export that
  only an unnamed escape reaches the same way, and `propagate_owner_contexts` passes no
  proof out of it. `open_world_unowned_only` (`owners.rs`) is the open-world test and
  ignores `OWNER_CONTEXT_LEAF`.
- **Leaf bits.** `OWNER_CONTEXT_LEAF_OWNED` (`owners.rs`) marks a leaf created under a
  children-capable owner; `QueuedLeaf` edges (an `onSettled` inside a leaf body) run with
  no owner. `tracked_leaf_body_context` (`owner_context.rs`) gives a `createTrackedEffect`
  body `LEAF | LEAF_OWNED` on every live path. `OWNER_CONTEXT_LEAVES_REGION` includes
  `LEAF`, since a leaf body is not region-owned. The owner gate reads the run-owned bit
  (`owner_context.rs`).

## How the Solid 1.x dialect landed

**Historical, and kept deliberately.** Solid 1.x was retired in 2026-09
([ADR 0110](../docs/decisions.md#adr-0110)) and the
crates named below no longer exist. The record stays because it is the only
worked account of what adding a *second* dialect actually costs, seam by seam —
`../docs/adding-a-dialect.md` is the forward checklist; this is the evidence
behind it. Read every crate name here as "what used to be there".

The sibling-directory shape sketched here before the 1.x dialect existed became
the shipped layout, and the plan's items resolved as follows:

1. **Compiler adapter** — `solid-v1-compiler` implements
   `CompilerFactsProvider` over the `solid-1x-compiler` fork's trace, kept at
   differential parity with the Babel compiler Solid 1.x ships, exactly as
   `solid-v2-compiler` does for 2.0.
2. **Rule catalog** — `solid-v1-rules` projects the same `Program` onto the
   1.x catalog. The backend's snapshot, diagnostics, and LSP paths needed no
   changes, as intended.
3. **Dialect selection** — both dialects register in `dialect::ALL`; the
   entry points auto-detect from the resolved `solid-js` version and accept
   an explicit `--dialect`/`"dialect"` override. Cache and session keys carry
   the dialect id.
4. **The IR's Solid 2.0 coupling** — resolved by the `solid-dialect`
   vocabulary crate (ADR 0006): the primitive names, callback positions, and
   boundary tags that used to be hardcoded 2.0 knowledge are now questions
   the engine asks of the dialect it was handed.
5. **Wire-format coupling** — still open. `CompilerOptions` in
   `solid_facts::compiler` remains shaped around JSX compiler options
   (`effect_wrapper`, `hydratable`, `static_marker`). It is part of the
   CLI/wasm request schema, so generalizing it (for example into per-dialect
   opaque options) is a protocol change to make deliberately.

## Conventions

- Infrastructure crates never depend on dialect crates, with one deliberate
  exception: `solid-facts-backend` (the composition root) and
  `solid-checker-wasm` (an entry point) wire in the current dialect.
- Dialect crates depend only on `solid-facts` and `solid-reactive-ir`.
- New version-specific compiler adapters, vocabulary, rule identities, and
  wording go under `dialects/`. An analyzer that requires private
  `solid-reactive-ir` facts may remain in that crate only when its version
  ownership is explicit in the module name and it is gated by the selected
  `Version`; the dialect catalog still owns the external finding.
- Dialect-neutral enablement lives in `RuleOptions`. It carries no
  dialect-specific option shapes today: the only rule that had one was
  `v1/prefer-classlist`, whose `classnames` list went with the 1.x catalog, and
  the nested `Solid1xRuleOptions` member went with it. The refusal it hung off
  stayed — a rule entry with any key but `enabled` is still rejected rather than
  read as defaults — so when a 2.0 rule grows an option, that arm grows a parser
  and the shared pipeline still does not learn version-specific rule names.
