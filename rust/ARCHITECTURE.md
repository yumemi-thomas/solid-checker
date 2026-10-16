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
([ADR 0110](../docs/adr/0110-the-checker-analyzes-solid-2-only.md)); the seams
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
`solid-v2@store-root-mutable+omit-predicate+until+flush-in-action+dynamic-static+store-setter-guards-roots`),
so caches and daemon sockets never mix two. The registry is built from
`Dialect::variants`, and shared code never classifies a release or names what a
package owns. See `solid-dialect/src/solid_2/releases.rs`.

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

## How the Solid 1.x dialect landed

**Historical, and kept deliberately.** Solid 1.x was retired in 2026-09
([ADR 0110](../docs/adr/0110-the-checker-analyzes-solid-2-only.md)) and the
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
