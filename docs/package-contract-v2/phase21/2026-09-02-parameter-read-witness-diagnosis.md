# The `parameter-rooted read has no exact implementation call or use` refusal class

Read-only diagnosis, worktree `codex/phase19a-authenticated-proof-policy` at
`c163c10b`. Every measurement was made with a debug checker built by
`make build-checker-debug` (a bare `cargo build -p solid-facts-backend --bin
solid-checker-rust` produces a binary that refuses with *"verifier build has no
configured Type Facts executable digest"*). `bin/solid-typefacts` was reused as
found — the stamp check reported it already at source
`6ae37483b7740fb7a0b21c8c160159b11102aa56c088c900074bbe98f40a5b3d`.

All temporary diagnostic patches described in §0.1 and §3 have been reverted and
the debug checker rebuilt from the reverted source; `git diff --stat` is empty
and `git status --short` shows only the pre-existing untracked
`packages/cli/solid-reactivity.json.refusals.json`. No
`benchmarks/ecosystem/report-probes-*.md` was created.

Scratch: `<scratch>/reach/` — `base.json`/`base.md` (unpatched reproduction),
`diag.jsonl`, `diag2.jsonl`, `diag3.jsonl` (producer census dumps),
`skip.json`/`skip.md` (the §3 ceiling run).

---

## 0. Two corrections to the premise

### 0.1 Method

Reproduction (unpatched HEAD binary) of all five distinct probes: **5/5
reproduce exactly** — same demand digest, same family, same artifact case, same
export, same refusal tail as the committed `benchmarks/ecosystem/report.json`.
(`@solid-primitives/keyed@3.0.0-next.2|solid2|head` is digest-identical to its
`floor` twin — demand `sha256:01b55dce…`, artifact case `099381488e…` — so four
probes cover the six rows.)

Three temporary diagnostic patches were then applied to
`rust/crates/solid-facts-backend/src/contract_certification/type_facts.rs`, all
gated on an env var and all reverted:

1. a JSON dump of `{operation, source, floor, ok, sites, transcript}` in the
   `OperationKind::Read` arm of `require_operation_evidence` (`:3667`);
2. a JSON dump of the implementation transcript at the
   `"operation value path is absent from the signature census"` refusal in
   `require_operation_recursive_signature` (`:4544-4550`) — this is the keyed
   rows' *current* first refusal, and it is the only way to see `SetValues`'
   census, which the read arm never reaches;
3. the §3 ceiling patch: `require_parameter_read_evidence` pushes
   `diagnostic-skip:parameter-rooted-read` and returns `Ok` instead of its `Err`.

Every byte offset quoted below was verified by extracting the range from the
**installed** file in the kept `--keep-temp` install, not from the producer's
private project copy and not from the package's own source tree.

### 0.2 The message is one helper, but the rows are three mechanisms

The refusal comes from one place —
`type_facts.rs:3831-3835`, in `require_parameter_read_evidence` (fn at `:3780`),
reached only from the `OperationKind::Read` arm of `require_operation_evidence`
(`:3667-3675`) for both `operation-reachability` and `operation-cardinality`. It
accepts either witness:

| witness | predicate | file:line |
| --- | --- | --- |
| a call whose **callee** is the parameter value | `is_call_expression && floor.admits(call.reach) && !call.captured && callee_parameter matches` | `require_parameter_read_call`, `:3723-3752` |
| a **use census** row | `floor.admits(use.reach) && !use.captured && kind ∈ {PropertyAccess, DirectCall, AliasCall} && parameter_binding_matches` | `:3786-3830` |

The three current rows fail three *different* predicates:

* **kobalte / local-store** — a witness of the demanded value exists, is
  `reach: reachable`, and is rejected **only** by `captured`;
* **tanstack `parseWithStandardSchema`** — no witness exists in the census at
  all, because the producer classifies a literal-keyed **element access** as
  `unknownEscape` and `parameterValueSourceLocked` returns `nil` for it;
* **keyed `SetValues`** (latent) — rejected only by `captured`, like kobalte, but
  it never reaches this helper today: `recursive-value-shape` refuses first.

### 0.3 Every one of these reads is stamped `SameStack` by a hardcoded default

This is the fact that decides the whole design question, and it is not in the
brief. `inferred_contract.rs:344-367`:

```rust
fn operation(id, kind, inputs, output) -> Operation {
    Operation {
        …
        trigger: Some(Trigger::Event(Event::Call)),
        at: Some(Event::Call),
        schedule: Some(Schedule::SameStack),
        tracking: Tracking::Untracked,
        cardinality: Cardinality { scope: Some(Call), min: Some(0), max: Some(Many) },
        …
    }
}
```

It is one shared constructor for every inferred `Read`, `Return` and `Create`
operation. Nothing derives `at`/`schedule` from where the site actually is. The
live dump confirms it for all six read operations:

```
kind: Read, trigger: Some(Event(Call)), at: Some(Call), schedule: Some(SameStack),
tracking: Untracked, cardinality { scope: Some(Call), min: Some(0), max: Some(Many) }
```

Two consequences:

* `operation_reachability_floor` (`:3129-3135`) returns `MayExecute` for **every**
  inferred read, because `min == Some(0)` is unconditional. The strict
  `Reachable` floor is unreachable for this family. All six rows measured
  `floor: MayExecute`.
* The `!captured` gate is the **only** enforcement anywhere of the `at: call /
  schedule: same-stack` claim these operations publish. The consumer side proves
  it: `local_access.rs:665-700` turns a published `parameter-member` read into a
  `ReactiveRead` whose `location` is *the caller's call site*, with the caller's
  own execution role — and `ContractReactiveRead` (`solid-reactive-ir/src/lib.rs:1054-1067`)
  has **no** execution or schedule column at all, so the row means
  unconditionally "this export reads that path when you call it".

So the captured veto is not decoration and not an oversight. It is load-bearing
for two of the three rows, and relaxing it alone would publish a false claim.

---

## 1. Per-row mechanism

### 1.1 `@solid-primitives/local-store@1.1.4|solid1|only` — export `default`

Demand `sha256:446606d9…`, `operation-reachability`, artifact case
`7bbf136035…`, operation
`artifact-case:7bbf1360…:default:operation:read-0`, `kind: Read`, `at: Call`,
`schedule: SameStack`, `tracking: Untracked`, `count: Call 0..Many`, floor
`MayExecute`.

**Demanded source: `Parameter { index: 1, path: [] }`** — parameter 1 is
`storage` of `createLocalStore(prefix = null, storage = localStorage)`.

*Why the path is empty*, which the brief's "wrong segment" hypothesis would not
predict: `contracts.rs:1218-1232` keys one row per parameter and names a path
only when **every** contributing access agrees. Parameter 1 contributes five
distinct paths (`getAll`, `getItem`, `setItem`, `removeItem`, `clear`), so
`paths.len() != 1` and `path` is `None`; `inferred_contract.rs:153-167` then
builds `path: Vec::new()`. The demand is therefore the *weakest* claim in the
model, and `parameter_binding_matches` (`:3047-3072`) accepts any observed path
against it. The IR demand is **not** wrong.

Runtime bytes (`node_modules/@solid-primitives/local-store/dist/index.js`, 1775
bytes total; implementation location 1757‥1773 = `createLocalStore`):

```
670‥1739   return [ new Proxy({}, { get(_, key) { … } }), (key, value) => {…}, key => {…}, () => {…} ];
715‥1298     get(_, key) { … }                                  ← Proxy handler method
813‥835        () => storage.getAll()
819‥835          storage.getAll()
1245‥1283      storage.getItem(`${propPrefix}${key}`)
1319‥1490    (key, value) => { storage.setItem(…); … }
1349‥1405      storage.setItem(`${propPrefix}${key}`, value.toString())
1500‥1647    key => { storage.removeItem(…); … }
1521‥1562      storage.removeItem(`${propPrefix}${key}`)
1657‥1732    () => { storage.clear(); signals.clear(); }
1677‥1692      storage.clear()
```

Producer census (`parameter_uses`, 9 rows): parameter 0 has two
`unknownEscape` rows; **all seven rows for parameter 1 are
`kind: capture, reach: reachable, captured: true`** (796‥803, 819‥826, 844‥851,
1245‥1252, 1349‥1356, 1521‥1528, 1677‥1684). `parameterUseCensusLocked`
(`invocation_transcripts.go:1462-1464`) overwrites the classification
unconditionally: `if captured { kind = ParameterUseCapture }`. So the use loop
rejects on both `use_site.captured` **and** the kind gate.

Producer `calls` (18 rows) — five carry a matching `calleeParameter`, every one
`captured: true`:

| call | `calleeParameter` | `enclosingCallable` |
| --- | --- | --- |
| 819‥835 `storage.getAll()` | `{1, [getAll]}` | 813‥835 |
| 1245‥1283 `storage.getItem(…)` | `{1, [getItem]}` | 715‥1298 (`get` method) |
| 1349‥1405 `storage.setItem(…)` | `{1, [setItem]}` | 1319‥1490 |
| 1521‥1562 `storage.removeItem(…)` | `{1, [removeItem]}` | 1500‥1647 |
| 1677‥1692 `storage.clear()` | `{1, [clear]}` | 1657‥1732 |

`require_parameter_read_call` rejects all five on `!call.captured` (`:3734`).

**The chain the existing machinery would build.** `control_flow.returns[0]`
(location 670‥1739, `reach: reachable`, `carryReach: reachable`) carries exactly
`[1319,1490]`, `[1500,1647]`, `[1657,1732]`. So
`callable_is_executed_within` (`:3450`) would answer `true` for the enclosing
callables of `setItem`, `removeItem` and `clear` via its
`carried_by_implementation_return` edge, and `implementation_call_is_executed`
(`:3398`) — which the **Invoke** arm already uses at `:3173` and `:3207` —
would accept those three. The `getAll` and `getItem` sites stay refused: their
chain terminates at the Proxy handler method 715‥1298, which no edge reaches
(`new Proxy({}, {…})` is `kind: construct`, its `argumentCallables` is empty
because an object literal is not a carried callable, and `Proxy` is neither
`solid-js` nor a reviewed default-library invoker).

**Class: (a) with (c) as its symptom.** The demanded path is right; the census
row exists; the consumer predicate is blunter than the machinery available two
arms above. But the claim being certified says the read happens *on the caller's
stack when it calls `createLocalStore`*, and it does not: the reads happen when
the consumer later touches the returned proxy or calls the returned
setter/remover/clearer. Certifying it from a returned-closure edge would publish
a false `parameter-member` read at the caller's call site. The defect is that
`inferred_contract.rs` stamped `at: call / schedule: same-stack` on a read whose
only sites are inside values the export hands back.

### 1.2 `@kobalte/core@2.0.0-alpha.0|solid2|only` — export `createDomCollection`

Demand `sha256:3a39de55…`, `operation-reachability`, artifact case
`f47c3eadec…`, entrypoint `./primitives/create-dom-collection`, operation
`…:createDomCollection:operation:read-0`, same attributes, floor `MayExecute`.

**Demanded source: `Parameter { index: 0, path: ["onItemsChange"] }`** — one
contributing access, so the path is named.

Runtime bytes (`node_modules/@kobalte/core/dist/create-dom-collection/OK7882aq.js`;
implementation location 3148‥3167 = `createDomCollection`):

```
3276‥3383  createEffect(() => items(), (currentItems) => {
3289‥3302     () => items()                                   ← argument slot 0
3304‥3365     (currentItems) => { props.onItemsChange?.(currentItems); }   ← argument slot 1
3326‥3361        props.onItemsChange?.(currentItems)
3326‥3331          props
                 }, { defer: true })
```

Producer census: **one** `parameter_uses` row for parameter 0 —
3326‥3331, `kind: capture`, `reach: reachable`, `captured: true`. One matching
call — 3326‥3361, `calleeParameter: {0, [onItemsChange]}`, `kind: call`,
`reach: reachable`, `captured: true`, `enclosingCallable: 3304‥3365`. The
optional-call punctuation costs nothing: `parameterValueSourceLocked`
(`export_value_transcripts.go:669-679`) handled `props.onItemsChange` correctly.

The chain is fully explicit in the census. The `createEffect` call at 3276‥3383
carries `target_module: "solid-js"`, `target_name: "createEffect"`, three
argument slots, and
`argumentCallables: [{argument: 0, locations: [3289‥3302]}, {argument: 1,
locations: [3304‥3365]}]` — slot 1's carried location is byte-identical to the
read call's `enclosingCallable`. So `callable_is_executed_within`'s argument
route reaches `argument_slot_is_proven_invoking(createEffect_call, 1)`
(`:3563-3583`), and every one of its three premises answers no:

* tier A: `solid_dialect::unambiguous_callback_argument("createEffect", 1, 3)` is
  **false**. `solid_1x.rs:467-476` lists `CreateEffect → [(0, Tracked)]`, so V1
  answers `None` for slot 1 (1.x's `createEffect(fn, value, options)` slot 1 is
  the *initial value*); `solid_2.rs:572-574` lists
  `CreateEffect → [(0, Tracked), (1, Deferred)]`, so V2 answers
  `Some(Deferred)`. `lib.rs:262-265` requires
  `let Some(Some(first)) = answers.first()` and V1 is first, so the whole
  helper returns `false`. This is pinned deliberately — `type_facts.rs:8990-8992`
  says so in as many words, and the doc at `:3528-3532` states
  "`createEffect(fn, initialValue)` at slot 1 stays refused";
* `default_library_invoker` is empty;
* no `callee_*` list and no `callee_pending_invocations` on this call.

**Class: (a) + (c) + a dialect-seam gap.** The site is a genuine, provably
*possible* read — but it is in `createEffect`'s **deferred** apply slot with
`{ defer: true }`, so it is neither same-stack nor guaranteed to happen at all.
The published claim (`at: call, schedule: same-stack`) is wrong in kind, and
even a corrected `queued` claim would still need a dialect answer for
`createEffect` slot 1 that the cross-dialect unanimity rule refuses to give.

### 1.3 `@tanstack/ai-solid@0.19.1|solid1|only` → `@tanstack/ai@0.49.1` (`./client`) — export `parseWithStandardSchema`

Demand `sha256:1097d02a…`, `operation-reachability`, artifact case
`5a06c846c6…`, operation `…:parseWithStandardSchema:operation:read-0`, same
attributes, floor `MayExecute`. (This row also carries a separate, earlier
dependency-graph budget refusal recorded in a prior round; not re-investigated
here.)

**Demanded source: `Parameter { index: 0, path: [] }`.**

Runtime bytes
(`node_modules/@tanstack/ai/dist/esm/activities/chat/tools/schema-converter.js`;
implementation location 12722‥12745; declaration
`export declare function parseWithStandardSchema<T>(schema: unknown, data: unknown): T`):

```
12233‥12257  isStandardSchema(schema)
12250‥12256    schema
12288‥12322  schema["~standard"].validate(data)
12288‥12294    schema
```

Producer census (4 `parameter_uses`, 4 `calls`), all `reach: reachable`, **none
captured**:

| row | bytes | kind |
| --- | --- | --- |
| use p0 | 12250‥12256 | `argumentKnown` |
| use p1 | 12266‥12270 | `return` |
| **use p0** | **12288‥12294** | **`unknownEscape`** |
| use p1 | 12317‥12321 | `argumentKnown` |
| call | 12288‥12322 | `calleeParameter: null` |

Two producer gaps, both the same root cause — an `ElementAccessExpression` with
a **string-literal** argument is an ordinary property read and neither code path
recognizes it:

* `parameterUseKindLocked` (`invocation_transcripts.go:1532-1553`) matches only
  `ast.IsPropertyAccessExpression(parent) && parent.Expression() == node`
  (`:1540`). `schema["~standard"]` falls through every arm to
  `ParameterUseUnknownEscape`, which the kind gate refuses;
* `parameterValueSourceLocked` (`export_value_transcripts.go:648-681`) descends
  only `IsIdentifier` (`:658`) and `IsPropertyAccessExpression` (`:669`), so
  `schema["~standard"].validate` recurses into the element access and returns
  `nil` at `:680`. Hence `calleeParameter: null` and no call witness.

The IR demand's empty path is the fail-closed answer of
`member_callee_receiver` (`indexes.rs:1190-1240`): the computed member clears the
segments collected so far (`:1219-1220`), so `["~standard","validate"]` truncates
to `[]`. That is deliberate and sound, and it means a producer-only fix suffices
here — a `propertyAccess` use with `binding_path: []` matches `Parameter{0, []}`
immediately.

**Class: (b), cleanly.** This read *is* same-stack, uncaptured and reachable; the
claim is honest and the census simply does not record it. This is the only one
of the six where the consumer predicate and the IR demand are both right.

### 1.4–1.6 the three `@solid-primitives/keyed` rows — export `SetValues`

`1.5.3|solid1|only` (demand `sha256:474ff89a…`, case `a4ca1cd39c…`) and
`3.0.0-next.2|solid2|floor`/`|head` (demand `sha256:01b55dce…`, case
`099381488e…`) currently refuse **earlier**, at `recursive-value-shape`:

```
operation value path is absent from the signature census
  (alternative=0, path=[PathSegment { kind: Property, property: "of", index: None },
                        PathSegment { kind: Property, property: "values", index: None }])
```

`props.of` is `Set<T> | undefined | null | false` — the nested nullable union the
census does not answer (the mechanism investigated and withdrawn, recorded at
`docs/precision-backlog.md:10996-11105`; ADR
`docs/typefacts/adr/0023-v1-apparent-callable-path-members.md`). That refusal
also confirms the demanded read input directly: it is
`ValueRoot::OperationInput { operation: …:SetValues:operation:read-0, index: 0 }`
with value path `["of","values"]`, i.e. the read operation's input is
**`Parameter { index: 0, path: ["of", "values"] }`** — the whole-path form, so
the 2026-09-01 last-segment fix did land (`docs/precision-backlog.md:212-216`
still describes the pre-fix `Parameter{0, ["values"]}`).

`SetValues`' census, dumped at that refusal
(`node_modules/@solid-primitives/keyed/dist/index.js`, 1.5.3; implementation
location 7396‥7405):

```
7575‥7764  createMemo(mapArray(…))                                  reach reachable, uncaptured
7586‥7763    mapArray(() => props.of && Array.from(props.of.values()), …)   solid-js, 3 slots
7595‥7642      () => props.of && Array.from(props.of.values())      ← argumentCallables[0]
7624‥7641        props.of.values()   calleeParameter {0,[of,values]}, reachable, CAPTURED, enclosing 7595‥7642
7548‥7553      props            use, propertyAccess, reachable, NOT captured, bindingPath []
7708‥7713      props            use, unknownEscape ("fallback" in props)
```

3.0.0-next.2 is byte-for-byte the same shape (`SetValues` at 6233, `mapArray`
imported from `"solid-js"` in both versions).

So after the census gap is answered, `SetValues` lands on exactly this refusal
class, which is what the brief reports (demands `af0021c3…` / `74a2e818…` are the
`operation-cardinality` twins of the same operation). Both witnesses fail:

* the call at 7624‥7641 matches the demanded path **exactly** and is rejected
  only by `captured`;
* the uncaptured `propertyAccess` use of `props` at 7548‥7553 has
  `binding_path: []`, and `parameter_binding_matches` (`:3068`) requires
  `path.len() >= expected_path.len()` — 0 ≥ 2 is false. **The use census can
  never witness a non-empty demanded path unless the parameter was destructured
  to that depth**, because `parameterCensusRootsLocked`
  (`invocation_transcripts.go:1485-1531`) derives `binding_path` from the
  *declaration's* destructuring pattern only, never from the member chain at the
  use site. For a plain `props` parameter every use row carries `[]`.

Under the swap to `implementation_call_is_executed`, `SetValues` would reach
`argument_slot_is_proven_invoking(mapArray_call, 0)` and still fail:
`unambiguous_callback_argument("mapArray", 0, 3)` is **false** because
`solid_1x.rs:504-506` gives `MapArray → [(0, Tracked), (1, Deferred)]` while
`solid_2.rs:578` gives `MapArray → [(1, Tracked)]` (2.0's `mapArray(list, mapFn)`
maps at 1 — `solid_2.rs:160-161`), and both dialects canonically own the name
(`solid_2.rs:68`).

**Class: (c) + dialect seam.** For `1.5.3|solid1` the *claim* is honest: 1.x
`createMemo` runs its compute during the creating call
(`solid_1x.rs` doc, "`updateComputation(c)` before it returns"), and `mapArray`'s
slot-0 source runs inside that compute, so the read really is same-stack. Only
the consumer's blunt `captured` veto plus the cross-dialect unanimity rule block
it. For the two `solid2` rows, V2 states nothing about `mapArray` slot 0 at all
— absence is not "invoked" — so those two stay honestly refused.

### 1.7 Two rows that *pass*, and why that matters

The dump caught two read demands succeeding in the same runs:

* **`keyed`'s `MapEntries`** (both versions) certifies — `sha256:3ec5e455…` /
  `sha256:a2a2ae18…` — on the witness
  `implementation-read-use:…/keyed/dist/index.js:6492:6497:propertyAccess`. It
  passes only because `MapEntries` performs **two** distinct member calls
  (`props.of.keys()` at 6568‥6583 and `props.of.get(…)` at 6637‥6654 / 6695‥6712),
  so `contracts.rs:1218-1232` cannot name an agreed path, the demand collapses to
  `Parameter{0, []}`, and the unrelated uncaptured `props.children` read at
  6492‥6497 satisfies it. Sound (a shorter path is a weaker claim) but coarse:
  the witness proves a read of `props`, not of `props.of.keys`. `SetValues` fails
  where `MapEntries` passes purely because `SetValues` has one member path and
  therefore keeps it.
* **`@tanstack/ai`'s `generationParamsFromRequest`** certifies —
  `sha256:4b3e9419…` — on `implementation-read:…/client.js:4724:4738`
  (`request.json()`, `calleeParameter {1,[json]}`, uncaptured). Note its
  transcript carries `openReasons: ["controlFlowUnsupported"]` and only two
  `parameter_uses` rows: `parameterUseCensusLocked` withheld the rest inside
  unsafe-jump regions (`invocation_transcripts.go:1436-1438`). Read evidence does not consult
  `control_flow`, so this passes on a partial census — worth a look in its own
  right, but not one of the six rows.

---

## 2. Shared cause

**No single defect explains all six, but one explains four.**

* **`captured` alone rejects a present, exact, reachable witness** in
  `local-store`, `kobalte`, and both keyed `SetValues` variants — 4 of the 6 rows
  (5 of 6 counting the keyed head twin). In every one of those the census already
  contains the enclosing callable and, where the enclosing callable is an
  argument slot, the exact `argumentCallables` edge to it. The consumer simply
  does not walk it: the `Read` arm uses the blunt `!call.captured` (`:3734`)
  while the `Invoke` arm two functions above uses the full execution premise
  `implementation_call_is_executed` (`:3173`, `:3207`).
* **But the reason it must not simply be relaxed is also shared**: all six read
  operations carry the hardcoded `at: call / schedule: same-stack` from
  `inferred_contract.rs:356`, and `ContractReactiveRead` has no column in which
  a different schedule could be published. So for `local-store` (returned
  closures) and `kobalte` (deferred effect) the captured veto is the only thing
  standing between the corpus and a false published claim.
* **`tanstack parseWithStandardSchema` is unrelated to all of that** — a
  producer classification gap for literal-keyed element access.

The honest summary: one *consumer* defect (asymmetric evidence rules for Read vs
Invoke) sitting on top of one *IR* defect (an unproven execution-point claim
stamped by a shared constructor), plus one independent *producer* gap.

---

## 3. True ceiling: **0 of 3 certify**

Temporary patch: `require_parameter_read_evidence` pushes
`diagnostic-skip:parameter-rooted-read` and returns `Ok` instead of its `Err`;
nothing else changed. Rebuilt debug checker, reran the three current rows plus
`keyed@1.5.3` as a control.

With the blocker skipped, all ten read demands across the four probes report
`ok: true` (`local-store` read-0 ×2, `MapEntries` ×2, `generationParamsFromRequest`
×2, `parseWithStandardSchema` ×2, `createDomCollection` ×2), and **every row
advances to a new first refusal in the `recursive-value-shape` family on the same
operation input**:

| row | next first refusal | demand | why |
| --- | --- | --- | --- |
| `@solid-primitives/local-store@1.1.4\|solid1\|only` | `recursive-value-shape (…:default)`: *operation value path is locally open (complete=false, presence=Required, callability=NonCallable, reasons=["unresolvedGeneric"])* | `sha256:f9d0cdce4525fc26fca70e8609f86b9d39b5849a846600d4b71e830ceb22502c` | `createLocalStore<T>(…): [store: T, …]` — the returned tuple's slot 0 is the unresolved generic `T`. The `unresolvedGeneric` (`Instantiable`) class already diagnosed in `docs/package-contract-v2/phase21/2026-09-01-producer-roots-diagnosis.md` |
| `@kobalte/core@2.0.0-alpha.0\|solid2\|only` | `recursive-value-shape (…:createDomCollection)`: *operation value path is locally open (complete=true, presence=Absent, callability=Unknown, reasons=[])* | `sha256:7c107f157cb7ecc5a60e872106c08e699bcdf0b3dcea9df6c88feb409a2ebcd2` | `createDomCollection<T>(props?: CreateDomCollectionProps<T>)` — the parameter is **optional**, so its type is `CreateDomCollectionProps<T> \| undefined` and the census answers `presence: Absent` for `onItemsChange`. Same family as keyed's withdrawn nullable-union mechanism, one level shallower |
| `@tanstack/ai-solid@0.19.1\|solid1\|only` | `recursive-value-shape (…:parseWithStandardSchema)`: *operation value root shape has no verifiable premise: the demand asserts no callability and the producer's root observation is open* | `sha256:acef8fc69139ad0d7b660b547baa9f8ebd29fdf06ea920b6296f000f7e5f9009` | `parseWithStandardSchema<T>(schema: unknown, …)` — the demanded root is typed `unknown`, so the root observation carries `openType`. The `openType` class from the producer-roots diagnosis |
| `@solid-primitives/keyed@1.5.3\|solid1\|only` (control) | unchanged: `recursive-value-shape sha256:474ff89a…`, `of.values` absent | — | the skip does not touch its earlier refusal, as expected |

The patch was reverted immediately; `git diff --stat` is empty and the debug
checker was rebuilt from the reverted source.

**Read this number carefully.** Solving this blocker alone converts **zero**
refused rows into certified rows. Its value is that it removes a *first* blocker
whose next blocker is, in two of three cases, a class already diagnosed and
already scheduled elsewhere — and in the third case (kobalte) a new instance of
the union-member-presence family. It also removes the blocker from any future row
whose second blocker does not exist.

---

## 4. Design per class (no implementation)

### 4.A The IR must derive a read's execution point instead of stamping it

**Owner: `rust/crates/solid-facts-backend/src/inferred_contract.rs` +
`rust/crates/solid-reactive-ir/src/{interproc,contracts}.rs`** (the demand
builder side).

Minimal sound fix. `invoked_parameter_members` currently carries
`(owner_span, parameter, path)` (`interproc.rs:990-995`, `:1071-1082`). Carry a
third fact with each contribution: **whether the contributing access sits
directly in the owner's body, or inside a nested callable** — and, when nested,
whether the innermost enclosing callable is a dialect callback slot and which
slot. `ContractReactiveRead` then gains an execution column (the same
`inline`/`deferred`/`tracked` vocabulary `ContractCallback` already publishes,
plus the `CallbackSchedule` triple), `inferred_contract.rs` reads it instead of
the `SameStack` default, and `contract_document.rs:490-499` publishes
`at: {event, schedule}` that is actually derived.

Fail-closed direction: a contributing access whose position cannot be
established publishes **no read row** for that parameter, rather than an
`inline` one. An omitted read row is the model's "unknown", not a negative
claim, and `local_access.rs` already treats an absent row as "no obligation to
report" rather than "proven not to read".

What this reaches: nothing on its own — it *unblocks* 4.C by making the claim
provable. It is a prerequisite, not a yield.

Must-not-clear traps, with concrete fixtures (extend
`fixtures/package-contracts/parameter-member-read-path`, whose README already
carries the prefix argument):

* `export function returnsReader(store) { return () => store.getItem("k"); }` —
  the read is in a returned closure. Must **not** publish an `inline` read; a
  consumer that saw one would attribute a dependency to its own call site that
  the runtime performs later. This is `local-store`'s exact shape.
* `export function deferred(props) { createEffect(() => props.dep, () => props.onChange?.() ); }`
  — must not publish `inline` for the second access. This is `kobalte`'s shape.
* `export function inlineRead(options) { return options.source.slice(0); }` —
  the existing control: still `inline`, unchanged.
* A row where one contributing access is inline and another is captured. The
  agreed-path rule (`contracts.rs:1225-1232`) collapses disagreeing *paths*; a
  row must equally refuse to publish one execution word for two positions —
  publish two rows, or none.

### 4.B The producer must classify a literal-keyed element access as a property read

**Owner: `apps/solid-typefacts/internal/typefacts/tsgo/`** (producer census).

Minimal sound fix, in two places that must move together:

* `parameterUseKindLocked` (`invocation_transcripts.go:1532`): add an arm for
  `ast.IsElementAccessExpression(parent) && parent.Expression() == node` **and**
  the argument expression is a string- or numeric-literal → `PropertyAccess`.
* `parameterValueSourceLocked` (`export_value_transcripts.go:648`): descend an
  `ElementAccessExpression` whose argument is such a literal, appending
  `PathSegment{Kind: Property, Property: <literal text>}`.

Both gated on a *literal* argument. `obj[key]` with any non-literal argument
must stay `unknownEscape` / `nil` — that is a computed access whose property
nobody can name, and the IR's own `member_callee_receiver` already clears its
segments (`indexes.rs:1219-1220`) for exactly that reason. A template literal
with no substitutions is a judgment call; refuse it in the first cut.

What this reaches: `@tanstack/ai@0.49.1 parseWithStandardSchema` (its read
demand — the row then refuses on the `unknown` root, so **no certification
gain**). Latent value across the corpus for every `obj["literal"]` access,
including the `["~standard"]` Standard-Schema convention that is spreading
through the ecosystem.

Must-not-clear traps (new fixture, e.g.
`fixtures/package-contracts/parameter-member-read-element-access`):

* `export function literalKey(options) { return options["source"].slice(0); }` —
  path `["source","slice"]`, witnessed.
* `export function computedKey(options, key) { return options[key].slice(0); }` —
  the path must truncate to `[]` and the use must stay `unknownEscape`. The
  existing `computedRoot` export in `parameter-member-read-path/index.js` is the
  IR half of this; the producer half is new.
* `export function numericKey(options) { return options[0].slice(0); }` — decide
  and pin whether a numeric literal is a `Property` or a `Tuple` segment. It must
  match whatever `member_callee_receiver` produces, or the two sides will demand
  and witness different paths.
* The write direction. `docs/precision-backlog.md:239-247` records that
  `props.children = 1` already produces `kind: propertyAccess, captured: false`
  and that this was **reviewed and deliberately kept** — evaluating `props` is
  required to store into it, so it is a true read of `props`. Element access must
  inherit exactly that reasoning and no more: `props["children"] = 1` may record
  a read of `props`, and must not be turned into a witness for a demanded path
  that the write does not evaluate.

### 4.C The consumer's Read arm should ask the question the Invoke arm asks — *once the claim can carry the answer*

**Owner: `rust/crates/solid-facts-backend/src/contract_certification/type_facts.rs`.**

Minimal sound fix: make the evidence rule a function of the operation's
*published* schedule rather than a constant.

* `schedule: SameStack` → keep `!call.captured` and `!use.captured` exactly as
  they are. A call in a nested callable is not a call on the export's own stack,
  whatever proves the callable runs.
* `schedule: Queued | External` → use `implementation_call_is_executed`
  (`:3398`), the same premise `require_parameter_flow` uses at `:3173`. The
  captured *use* loop gains the parallel edge: a `capture`-kind row whose
  location lies inside a callable `callable_is_executed_within` admits, with the
  original classification recoverable (see below).

**Answering the brief's `captured` question directly.** Yes — the sound witness
shape is *"the use sits inside a callable that occupies a proved-invoking
argument slot"*, and it is exactly what `callable_is_executed_within`'s argument
route already computes for calls. But it is **not sufficient on its own**, and it
is **not what a same-stack claim needs**. Three things have to be true together:

1. the operation's published schedule must permit a non-same-stack site — which
   today it never does (§0.3), hence 4.A first;
2. the enclosing callable must be reachable by an edge already in the census —
   `carried_by_implementation_return`, `carried_by_callable_return`, or an
   `argumentCallables` slot that `argument_slot_is_proven_invoking` admits;
3. the *floor* must match the strength of the slot's answer — and here the
   existing helper is already exactly right, which is worth stating precisely
   because it is easy to get backwards. `argument_slot_is_proven_invoking`'s own
   contract (`type_facts.rs:3550-3562`) is explicit that every tier is a **"can
   execute"** premise: *"a consumer may read 'this argument is a callback this
   implementation runs' and may not read 'it is run at least once'. Nothing
   downstream may derive a lower bound from it."* That is precisely the strength
   a `min: Some(0)` read asks for, so under today's unconditional
   `MayExecute` floor the helper discharges the claim as-is. The moment a read
   carries `min >= 1` this helper stops being adequate and something stronger
   would be needed — not a relaxation of it. `{ defer: true }` and
   `Execution::Deferred` therefore need no extra treatment for reachability;
   they matter for 4.A, where they decide the *schedule* the row publishes.

**Two facts are missing to carry it.** The producer states `captured: bool` and
`enclosing_callable: Option<Location>` on a *call* (`invocation.rs:411-423`) but
`ParameterUse` (`invocation.rs:683-699`) carries **only** `captured: bool` — no
`enclosing_callable`, and the kind is destroyed
(`invocation_transcripts.go:1462-1464` overwrites it with `Capture`). So the use
census cannot participate in any chain at all. Two additive producer fields:

* `ParameterUse.enclosingCallable: Option<Location>` — the same fact the call
  census already publishes, from the same walk;
* keep the syntactic kind and report capture as a separate boolean instead of
  overwriting it (or add `capturedKind`). Today a captured `props.of` read and a
  captured `props` escape into a registry are the same wire value.

Both are additive and versioned like every other census field. Note the existing
comment at `invocation_transcripts.go:1422-1430` explaining that the *use* census
already exempts an implementation whose own body is a callable where the call
census does not — that asymmetry is measured and recorded; adding
`enclosingCallable` must not disturb it.

What this reaches, in combination:

| row | reached by | outcome |
| --- | --- | --- |
| `local-store` | 4.A (publish `deferred`, not `inline`) + 4.C (return-carry edge, 3 of 5 witnesses) | read blocker cleared; row then refuses on `unresolvedGeneric` |
| `kobalte createDomCollection` | 4.A + 4.C + 4.D | read blocker cleared; row then refuses on the optional-parameter union presence |
| `keyed SetValues` `1.5.3\|solid1` | 4.C + 4.D (claim already honest — 1.x memo compute is same-stack) | read blocker cleared; row still behind the withdrawn nullable-union census gap |
| `keyed SetValues` 3.0 `floor`/`head` | not reached — V2 states nothing about `mapArray` slot 0 | stays refused, honestly |
| `tanstack parseWithStandardSchema` | not reached — 4.B is its blocker | — |

Must-not-clear traps:

* The debounce shape the doc at `:3320-3336` already names:
  `export function f(cb) { createEffect(() => { const inner = () => cb.read(); store(inner); }); }`
  — the read site lies lexically inside a range `createEffect` invokes, and
  `cb.read()` never runs. `enclosing_callable` on the *use* must be the
  immediately enclosing callable and every callable above it proven in turn, or
  this clears.
* `export function f(handler) { document.addEventListener("x", () => handler.value); }`
  — a non-dialect closure with no proven-invoking slot. Must stay refused.
* A loop: `export function f(items) { for (const i of items) i.read(); }` —
  `reach: Unknown`, admitted only by `MayExecute`. Must stay refused for any
  future `min >= 1` read.
* `export function f(props) { const cb = () => props.dep; return cb; }` where
  the caller may never call `cb` — clears only under a `deferred`/`external`
  published schedule, never under `inline`.
* The pre-existing rest-element defect (`docs/precision-backlog.md:227-238`):
  `function f({ a, ...rest })` gives root path `["rest"]`, so a demand for
  `Parameter{0,["rest"]}` is witnessed by any use of `rest`. Widening the use
  census's role makes this a second consumer of the same roots; the fix belongs
  in `parameterCensusRootsLocked` and should land before, not after.

### 4.D The dialect gate is unanimity where the artifact case pins a version

**Owner: `rust/crates/solid-dialect/` (the shared `Dialect` seam) plus the
certification plan.**

`unambiguous_callback_argument` (`lib.rs:252-266`) asks "do all dialects that own
this name agree", and the two rows it blocks are blocked by *real* 1.x/2.0
divergences: `createEffect` slot 1 is the initial value in 1.x and the apply
callback in 2.0; `mapArray` slot 0 is the tracked source in 1.x and unmodelled in
2.0. Unanimity is the right answer for a claim that must hold for *any* installed
Solid. But an ecosystem artifact case is not that claim: `@kobalte/core@2.0.0-alpha.0`
was certified against `solid-js@2.0.0-rc.0` and `@solid-primitives/keyed@1.5.3`
against `solid-js@1.9.14`, both recorded in the probe's `installedVersions`.

Minimal sound fix: thread the artifact case's **resolved `solid-js` identity**
into the certification plan and answer the slot question with that dialect alone,
recording the dialect identity in the witness site and in the emitted contract's
own applicability. A contract certified under the v1 dialect must not be
believed by a v2 consumer, which the artifact case's condition set is already
the place to say.

This is the largest of the four and the one most likely to be out of scope for
this phase. Note the cheaper half-measure and why it is *not* sound: taking
"whichever dialect answers" instead of unanimity would make
`createEffect(fn, initialValue)` at slot 1 a proven callback slot under 1.x,
which is exactly the refusal `type_facts.rs:8990-8992` pins.

What this reaches: `kobalte` (with 4.A/4.C) and `keyed SetValues 1.5.3|solid1`
(with 4.C). Not the keyed 3.0 rows.

Must-not-clear traps:

* the existing pinned refusals at `type_facts.rs:8988-9000` — a same-named
  function from another package, `createEffect` slot 1 **under a v1-resolved
  case**, and an empty `requires` list — must all still refuse;
* a case whose `solid-js` resolution is absent or unparsable must fall back to
  unanimity, never to "whichever dialect answers". This is the same trap
  `rust/crates/solid-facts-backend/src/dialect.rs` already carries for fixture
  dialect selection (a missing stub silently selects v2), and it is recorded in
  AGENTS.md's "Known traps";
* the differential pair `fixtures/reactive-ir/dialect-solid-1x` /
  `dialect-solid-2` pins where the dialects deliberately differ; a slot answer
  that starts varying by case must not blur that pair.

### 4.E One observation that needs no fix, only a decision

`MapEntries` certifies because two disagreeing member paths collapse its demand
to `Parameter{0, []}`, which an unrelated `props.children` read then satisfies
(§1.7). That is sound under the prefix rule, but it means **a coarser demand is
easier to witness than a precise one** — `SetValues` is refused for being more
precise. If that asymmetry is unintended, the answer is not to loosen the
matcher (the README at
`fixtures/package-contracts/parameter-member-read-path/README.md` argues
correctly that "the demanded segment appears somewhere in the observed path"
would be a guess). It is to emit **one row per distinct agreed path** instead of
one row per parameter with a collapsed path — a `contracts.rs:1218-1232` change
with its own fixture, and one that would make `MapEntries` demand
`["of","keys"]` and `["of","get"]` separately, i.e. move it *into* this refusal
class. Worth deciding deliberately rather than discovering later.

---

## 5. Cost and expected yield

| fix | owner | rough size | rows whose *first* blocker it clears | rows it certifies |
| --- | --- | --- | --- | --- |
| **4.B** literal element access | producer (2 functions, ~20 lines) + 1 fixture | small | 1 (`parseWithStandardSchema`) | **0** — the row's root is `unknown` |
| **4.A** derived read execution point | IR demand builder + contract schema column + document emitter | medium-large (schema-visible; touches `ContractReactiveRead`, `contract_document.rs`, every published read row and its snapshots) | 0 alone | 0 alone |
| **4.C** schedule-matched read evidence | consumer predicate + 2 additive producer fields on `ParameterUse` | medium (small in Rust; the producer fields carry a protocol/schema bump and a `bin/solid-typefacts` rebuild) | 2–3 (`local-store`, `keyed SetValues 1.5.3` — `kobalte` needs 4.D too) | **0** |
| **4.D** dialect-scoped slot answer | dialect seam + certification plan applicability | large; architectural | 1–2 (`kobalte`, and `keyed SetValues 1.5.3` jointly with 4.C) | **0** |

**Honest bottom line, with the first-blocker caveat applied: this refusal class
is worth clearing for the corpus's future, not for its present score.**

* **Certified outright by any combination of the four: zero of the six rows.**
  §3 measured it directly.
* **Advance to a new first refusal: all three current rows**, each into
  `recursive-value-shape`, and two of those three (`local-store`'s
  `unresolvedGeneric`, `parseWithStandardSchema`'s `openType`) into classes the
  2026-09-01 producer-roots diagnosis already owns. `kobalte`'s next blocker
  (`presence: Absent` for a member of an **optional** parameter's union) is a new
  instance of the same union-member-presence family as keyed's withdrawn
  mechanism, one level shallower — and therefore possibly the cheaper end of that
  family.
* **The three keyed rows do not move at all** until the nullable-union census gap
  is answered; their read blocker is strictly behind it. Of the three, only
  `1.5.3|solid1` would then be reachable by 4.C+4.D; the two 3.0 rows stay
  honestly refused because Solid 2 states nothing about `mapArray` slot 0.

Remaining fail-closed / uncertifiable after all four, by construction and not by
omission:

* a read inside a non-dialect closure with no proven-invoking slot
  (`local-store`'s `Proxy` handler `get` method, 2 of its 5 witnesses;
  `addEventListener` shapes);
* a read in a loop body (`reach: Unknown`) against any future `min >= 1` read;
* a read through a genuinely computed member (`obj[key]`) — the path truncates and
  the use stays `unknownEscape`, deliberately;
* `mapArray` slot 0 under the v2 dialect, and any slot no dialect models;
* every read whose demanded root is `unknown`, `any`, or an unresolved generic —
  a separate, already-diagnosed class that gates two of these three rows anyway.
