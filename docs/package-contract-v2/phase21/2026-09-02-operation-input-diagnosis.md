# The `operation-input is unsupported` refusal class

Read-only diagnosis, worktree `codex/phase19a-authenticated-proof-policy` at `c2441245`.
All measurements were made with a debug checker built by `make build-checker-debug`
(which supplies `SOLID_TYPEFACTS_CERTIFICATION_SHA256` /
`SOLID_TYPEFACTS_SOURCE_MANIFEST_SHA256`; a bare
`cargo build -p solid-facts-backend --bin solid-checker-rust` produces a binary that
refuses with *"verifier build has no configured Type Facts executable digest"* and
tells you nothing about this class).

All temporary diagnostic patches described in §2 have been reverted;
`git diff --stat` is empty and `git status --short` shows only the pre-existing
untracked `packages/cli/solid-reactivity.json.refusals.json`.

---

## 0. Correction to the premise: this is **two** blockers, not one

The message is emitted by a single helper, so the six rows look like one class, but
they fail at two different call sites in two different demand families:

`rust/crates/solid-facts-backend/src/contract_certification/type_facts.rs:2944`

```rust
fn parameter_source(value: &ValueShape) -> Result<ValueSource, TypeFactsCertificationError> {
    match value {
        ValueShape::Parameter { index, path } => Ok(ValueSource::Parameter { … }),
        _ => Err(TypeFactsCertificationError::UnsupportedDemand {
            demand: "operation-input".into(),
            reason: "implementation census only binds exact parameter-rooted operation inputs".into(),
        }),
    }
}
```

Three fatal call sites, all reached for a non-`Parameter` input:

| site | family | reached for |
| --- | --- | --- |
| `type_facts.rs:3597` in `require_operation_evidence` (`OperationKind::Read` arm, fn at `:3573`) | `operation-reachability` **and** `operation-cardinality` (both callers at `:2698`, `:2722`) | every `read` operation whose `inputs[0]` is not `Parameter` |
| `type_facts.rs:3959` in `require_operation_recursive_subject` (fn at `:3925`) | `recursive-value-shape` | every `RecursiveValue` demand rooted at `OperationInput` whose input is not `Parameter` |
| `type_facts.rs:4024` in `require_operation_recursive_signature` (fn at `:4004`) | `recursive-value-shape` (fall-through after the block above) | unreachable in practice — `:3959` errors first with `?` |

`type_facts.rs:1745` also calls it, but with `.ok()`, so it is not fatal (it only
computes a callable-path depth bound).

Consequences:

* An **`invoke`** operation with a non-`Parameter` input loses **one** demand
  (`recursive-value-shape`). Its `operation-reachability` /
  `operation-cardinality` demands take the `OperationKind::Invoke` arm at
  `:3583-3591`, which reads `callback.from` — always
  `ValueSource::Parameter` (built at `inferred_contract.rs:127-141`) — so they pass.
* A **`read`** operation with a non-`Parameter` input loses **three** demands.
* `require_operation_recursive_signature`'s `else { return Err(open("recursive
  input is not parameter-rooted")) }` at `:4025-4027` (the `else` guard whose `Err` is at `:4026`) is dead code.

Everything below distinguishes the two.

---

## 1. Exact demand inventory for the six rows

### 1.1 What actually gets inventoried

`inventory_export_facts` →
`rust/crates/solid-reactive-ir/src/contract_semantics/certification.rs:559-571`
walks `operation.inputs` and calls `inventory_value_shape` (`:645`) with
`root = ValueRoot::OperationInput { operation, index }`, `path = ValuePath::default()`.
`inventory_value_shape` returns early for `ValueShape::Unknown` (`:653-655`), and
`ValueShape::Reactive { .. }` is a **leaf** in its `match` (`:709-720`), so each
non-`Parameter`, non-`Unknown` input in these six rows produces **exactly one**
`RecursiveValue` demand subject with `path = []` and
`callable = DemandedCallability::Unknown` (`recursive_value_callability` at
`:745-764` maps `Reactive` to `Unknown`).

Every non-parameter input in all six rows is the same shape:

```json
{ "kind": "reactive", "role": "accessor" }
```

i.e. `ValueShape::Reactive { role: ReactiveRole::Accessor, resource: None,
capabilities: KnowledgeSet::Unknown }` (verified from the emitted documents *and*
from a diagnostic `{input:?}` print of the live `ValueShape`). There is **no**
`Plain`, `Object`, `Callable`, `Store`, `Tuple` or `Choice` input anywhere in these
six rows. `mapArray`'s `callback-1` input 0 is the literal string `"unknown"`
(`ValueShape::Unknown`), which is skipped by the early return and demands nothing.

### 1.2 Per-row inventory

Counted from the emitted contract documents by
`<scratch>/opinput/inv.py` (enumerates entrypoint × case × export × operation ×
input index), and cross-checked against the live diagnostic prints.

**Mechanism A — callback argument (`invoke`).** 1 demand each
(`recursive-value-shape` only).

| row | artifact case | export | operation | idx | callability | runtime source |
| --- | --- | --- | --- | --- | --- | --- |
| `@solid-primitives/marker@0.2.2\|solid1\|only` | `097ee468…` (1 case) | `createMarker` | `callback-0` (`invoke`, `queued`, `untracked`, `0..many`/call) | 0 | `Unknown` | `dist/index.js` bytes 2661‥2675: `mapMatch(text)`, where `const [text, set] = createSignal(matchText)` (bytes 2599‥2622) |
| `@solid-primitives/marker@2.0.0-next.2\|solid2\|floor` and `\|head` | `58b6679b…` (1 case, identical for both probes — same `closureSha256 c54bd02b…`) | `createMarker` | `callback-0` (same attributes) | 0 | `Unknown` | same expression; `createSignal(matchText, { ownedWrite: true })`, wrapped one level deeper in `runWithOwner(owner, () => createRoot(dispose => …))` |
| `solid-js@1.9.14\|solid1\|only` | 6 cases each (`.`×4, `./dist/dev.js`, `./dist/solid.js`) | `mapArray` | `callback-1` (`invoke`) | **1** | `Unknown` | `dist/solid.js:1202` `return mapFn(newItems[j], s)` inside `function mapper(disposer)`, where `const [s, set] = createSignal(j)` (`:1200`) |
| `solid-js@1.9.14\|solid1\|only` | 6 cases | `indexArray` | `callback-1` (`invoke`) | **0** | `Unknown` | `dist/solid.js:1261` `return mapFn(s, i)` inside `function mapper(disposer)`, where `const [s, set] = createSignal(newItems[i])` (`:1259`) |

