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
│   ├── solid-reactive-ir/       # reactive program IR + the Finding model
│   ├── solid-facts-backend/     # daemon, caches, snapshots, contracts, CLI
│   └── solid-checker-wasm/      # process-free Node-API/WASI entry point
└── dialects/
    └── solid-v2/                # everything Solid 2.0-specific
        ├── rules/               # solid-v2-rules: rule catalog + solve()
        └── compiler/            # solid-v2-compiler: dom-expressions adapter
```

## The two dialect seams

**Compiler.** `solid_facts::compiler::CompilerFactsProvider` is the checker's
whole view of a Solid JSX compiler: `AnalysisRequest` in, validated
`ExecutionMap` out. `solid-v2-compiler` implements it over the pinned
`dom-expressions-compiler` semantic trace; no other crate speaks the
compiler's own types. The analysis pipeline in `solid-facts-backend` is
generic over the trait.

**Rules.** `solid-reactive-ir` builds a `Program` and defines the
dialect-neutral diagnostic model (`Finding`, `EvidenceStep`, `RuleMetadata`).
`solid-v2-rules` owns the Solid 2.0 catalog — which rules exist, their codes
`SCxxxx`, severities, messages, and hints — and turns a `Program` into
findings via `solve()`.

**Composition.** `solid_facts_backend::dialect::Dialect` bundles everything a
Solid version contributes: the compiler-provider factory, the catalog's
`solve`, rule documentation, the contract-missing rule identity, and the
bundled contract set, plus a stable `id`. Dialects register in
`dialect::ALL` and are resolved by id at the entry points — the CLI's
`--dialect` flag (default `solid-v2`) and the wasm request's optional
`dialect` field — then threaded as a value through the whole pipeline: build
functions, retained sessions, diagnostics, and the daemon. No backend code
names a dialect crate outside the registry. The dialect `id` is folded into
the compiler cache key, the retained diagnostic identity, and the daemon
socket identity, so artifacts from two dialects can never answer for each
other. `alternate_dialect_flows_through_native_pipeline` in the backend's
tests proves a non-default dialect's compiler and catalog flow end to end.

## Adding a Solid 1.x dialect later

The intended shape is a sibling directory:

```
dialects/
├── solid-v1/
│   ├── rules/          # solid-v1-rules: its own catalog over the same IR
│   └── compiler/       # solid-v1-compiler: the Solid 1.x Oxc compiler
│                       # behind the same CompilerFactsProvider trait
└── solid-v2/
```

What that requires, in order of effort:

1. **Compiler adapter** — mechanical. Implement `CompilerFactsProvider` over
   the Solid 1.x Oxc compiler's trace, projecting its execution decisions
   onto `ExecutionMap`, exactly as `solid-v2-compiler` does.
2. **Rule catalog** — new crate, shared shape. Solid 1.x rules differ
   (`createResource`/`Suspense` instead of async signals and `Loading`
   boundaries, `batch`, `on`, …), but they produce the same `Finding` model,
   so the backend's snapshot, diagnostics, and LSP paths need no changes.
3. **Dialect selection** — already in place: construct the second `Dialect`
   value, list it in `dialect::ALL`, and it is selectable with
   `--dialect solid-v1` (CLI) or `"dialect": "solid-v1"` (wasm). Cache and
   session keys carry the dialect id, so retained state stays correct the
   moment two dialects coexist.
4. **Known remaining coupling** — parts of `solid-reactive-ir` still encode
   Solid 2.0 API semantics directly, most visibly `static_api.rs` (Solid 2.0
   call signatures and the SC7xxx static violations it emits) and
   `runtime_semantics.rs` (primitive behavior tables), and Solid 2.0
   primitive names appear throughout IR construction. Before a 1.x dialect
   can reuse the IR, those tables need to move behind the dialect boundary or
   become data the dialect supplies. This is the real work item; the seams
   above were cut so it can happen without touching the backend.
5. **Wire-format coupling** — `CompilerOptions` in `solid_facts::compiler` is
   shaped around dom-expressions options (`effect_wrapper`, `hydratable`,
   `static_marker`). It is part of the CLI/wasm request schema, so
   generalizing it (for example into per-dialect opaque options) is a
   protocol change to make together with dialect selection.

## Conventions

- Infrastructure crates never depend on dialect crates, with one deliberate
  exception: `solid-facts-backend` (the composition root) and
  `solid-checker-wasm` (an entry point) wire in the current dialect.
- Dialect crates depend only on `solid-facts` and `solid-reactive-ir`.
- New Solid-version-specific behavior goes under `dialects/`, never into
  `crates/`.
