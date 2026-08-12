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
│   ├── solid-dialect/           # the vocabulary both versions answer through
│   ├── solid-reactive-ir/       # reactive program IR + the Finding model
│   ├── solid-facts-backend/     # daemon, caches, snapshots, contracts, CLI
│   └── solid-checker-wasm/      # process-free Node-API/WASI entry point
└── dialects/
    ├── solid-v1/                # everything Solid 1.x-specific
    │   ├── rules/               # solid-v1-rules: the 1.x catalog
    │   └── compiler/            # solid-v1-compiler: the 1.x compiler adapter
    └── solid-v2/                # everything Solid 2.0-specific
        ├── rules/               # solid-v2-rules: rule catalog + solve()
        └── compiler/            # solid-v2-compiler: dom-expressions adapter
```

## Version ownership at a glance

“Shared” means the algorithm is common, not that the two runtimes behave the
same. Shared code asks the selected vocabulary and receives the 1.x or 2.0
answer; it must not switch on an API spelling itself.

| Concern | Shared module / seam | Solid 1.x ownership | Solid 2.0 ownership |
| --- | --- | --- | --- |
| Syntax, TypeScript facts, reactive IR, caches | `crates/solid-facts`, `crates/solid-reactive-ir`, backend infrastructure | No separate implementation | No separate implementation |
| Primitive names, callback semantics, ownership, boundaries, import modules | `crates/solid-dialect::Dialect`; consumers read one `CallbackSemantics` descriptor per call argument | `solid-dialect/src/solid_1x.rs` (`Solid1x`, `Version::V1`) | `solid-dialect/src/solid_2.rs` (`Solid2`, `Version::V2`) |
| JSX compiler facts | `CompilerFactsProvider` | `dialects/solid-v1/compiler` | `dialects/solid-v2/compiler` |
| Rule identity, severity, message, hint | dialect-neutral `Finding` | `dialects/solid-v1/rules`; external names are `v1/<rule>` | `dialects/solid-v2/rules`; external names are unprefixed |
| ESLint-era file-local checks | fact helpers live in `solid-reactive-ir::upstream_compat` | `solid1x_*` modules; executed only for `Version::V1` | Not executed |
| Shared static and fine-grained defects | `StaticDefectKind`, populated by static analysis and `upstream_compat::shared_reactivity`; contains no rule prose | Projected and worded by the 1.x catalog | Projected and worded by the 2.0 catalog; the async-tracked-scope check is omitted |
| Bundled runtime contracts | shared contract schema and resolver | `solid-js-v1.json` / `solid-js-1x.json`, package `solid-js@1.9.14` | `solid-js.json` + `solidjs-web.json`, packages `solid-js` and `@solidjs/web` at `2.0.0-beta.31` |

At runtime the stable dialect ids are `solid-v1` and `solid-v2`. In Rust,
`Version::V1` always means Solid 1.x and `Version::V2` always means Solid 2.0;
other protocol/schema versions are unrelated.

## The three dialect seams

**Vocabulary.** `solid-dialect` owns everything version-specific about
Solid's *vocabulary* — which names are primitives, which argument of a call
is its callback, which JSX tags open a boundary. The reactive engine takes a
`&dyn solid_dialect::Dialect` and asks; it does not know. This is the seam
ADR 0006 reopened so one engine could serve two dialects rather than one
engine per branch.

**Compiler.** `solid_facts::compiler::CompilerFactsProvider` is the checker's
whole view of a Solid JSX compiler: `AnalysisRequest` in, validated
`ExecutionMap` out. `solid-v2-compiler` implements it over the pinned
`dom-expressions-compiler` semantic trace, and `solid-v1-compiler` over the
same crate name from the pinned `solid-1x-compiler` fork; no other crate
speaks a compiler's own types. The analysis pipeline in `solid-facts-backend`
is generic over the trait.

**Rules.** `solid-reactive-ir` builds a `Program` and defines the
dialect-neutral diagnostic model (`Finding`, `EvidenceStep`, `RuleMetadata`).
Each rules crate owns its version's catalog — which rules exist, their codes
`SCxxxx`, severities, messages, and hints — and turns a `Program` into
findings: `solid-v2-rules` for the 34-rule Solid 2.0 catalog,
`solid-v1-rules` for the 38-rule 1.x catalog (`v1/<rule>` names, spanning the
engine slices under 1.x vocabulary plus the eslint-plugin-solid file-local
surface). The wording duplication between them is deliberate: a 1.x
diagnostic never tells its reader to call an API their Solid version does
not have.

**Composition.** `solid_facts_backend::dialect::Dialect` bundles everything a
Solid version contributes: the compiler-provider factory, the catalog's
`solve`, rule documentation, the contract-missing rule identity, and the
bundled contract set, plus a stable `id`. Dialects register in
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

## How the Solid 1.x dialect landed

The sibling-directory shape sketched here before the 1.x dialect existed is
now the shipped layout, and the plan's items resolved as follows:

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
   `solid_facts::compiler` remains shaped around dom-expressions options
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