Both marker versions produce byte-identical summaries
(`summary-b7b44024…` / `summary-ae35fff9…`), so the two 2.0.0-next.2 probes are one
semantic case.

**Mechanism B — reactive read (`read`).** 3 demands each
(`recursive-value-shape` + `operation-reachability` + `operation-cardinality`).

| row | export | operation | value path | callability | runtime source |
| --- | --- | --- | --- | --- | --- |
| `@solid-primitives/timer@1.4.5-next.1\|solid2\|floor` and `\|head` (case `5b9787fb…`, identical for both — `closureSha256 4bcf2417…`) | `createPolled` | `read-0` (`read`, `same-stack`, `untracked`, `0..many`/call) | `[]` | `Unknown` | `dist/index.js` bytes 4105‥4116 (`createSignal(depSignal(), options)`) **and** bytes 4148‥4159 (`() => depSignal()` inside `createEffect`), where `const [depSignal] = createSignal((prev) => { tick(); return fn(…) })` (bytes 3978‥4061) |
| same | `createIntervalCounter` | `read-0` (same attributes) | `[]` | `Unknown` | **no expression in this export's own body.** `createIntervalCounter`'s implementation census has exactly one call, `createPolled(…)` at bytes 4655‥4709. The row is `createPolled`'s row composed interprocedurally |
| `solid-js@1.9.14\|solid1\|only` | `ErrorBoundary`, `For`, `Index`, `Show`, `Switch`, `Suspense`, `catchError`, `children`, `createComputed`, `createDeferred`, `createEffect`, `createMemo`, `createReaction`, `createRenderEffect`, `createSelector`, `mergeProps`, `onMount` (6 cases each) and `createComponent` (3 cases) | `read-0` | `[]` | `Unknown` | `dist/solid.js:777` `track();` inside `c.fn = x => { track(); … }` in `createComputation`, where `const [track, trigger] = createSignal(undefined, { equals: false })` (`:764`). Composed into all 18 exports interprocedurally |
| `solid-js@1.9.14\|solid1\|only` | `createResource` (6 cases) | `read-0` | `[]` | `Unknown` | `error()` at `dist/solid.js:327` (inside `function read()`), also `:388` and `:399`, from `const [error, setError] = createSignal(undefined)` (`:281`) |
| same | `createResource` | `read-1` | `[]` | `Unknown` | `state()` at `dist/solid.js:385` and `:392`, from `const [state, setState] = createSignal(resolved ? "ready" : "unresolved")` (`:285`) |
| same | `createResource` | `read-2` | `[]` | `Unknown` | `track()` at `dist/solid.js:331` inside the `createComputed(() => { track(); … })` in `read()`, from `const [track, trigger] = createSignal(undefined, { equals: false })` (`:282`) |

The read identities were obtained by a temporary print of `ContractReactiveRead
{ kind, label }` at the exact point `inferred_contract.rs:164-172` discards them
(`kind=accessor label=track` / `error` / `state` / `depSignal`); §3(a) explains why
that label never reaches the certifier.

### 1.3 Demand-instance totals

| row | non-parameter inputs | blocked demand instances | mechanisms |
| --- | --- | --- | --- |
| marker 0.2.2 \| solid1 \| only | 1 | 1 | A |
| marker 2.0.0-next.2 \| solid2 \| floor | 1 | 1 | A |
| marker 2.0.0-next.2 \| solid2 \| head | 1 | 1 | A |
| timer 1.4.5-next.1 \| solid2 \| floor | 2 | 6 | B |
| timer 1.4.5-next.1 \| solid2 \| head | 2 | 6 | B |
| solid-js 1.9.14 \| solid1 \| only | 135 | **381** (123 reads × 3 + 12 invokes × 1) | A + B |

For solid-js those 381 sit inside a plan of 936 `recursive-value-shape`, 475
`operation-reachability` and 475 `operation-cardinality` demands over 31 artifact
cases.

---

## 2. True ceiling: 5 of 6 rows certify

**Temporary patch (applied, measured, reverted).** Two clearly-marked
`// TEMPORARY DIAGNOSTIC` blocks:

1. In `require_operation_recursive_subject`, immediately after resolving the
   operation and before `.and_then(parameter_source)?`: when
   `operation.inputs[index]` is not `ValueShape::Parameter`, push a site
   `diagnostic-skip:non-parameter-operation-input:<export>:<op>:<index>` and
   `return Ok(())`.
2. In `require_operation_evidence`'s `OperationKind::Read` arm, before
   `.and_then(parameter_source)?`: when `inputs[0]` is not
   `ValueShape::Parameter`, push `diagnostic-skip:non-parameter-read-input:<op>`
   and `return Ok(())`.

Both were gated to log the demand (and, under a second env var, the whole
`implementation.calls` census) to a file. Nothing else was changed. Patch 1 alone
was enough for marker; patch 2 was required for timer and solid-js, which is how
the two-blocker structure in §0 was discovered.

Command per run (six `--probe` flags in one invocation):

```sh
SOLID_CHECKER_NATIVE_BIN=$PWD/rust/target/debug/solid-checker-rust \
SOLID_TYPEFACTS_BIN=$PWD/bin/solid-typefacts \
bun scripts/ecosystem-benchmark/run.mjs --timeout 600 --attempt-certification --keep-temp …
```

### Baseline (unpatched, HEAD) — all six refused

All six: `stage=witness-acquisition`, `owner=certifier`, `demandId=null`,
`family=null`, reason
`… Type Facts demand operation-input is unsupported: implementation census only
binds exact parameter-rooted operation inputs`
(prefixed `policy-2 proof finalization failed` for the five single-case rows and
`policy-2 case-set finalization failed` for solid-js).

### With the blocker skipped

| row | outcome | next first refusal |
| --- | --- | --- |
| `@solid-primitives/marker@0.2.2\|solid1\|only` | **certified** (1 demand skipped) | — |
| `@solid-primitives/marker@2.0.0-next.2\|solid2\|floor` | **certified** (1) | — |
| `@solid-primitives/marker@2.0.0-next.2\|solid2\|head` | **certified** (1) | — |
| `@solid-primitives/timer@1.4.5-next.1\|solid2\|floor` | **certified** (6) | — |
| `@solid-primitives/timer@1.4.5-next.1\|solid2\|head` | **certified** (6) | — |
| `solid-js@1.9.14\|solid1\|only` | **still refused**, `stage=witness-acquisition` | `Type Facts demand sha256:6a05e5b6fd79c4a55e16e73b2923a251e2d4f1243da5118bef2a8e54f979993d is locally open: argument-binding (artifact-case:331dfa4929983278c23d6479ad3858531982d1317cd6b3745f89c7ac26272ade:createReaction): callback parameter has no exact direct-call or resolved-argument flow` — an unrelated blocker, hit after 37 skips in the first artifact case |

So **this blocker alone gates 5 of the 6 rows**, and solid-js has at least one more
independent blocker behind it (`createReaction`'s `argument-binding`; solid-js
plans 183 `argument-binding` demands, so more may follow).

### Caveat the ceiling number hides

The skip is *not* a proof, and one of the five rows would **not** certify under a
sound rule confined to the export's own census: `createIntervalCounter`'s
`read-0` (§1.2, mechanism B) has no witnessing expression in
`implementation.calls` at all — the census is the single call
`createPolled(timeout, options)`. So the honest reading is:

* marker ×3: certify with a sound argument-provenance fact (mechanism A).
* timer ×2: `createPolled` certifies; `createIntervalCounter` needs
  interprocedural composition on top (§4).
* solid-js: blocked behind `argument-binding` regardless.

---

## 3. Evidence available today

### 3(a) How the IR derives `reactive/accessor` — and what it throws away

The `Reactive { role: Accessor }` in a contract document is **not** a claim that a
`createSignal` accessor was seen. It is the *residue of a discarded identity*,
produced twice over:

**Mechanism B (reads).**

1. `rust/crates/solid-reactive-ir/src/interproc.rs:230-292`
   `discover_typed_accessors` walks `file.ast.calls`; for each callee it takes the
   project index's `type_descriptor` and asks
   `solid_accessor_declaration(descriptor, dialect)`
   (`rust/crates/solid-reactive-ir/src/owners.rs:2606-2613`), which is
   `descriptor.alias_declarations.iter().find(|d| dialect.type_role(descriptor.origin_module, d.name) == Some(TypeRole::Accessor))`.
   The evidence is therefore a **Type Facts alias declaration plus origin module**,
   matched against the dialect's type-export index — not a name, not a text match.
   It emits a `SummaryRead { symbol: "typed:<path>\0<start>\0<end>", display: <callee
   source text>, kind: Some("accessor"), declaration: <the `Accessor` alias
   declaration's location>, origin: <call location>, origin_context }`.
2. `rust/crates/solid-reactive-ir/src/contracts.rs:1191-1210`
   (`contract_export_function`) projects each `SummaryRead` to
   `ContractReactiveRead { kind: read.kind.unwrap_or("accessor"), label: <display or
   bundled-return label>, parameter: None, path: None }`, **deduplicated by
   `(kind, label)`**. This is where the read's origin location, its resolved
   declaration, and its symbol are dropped; only a display string survives.
3. `rust/crates/solid-facts-backend/src/inferred_contract.rs:150-173`
   (`normalize_export`) turns `(parameter: None, path: _)` into
   `ValueShape::Reactive { role: Accessor, resource: None, capabilities: Unknown }`.
   The `label` is dropped here. `resource: None` and `capabilities: Unknown` are
   literal defaults, not observations.
4. `rust/crates/solid-reactive-ir/src/contracts.rs:206-227`
   (`project_reactive_reads`, the `inputs.first()` the brief points at) is the
   **inverse** projection — v2 semantics back to the legacy `ContractReactiveRead`
   for accepted-contract ingestion. `Some(ValueShape::Reactive { .. }) =>
   ContractReactiveRead { kind: "accessor", label: "normalized reactive read",
   parameter: None, path: None }`. Note the label is a fixed placeholder: even the
   display string does not round-trip.

**Mechanism A (callback arguments).**

1. `rust/crates/solid-reactive-ir/src/interproc.rs:551-574`
   `callback_argument_contracts(file, call, entities, accessors)`: for each written
   argument of the invocation, resolve the exact symbol at the argument's span
   through `entities` (`EntitySymbols`), then look it up in
   `accessors: &HashMap<SymbolId, (SymbolId, Location)>`. A hit yields
   `ContractReturn { kind: "accessor", label: <display symbol>, .. }`; a miss yields
   `None`; trailing `None`s are popped. So the fact is a **resolved local symbol
   that source discovery already classified as an accessor**, together with its
   declaration `Location` — both discarded.
   The `accessors` map is populated by `source_discovery.rs` (`:849`, `:1004`,
   `:113-116`) from dialect primitives and accepted contracts.
2. `inferred_contract.rs:268-313` `callback_operation` maps each argument through
   `return_shape` (`:445-497`), where `"accessor" => ValueShape::Reactive { role:
   Accessor, resource: None, capabilities: Unknown }`.

**Net effect:** the IR knows the exact accessor symbol and its declaration site; the
schemaVersion-1 wire shape can carry neither, so the certifier receives a shape
that says "some accessor" and cannot address it.

Note also that the pipeline is **not** the `proposal_generation::construct_proposal`
path (that function has no non-test caller). Certification consumes the emitted
`NormalizedContract` through
`certification.rs:298-345` `inspect_candidates`, so the demand inventory really is a
pure function of the emitted document, which is why §1.2 can be computed from the
`*.json` files.

### 3(b) What the Type Facts producer already computes

`rust/crates/typefacts/src/invocation.rs:578-592` /
`apps/solid-typefacts/internal/typefacts/invocation.go` (`ImplementationValueSource`)
and `schema/typefacts-v1.schema.json:562-574`:

```
ImplementationValueSource { path: Vec<PathSegment>, kind: DirectCallable | CallResult,
                            target, target_name, target_module, target_path: Vec<PathSegment> }
```

Produced today **only** for return expressions, by
`apps/solid-typefacts/internal/typefacts/tsgo/export_value_transcripts.go:513-580`
`returnValueSourcesLocked(expression)`, and attached to
`ReturnSite.sources` (`invocation.rs:683-700`; Go `ControlFlowCensus`).
Its walk:

* peel `ParenthesizedExpression`;
* `ArrayLiteralExpression` → recurse with a `PathSegmentTuple{index}` appended;
* `ArrowFunction` / `FunctionExpression` → `DirectCallable`;
* `CallExpression` → `CallResult` with `implementationCallTargetLocked(node.Expression())`;
* `Identifier` whose symbol has a declaration that is a `BindingElement` inside an
  `ArrayBindingPattern` whose `VariableDeclaration.Initializer` is a
  `CallExpression` → `CallResult` with `TargetPath = [Tuple{index}]`
  (`:548-577`).

That last arm is *exactly* the provenance both mechanisms need:
`const [text, set] = createSignal(matchText)` → `text` resolves to
`CallResult{ targetName: "createSignal", targetModule: "solid-js", targetPath: [tuple 0] }`.

**Judgement: yes, the same tracer answers both questions, unchanged.**

* **(i) argument values.** `implementationCallCensusLocked`
  (`export_value_transcripts.go:346-431`) already walks every argument and calls
  `parameterValueSourceLocked(argument, bySymbol)` for the slots
  `exactArgumentSlots` admits. Calling `returnValueSourcesLocked(argument)` beside
  it, per slot, needs no new resolution machinery. Verified live for marker:
  the census row for `mapMatch(text)` (bytes 2661‥2675) has
  `argument_parameters = [None]` and no other argument provenance, while the
  sibling row for `createSignal(matchText)` (bytes 2599‥2622) already carries
  `target_name=createSignal target_module=solid-js` and its resolved declaration
  in `node_modules/solid-js/types/reactive/signal.d.ts`.
* **(ii) callee value.** `implementationCallTargetLocked` already resolves the
  callee to a canonical symbol, an exported name, an import module and a
  `ResolvedDeclaration`. For `depSignal()` the live census shows
  `target_name=depSignal target_module= (empty) decl=(…/timer/dist/index.js, 3965, 3974)`
  — i.e. the declaration of the `BindingElement` itself. The missing step is
  precisely `returnValueSourcesLocked(node.Expression())`, whose identifier arm
  would turn that into `CallResult{createSignal, solid-js, [tuple 0]}`.

Known gaps in that tracer, all of which matter to a design:

* No `ObjectLiteralExpression` arm — object-property paths are not traced.
* Only `BindingElement` in an `ArrayBindingPattern`. A plain
  `const c = createMemo(fn)` (a bare accessor, not a tuple) yields **nothing**,
  because its declaration is a `VariableDeclaration`, not a `BindingElement`.
  That is a real hole for Solid: `createMemo`, `createDeferred`, `createSelector`
  all return a bare accessor.
* The callee resolution is by symbol, so a computed or non-identifier initializer
  callee produces `target == ""` and no source at all. `dist/solid.js:280`
  `const [value, setValue] = (options.storage || createSignal)(options.initialValue)`
  is exactly this case and must stay unproven.
* `target_module` is the **written import specifier text**
  (`importedAliasIdentity` → `owner.AsImportDeclaration().ModuleSpecifier.Text()`,
  `export_value_transcripts.go:476-510`), not a resolved package identity, and it
  is empty for a local declaration.

### 3(c) What the consumer already has for the return side

`type_facts.rs:4104-4162` `require_return_callable_source`:

```rust
typefacts::ImplementationValueSourceKind::CallResult => {
    source.target_module.as_ref() == "solid-js"
        && !source.target.is_empty()
        && source.target_path.len() == 1
        && source.target_path[0].kind == PathSegmentKind::Tuple
        && source.target_path[0].index.is_some_and(|index|
             solid_dialect::unambiguous_callable_result_tuple_item(&source.target_name, index))
}
```

with a `Reachability::Unknown` veto on *any* return site and a requirement that
**every** reachable return site carries a matching source.

Dialect result-shape surface in `rust/crates/solid-dialect/src/lib.rs`:

| function | line | answers |
| --- | --- | --- |
| `unambiguous_callable_result_tuple_item(name, index)` | 285 | free fn. `true` only when every dialect that canonically exports `name` agrees; the body is `Primitive::CreateSignal => matches!(index, 0 \| 1)`, `_ => false`. **Callability**, not accessor-ness, and `createSignal` only |
| `unambiguous_callable_type(origin_module, name)` | 271 | type export is `Accessor`/`Setter` in every dialect that knows it |
| `Dialect::type_role(origin_module, name) -> Option<TypeRole>` | 1189 | `Accessor \| SourceAccessor \| Resource \| InitializedResource` → `TypeRole::Accessor`; gated on `export_modules(name, ExportPosition::Type)` containing the module |
| `Dialect::creates_reactive_source(primitive)` | 833 | *whether* a call produces a source (accessor, store, or tuple containing one) |
| `Dialect::returns_store(primitive)` | 907 | *which kind* — store vs accessor |
| `Dialect::callback_accessor_parameters(primitive, argument)` | 888 | which of a primitive callback's parameters are accessors (this is what already encodes `mapArray`/`indexArray`) |
| `Dialect::children_accessor_parameters(primitive, key)` | 878 | same for JSX children callbacks |

**There is no per-slot accessor-result table.** The closest things:

* `creates_reactive_source` + `returns_store` together say "this primitive
  produces an accessor" but carry **no slot**, so they cannot distinguish
  `createSignal()[0]` from `[1]`.
* The bundled contracts *do* carry a single-value accessor result:
  `pkg/contracts/bundled/solid-v1/solid-root-browser-production.json`'s
  `createMemo` summary has `operations[].kind == "return"` with
  `output: {"kind":"reactive","role":"accessor"}`. But its `createSignal` summary
  is `{"call":{"callbacks":[],"closed":["callbacks","reads","creates","returns"],
  "creates":[],"reads":[],"returns":[]},"shape":"callable"}` — an **empty and
  *closed*** returns claim, i.e. a negative claim, because the v1 contract format
  cannot express a tuple. A design must not read that as "createSignal returns
  nothing reactive". `pkg/contracts/bundled/solid-v2/` splits `solid-js.json` and
  `solidjs-signals.json` with the same shape limitation.

**Dialect module identity.** `export_modules` is implemented per dialect:
`solid-dialect/src/solid_1x.rs:801-808` over `exports::solid_v1_solid_js` only;
`solid-dialect/src/solid_2.rs:820-838` over `exports::solid_v2_solid_js` **plus**
`exports::solid_v2_solidjs_web`. There is **no `@solidjs/signals` module** in the
export index (`rg '@solidjs/signals' rust/crates/solid-dialect/src` hits only
doc comments). So:

* `solid-js` 2.0.0-rc.x re-exports `createSignal`, and both marker 2.0.0-next.2 and
  timer 1.4.5-next.1 import from `"solid-js"` — the literal `== "solid-js"` check
  in `require_return_callable_source` happens to hold for these rows.
* A package importing `createSignal` from `@solidjs/signals` directly (the
  `@solidjs/signals@2.0.0-rc.3` probe rows exist in the manifest) would have
  `target_module == "@solidjs/signals"` and would be refused by that literal, and
  `type_role`/`export_modules` would not resolve it either.
* **The hardest case is solid-js itself.** In `solid-js@1.9.14/dist/solid.js`,
  `createSignal` is declared locally (`function createSignal(value, options)` at
  `:198`); the `track`/`error`/`state` accessors that produce all 123 read demands
  come from that local function. `target_module` would be `""`. So the 123 solid-js
  read demands cannot be discharged by any `target_module`-based rule at all; they
  need the audited-dialect-artifact's own export identity (the resolved
  `Declaration` path lands inside the package under certification).

---

## 4. Design proposal (no implementation)

### 4.1 Producer facts

Two additive fields on `ImplementationCall`, both filled by the existing
`returnValueSourcesLocked` (Go `apps/solid-typefacts/internal/typefacts/tsgo/export_value_transcripts.go`,
census builder at `:346`; Rust mirror `rust/crates/typefacts/src/invocation.rs:360`):

```
argumentSources: Vec<Vec<ImplementationValueSource>>   // one entry per written argument slot
calleeSources:   Vec<ImplementationValueSource>        // for the callee expression
```

Semantics and rules:

* **One entry per written argument slot**, parallel to the existing
  `argumentParameters`, and subject to the *same* `exactArgumentSlots` gate: a slot
  a spread has displaced gets an empty list, never a traced one.
* **Absence is never authority.** An empty list means "the producer traced
  nothing", never "this argument is not an accessor" and never "this argument is
  plain". Both fields must be documented that way in the Go struct and the Rust
  struct, and every consumer must fail closed on empty.
* **`calleeSources` is a value trace, not a resolution.** It coexists with
  `target`/`targetName`/`targetModule`/`declaration`/`calleeParameter`; it does not
  replace them, and a consumer that wants "the callee is parameter N" keeps using
  `calleeParameter`.
* **Depth bound.** Reuse `path`/`targetPath` `maxItems: 8`
  (`schema/typefacts-v1.schema.json:566-572`) and the existing
  `typefacts::MAX_INVOCATION_CALLABLE_DEPTH` check at `type_facts.rs:1745-1762`.
* **Cycle bound.** `returnValueSourcesLocked` recurses only *into* array-literal
  elements and *once* through a binding-element declaration; it never re-enters the
  identifier arm from a `CallResult`. Keep that: exactly one declaration hop, so no
  cycle is reachable. If the identifier arm is ever extended to plain
  `VariableDeclaration` initializers (needed for `createMemo`, see §4.6), bound the
  hop count explicitly at 1 and refuse a declaration with more than one
  `Declarations` entry (the current arm already iterates all declarations and
  returns on the first match — that should become "refuse when more than one
  declaration exists", so a redeclared binding proves nothing).

### 4.2 Protocol and schema footprint

`ImplementationCall` is CBOR/JSON with `deny_unknown_fields` on the Rust side
(`invocation.rs:358-360`), so **any** added field is a break in both directions:

* `rust/crates/typefacts/src/v3.rs:54` `TYPE_FACTS_HANDSHAKE_PROTOCOL: 11 → 12`
  and `apps/solid-typefacts/internal/typefacts/protocolv3.go:21`
  `TypeFactsHandshakeProtocol = 11 → 12` (plus its test at
  `protocolv3_test.go:37`).
* `schema/typefacts-v1.schema.json`: two new properties under
  `$defs.implementationCall` (`:583-600`), referencing the existing
  `$defs.implementationValueSource`; then `TYPE_FACTS_SCHEMA_SHA256`
  (`v3.rs:38`, currently
  `sha256:b9f4c20081ad2a9ac81502514d296ac229c3301ef960ea3b745d5eaad5b31d51`)
  is regenerated — it is asserted against the file at `v3.rs:1926-1929`.
* Go struct fields with `cbor:"argumentSources,omitempty"` /
  `cbor:"calleeSources,omitempty"` in `invocation.go`.
* `bin/solid-typefacts` rebuilt via `scripts/build-typefacts.sh`; the
  source-manifest identity in `scripts/typefacts-source-identity.mjs` already
  includes `schema/typefacts-v1.schema.json`, so the stamp moves and
  `verify-delta`'s `build-typefacts` gate escalates as designed.
* No table-schema (`TYPE_FACTS_TABLE_SCHEMA_V18`) change: invocation transcripts
  do not travel through the compact table encoding.

### 4.3 Dialect-seam additions

`rust/crates/solid-dialect` owns this; nothing about accessor-ness may live in the
IR or the certifier.

```rust
/// The reactive role of one slot of what `name` returns, when every dialect that
/// canonically exports `name` agrees. `None` means "no dialect answer" and is
/// never "not reactive".
pub fn unambiguous_reactive_result_slot(name: &str, slot: ResultSlot) -> Option<ReactiveRole>;

pub enum ResultSlot { Whole, TupleItem(usize) }
```

Implemented per dialect (`solid_1x.rs`, `solid_2.rs`) as a table beside the
existing `creates_reactive_source` / `returns_store` / `callback_accessor_parameters`,
built from the same audited export census the `exports/` tables come from — with
per-version rows, because the sets differ:

* 1.x: `createSignal` → `TupleItem(0) = Accessor`, `TupleItem(1) = Setter`;
  `createMemo` / `createDeferred` / `createSelector` → `Whole = Accessor`;
  `createResource` → `TupleItem(0) = Accessor`; `createStore`/`createMutable` →
  store, **not** accessor; `useTransition` → `TupleItem(0) = Accessor`.
* 2.0: the audited 2.0 vocabulary only — `createProjection` is a store,
  `createOptimistic`/`action` need their own review, and no 1.x row may be
  mirrored without an explicit dialect owner.

Keep `unambiguous_callable_result_tuple_item` as-is: it answers callability for the
return path and must not be widened into a reactivity table.

Module identity also belongs here, replacing the `== "solid-js"` literal:

```rust
/// Whether `origin_module` is a module this dialect exports `name` from in value
/// position. Empty or unknown module: `false`.
pub fn exports_value_from(origin_module: &str, name: &str) -> bool; // wraps export_modules(.., Value)
```

and, for the solid-js-certifying-itself case, a **self-artifact** premise owned by
the certifier rather than the dialect: the resolved `Declaration` of the
`CallResult` target lies inside the artifact case under certification *and* that
symbol is the artifact's own export of `name`. That premise is already available
from `plan.candidates.proposal().artifact_case(..)` plus the census
`declaration`; it must be an explicit named rule, not a fallback.

### 4.4 Consumer rules

**Mechanism A — `invoke` operation input (`recursive-value-shape` only).**

In `require_operation_recursive_subject` (`type_facts.rs:3925`), add a third
`RecursiveValueEvidence` arm before `parameter_source` is reached:

> `ReactiveInputAsserted`: the demand's root is `OperationInput{operation, index}`,
> the operation's `kind == Invoke`, `path.0.is_empty()`, `callable ==
> DemandedCallability::Unknown`, and the input is
> `ValueShape::Reactive { role, .. }`.

Discharge condition — **all matching calls, not one**:

1. Resolve the operation's callback binding the way the `Invoke` arm of
   `require_operation_evidence` does: `export.callbacks().items()` → the callback
   whose `operation == operation.id` → `callback.from`, which is always
   `ValueSource::Parameter{index: p, path: []}`.
2. Collect **every** `ImplementationCall` with `is_call_expression(call)` whose
   `callee_parameter` matches `p` exactly (`parameter_value_source_exact`) — this
   is the same predicate `recursive_parameter_call_site` (`:3852`) already uses.
3. Refuse unless the set is non-empty **and** every member has
   `argument_sources[index]` containing a source that satisfies:
   `kind == CallResult`, `!target.is_empty()`, `path.is_empty()`,
   and the `(target_name, target_path)` pair resolves through
   `unambiguous_reactive_result_slot` to exactly the demanded
   `role`, with the module premise of §4.3 satisfied.
   A call whose `argument_sources[index]` is empty, or whose sources disagree,
   refuses the whole demand.
4. Reachability floor: **`MayExecute`, not `Reachable`.** This is a deliberate
   departure from the existing comment on
   `operation_input_value_shape_evidence` (`:3894`), and it needs its own
   justification in the code: the claim is not "this invoke happens" (that is the
   `operation-reachability` demand's job, already discharged through
   `callback.from`) but "whenever it happens, input `index` is this shape", and a
   call in a `do…while` body answers that conditional exactly. marker forces the
   issue: `mapMatch(text)` is `reach=Unknown, captured=true, enclosing=Some((2546,
   2710))` (the `createRoot(dispose => …)` callback) — a `Reachable`, uncaptured
   floor would refuse all three marker rows. `Reachability::Unreachable` must still
   clear nothing.
5. `captured` must **not** be a veto here, for the same reason — but the enclosing
   callable's identity must be recorded in the witness site so the proof is
   auditable.

**Mechanism B — `read` operation input (three families).**

The `read` operation carries **no span**: `ContractReactiveRead` drops the read's
`origin` location at `contracts.rs:1191-1210`, and `ValueShape::Reactive` carries
nothing. So the operation cannot be matched to *a* census call. The only sound
matching rule is a **universal** one:

> For a `read` operation whose `inputs[0]` is `Reactive{role}`, collect every
> `ImplementationCall` with `is_call_expression(call)`, `floor.admits(call.reach)`,
> and a non-empty `callee_sources`. Discharge only when that set is non-empty and
> **every** member's `callee_sources` proves the demanded `role` through
> §4.3. A single call whose callee is a traced accessor of a *different* role, or
> a call with an empty `callee_sources`, refuses.

That is too strong to be useful as written (any ordinary `push()` call has an empty
`callee_sources`), so the rule has to be narrowed by *what the operation claims*:
the read row says "this export performs one reactive read of an accessor", and the
witness must be "there is at least one reachable call whose callee is a traced
accessor of role `role`, and no reachable call whose callee is a traced accessor of
a *different* role". Calls whose callee traces to nothing are irrelevant to the
role claim (they are not accessor reads); calls whose callee traces to a setter
are, and must refuse. This is weaker than mechanism A on purpose, and the weakness
should be recorded in `docs/precision-backlog.md`: the demand as inventoried cannot
distinguish *which* read, so the strongest provable claim is existential-plus-no-counterexample.

Two consequences to accept explicitly:

* **Multiple reads of the same shape collapse.** `createPolled` has two
  `depSignal()` reads (bytes 4105‥4116 uncaptured, 4148‥4159 captured inside
  `() => depSignal()`), deduplicated by `(kind, label)` into one `read-0`. The
  witness site list should carry **both**, so the audit records that the row covers
  a set.
* **Floor.** `createPolled`'s uncaptured read (4105‥4116) is `Reachable`, so the
  existing `!call.captured` + floor discipline of `require_parameter_read_call`
  (`:3647-3670`) can be kept verbatim for the read families. Do **not** relax
  `captured` here: for `operation-reachability` the claim really is "the export
  performs this read", and a read inside a stored closure does not establish it.

**Interprocedural composition (required for `createIntervalCounter`).**
`createIntervalCounter`'s census is the single call `createPolled(…)`. Nothing in
its own transcript witnesses the read. Options, in order of preference:

1. Do not solve it now. `createIntervalCounter`'s three demands stay refused, timer
   stays refused, and the acceptance set for this work is marker ×3 only. This is
   the honest minimum and should be the first commit.
2. Add a *composition* premise: the IR records, per composed operation, the
   `(export, operation)` it was composed from, and the certifier discharges
   `createIntervalCounter:read-0` when (a) the call to the composing target is
   proven reachable and resolves by declaration to the same artifact case's
   `createPolled`, and (b) `createPolled:read-0`'s own demand is discharged with the
   same shape. This needs a new *semantic* field in the contract model (provenance
   of a composed operation), not a producer fact, and it is a much larger change —
   it is the right second step, not part of this one.

Option 2 must not be approximated by "some other export in this artifact case
proves it": that would let any accessor read anywhere in the package discharge any
export's read row.

### 4.5 Must-not-clear traps

Each of these must have a fixture that stays refused.

1. **Locally created accessor from a NON-dialect helper.**
   `const x = myOwnAccessorFactory(); cb(x)`. `calleeSources`/`argumentSources`
   would report `CallResult{target_name: "myOwnAccessorFactory", target_module: ""}`;
   `unambiguous_reactive_result_slot` answers `None` → refuse. The negative fixture
   must also cover a helper *named* `createSignal` but declared locally in a
   non-dialect module.
2. **Non-identifier creator callee.** `dist/solid.js:280`
   `const [value, setValue] = (options.storage || createSignal)(options.initialValue)`.
   `implementationCallTargetLocked` returns `target == ""` for the parenthesized
   `||`, so `returnValueSourcesLocked` emits nothing → refuse. Fixture: the exact
   shape.
3. **Accessor through a variable reassigned in a branch.**
   `let a = s0; if (c) a = s1; cb(a)`. The identifier arm requires the symbol's
   declaration to be a `BindingElement`; a `let` with a later assignment still has
   one declaration, so the trace would succeed and be **wrong**. This is a real
   hole in the existing tracer and the producer must be tightened: refuse when the
   traced binding is not `const`/single-assignment, or when the symbol has any
   write outside its declaration. Fixture: reassignment across a branch, must stay
   refused.
4. **A callback invoked twice with different values.**
   `cb(accessor); cb(plainValue);`. The "all matching calls" rule in §4.4 catches
   it: the second call's `argument_sources[0]` is empty (or traces to a
   non-accessor), so the demand refuses. Fixture required — this is the rule that
   makes "one witnessing call" unsound.
5. **`Plain` / `Object` / `Callable` / `Tuple` / `Choice` / `Store` inputs stay
   unsupported.** None occurs in these six rows. Do not widen the arm to them:
   `Plain` carries `DemandedCallability::NonCallable`, which is a negative claim the
   census cannot make about an argument value at all, and `Object`/`Tuple` need a
   property/slot path the tracer does not produce for object literals. `Store`
   needs its own dialect table row and its own review.
6. **`role` must be checked, not assumed.** `createSignal()[1]` is a `Setter`.
   A rule that proved "traced to createSignal tuple slot *n*" without comparing
   `role` would certify a setter as an accessor.
7. **`Reachability::Unreachable`** clears nothing in either mechanism.

### 4.6 Acceptance digests and control rows

Rows the §2 ceiling says can certify with mechanism A alone:

* `@solid-primitives/marker@0.2.2|solid1|only` —
  case `artifact-case:097ee46807f9c3744a0d4d52750995864bd13fd595342097da576de1040dd2d0`,
  semantic digest `sha256:488c151b8fe6bcfc3549dde8c08d14824cbf66ef13ad4e0b997c6c303f92a5fd`,
  policy digest `sha256:f0dfd235055d1aba95f1de513eeee8109178a186fb2be3901d1f3092a42bb278`,
  demand-graph root `sha256:7ffce3739ec364acd699c81b6926e144a27058563cf0c0f6636b9d6cf38d301e`,
  runtime `sha256:c0655c82e93df388db6bb788c644f977b471395b3c29068585c049dbb14c048e`,
  the single demand being `recursive-value-shape sha256:3b32b258…` / `5e518cf8…` /
  `ad2bc344…` / `d6b2c26c…` (four are planned; the failing one is
  `createMarker:callback-0:0`).
* `@solid-primitives/marker@2.0.0-next.2|solid2|floor` and `|head` —
  case `artifact-case:58b6679b…`, runtime
  `sha256:d201b2eee90a3ffded5b8c7543f258831aeecd1376f75a4ac6ca1e4b6e0a553d`,
  closure `sha256:c54bd02b44dc5361e7c099e457ee1e341629f3b628a217772eada543a49a8091`.

Rows that must **stay refused** as controls, with their refusal recorded:

* `@solid-primitives/timer@1.4.5-next.1|solid2|floor` and `|head` — until
  composition (§4.4 option 2) lands, `createIntervalCounter:read-0` stays open.
  Pin the *exact* refusal text so a later change cannot silently certify it.
* `solid-js@1.9.14|solid1|only` — stays refused on
  `argument-binding sha256:6a05e5b6fd79c4a55e16e73b2923a251e2d4f1243da5118bef2a8e54f979993d`
  (`artifact-case:331dfa49…:createReaction`). Its 123 read demands additionally need
  the self-artifact premise of §4.3.
* Each trap in §4.5 as a focused fixture under
  `fixtures/reactive-ir/` with a `dialect-solid-1x` / `dialect-solid-2` pair where
  the answer differs.

### 4.7 The refusal message

Current message names neither the subject nor a digest, which is why the audit
sidecar reports `demandId: null, family: null`:

```
Type Facts demand operation-input is unsupported:
implementation census only binds exact parameter-rooted operation inputs
```

The `demand:` field is the literal `"operation-input"` because `parameter_source`
does not receive the `ScheduledProofDemand`. Proposed: thread `&ScheduledProofDemand`
(or at least `proof.id` and the export/operation identity) into it, and emit

```
Type Facts demand <proof.id> is unsupported: operation input
<artifact-case>:<export>:<operation>[<index>] is <shape-constructor>, and the
implementation census binds only parameter-rooted operation inputs
(family=<recursive-value-shape|operation-reachability|operation-cardinality>)
```

for example

```
Type Facts demand sha256:3b32b258… is unsupported: operation input
artifact-case:097ee468…:createMarker:operation:callback-0[0] is reactive/accessor,
and the implementation census binds only parameter-rooted operation inputs
(family=recursive-value-shape)
```

`<shape-constructor>` must be a stable spelling (like
`parameter_use_kind_site` at `type_facts.rs:3744-3756`), not `{:?}`. With
`proof.id` present, `demandId` and `family` stop being `null` in the audit
sidecar and the six rows become distinguishable without a source patch.

---

## 5. Alternatives considered

### (a) The producer-fact route (§4)

**Cost.** Protocol 11 → 12 + schema digest + producer rebuild (Go and Rust move
together, `scripts/build-typefacts.sh`, and every gate re-runs because the
`.buildinfo` source-manifest identity changes); one new dialect table with two
per-version rows sets; a new evidence arm and a new universal-quantification rule
in the certifier; the producer tightening for trap §4.5(3); and at least five new
fixtures. Roughly the largest of the three.

**What it buys.** marker ×3 certify soundly. `createPolled` becomes provable.
It also unblocks a general capability — value provenance at any call site, not just
at returns — which the return side already needs for object-literal paths.

**What it loses.** Nothing in precision. The risk is entirely in the traps: the
`let`-reassignment hole (§4.5(3)) exists *today* in `returnValueSourcesLocked` and
extending that tracer to arguments extends the hole to a new surface, so the
tightening is not optional.

### (b) Generator withdraws unprovable non-parameter input shapes to `unknown`

Change `inferred_contract.rs:150-173` and `:300-312` so a read/argument the
generator cannot bind to a parameter emits `ValueShape::Unknown` instead of
`Reactive{Accessor}`. `inventory_value_shape` then demands nothing
(`certification.rs:653-655`) and all six rows lose the blocker with **zero**
producer work.

**Cost: this is a false-negative machine, and the loss is in the user's project,
not in the contract.** Two live consumers read exactly these shapes:

1. **Callback argument shapes** —
   `rust/crates/solid-reactive-ir/src/source_discovery.rs:1774-1808`. For every
   call to a contracted helper, it walks
   `callback.arguments[i]`, and for `descriptor.kind == "accessor"` registers the
   *user's* callback parameter symbol in the `accessors` map. Its own comment says
   why: *"This is not derivable from the callback's TypeScript declaration:
   mapArray/indexArray create the index/item signals internally and hand them to
   the mapper."* Withdraw the shape and the user's `(item) => item()` parameter
   stops being a reactive source; every read through it traces to nothing and every
   reactivity rule on it goes silent. `createMarker((text) => <mark>{text()}</mark>)`
   is exactly this shape.
   `contract_callbacks_unbound` /`contract_callback_arguments_unbound`
   (`interproc.rs:603-640`) is the fail-closed counterpart, and withdrawing the
   descriptor bypasses it rather than triggering it.
2. **Read shapes** — `source_discovery.rs:1427-1446` builds `contract_reads` from
   `reactive_reads` rows with `kind == "accessor" | "store-path"`, and
   `local_access.rs:634-661` turns each into a `ReactiveRead` **at the call site of
   the helper in the user's project**, feeding `strict_read_obligations`. Withdraw
   the shape and `createPolled(...)`, `createIntervalCounter(...)`,
   `createMemo(...)` called outside a reactive scope stop producing an untracked
   read at all.

So (b) buys certification by deleting the very facts certification exists to
publish. It is the wrong direction, but it is worth writing down that it is also
*cheap and green*: no gate would catch the regression except a project-level
fixture that reads through a contracted callback parameter.

A defensible narrow variant: withdraw to `Unknown` **only** where the generator
proves the value is not a dialect accessor — but the generator has no such proof;
the current `Reactive{Accessor}` is already the answer it has, just unaddressable.

### (c) Cheaper sound routes

**(c1) Consumer-only: discharge from `argumentCallables`/`calleeParameter` already
on the wire.** Nothing on the wire says "accessor", so this cannot prove the
`role`. It could only prove a *weaker* subject. Rejected — it would require
changing what the demand asserts, i.e. (c2).

**(c2) Weaken the inventoried subject instead of proving it.** In
`recursive_value_callability` (`certification.rs:745`) `Reactive` already maps to
`DemandedCallability::Unknown`; the *subject itself* could be dropped for
`OperationInput` roots whose shape the census cannot address, exactly as
`ValueShape::Unknown` is dropped — while **keeping** the shape in the emitted
document. That is one small, local change in the IR: "a `Reactive` operation input
inventories no `RecursiveValue` fact". It costs nothing at the producer, keeps
every consumer in §5(b) working, and is sound in the narrow sense that it stops
*claiming* what it cannot prove.

But it does not remove the read blocker: `require_operation_evidence`'s
`OperationKind::Read` arm needs `inputs[0]` to be a parameter to prove
*reachability*, and dropping the recursive subject does not touch that. So (c2) is
mechanism A only: marker ×3 certify, timer and solid-js do not. And it publishes a
contract whose `Reactive{Accessor}` input is **certified-but-unproven**, which is
precisely the "positive fact carried without a witness" the policy-2 design exists
to prevent. It is defensible only as an explicit, documented *withdrawal* of the
input shape from the certified surface — i.e. the shape becomes advisory and the
audit says so — which is a policy decision, not an implementation one.

**(c3) For the read families only: prove reachability without the input.** The
`read` operation's `operation-reachability` demand asks "does the export perform
this read". For a `Reactive{Accessor}` input, an existential witness is available
today with **no** new producer fact and no dialect table: at least one reachable,
uncaptured `ImplementationCall` whose callee's resolved `declaration` is a
`BindingElement`/`VariableDeclaration` *inside the artifact case's own runtime
module* and whose `target_module` is empty (a local value), i.e. "the export calls
something it declared locally". That is far weaker than "reads an accessor" and
would certify a row that says `accessor` on the strength of "calls a local
function" — an over-claim. Rejected, but recorded: it is the shape of the mistake a
cheap fix would take.

**Recommendation.** (a) for mechanism A, scoped to the three marker rows, with
every trap in §4.5 fixtured; timer and solid-js explicitly pinned as still-refused
controls; and the improved refusal message (§4.7) landed first, on its own, because
it costs nothing and makes every subsequent measurement of this class attributable.
