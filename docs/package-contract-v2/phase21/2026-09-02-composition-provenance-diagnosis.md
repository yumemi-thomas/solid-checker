# Composition: intra-package operation composition (timer) and cross-package
# semantic-claim composition (until)

Read-only diagnosis. Repo `/Users/thomas/Documents/Github/solid-checker`, branch
`codex/phase19a-authenticated-proof-policy`, **HEAD `79e71286`** (verified).
Every measurement used a debug checker built by `make build-checker-debug`
(`bin/solid-typefacts` reused as found, stamp already at source
`6ae37483b7740fb7a0b21c8c160159b11102aa56c088c900074bbe98f40a5b3d`, build id
`dev`).

All temporary diagnostic patches (§0.1) have been reverted, the debug checker
rebuilt from the reverted source, and the controls re-measured against that
build. `git diff --stat` is empty and `git status --short` shows only the
pre-existing untracked `packages/cli/solid-reactivity.json.refusals.json`. No
`benchmarks/ecosystem/report-probes-*.md` was created.

Scratch: `<scratch>/compose/` — `base.json` (unpatched reproduction of the three
rows), `diag-timer*.jsonl` / `diag-until.jsonl` (certifier-side operation +
transcript dumps), `ir-timer.jsonl` (IR-side `SummaryRead` dump), `t2..t4.json`
and `u1..u3.json` (the §D ceiling runs), `controls.json` (the reverted-source
control set).

---

## 0. Headline, and four corrections to the brief

**Headline.**

| | timer (intra-package) | until (cross-package) |
|---|---|---|
| refusing predicate | `operation_input_parameter_root` (`type_facts.rs:2965`) | `require_parameter_flow` (`type_facts.rs:3164`, `Err` at `:3191`) |
| blocking premise | mechanism B (a `read` operation's `Reactive` input has no census-addressable site) | `argument_slot_is_proven_invoking`'s Tier A `target_module == "solid-js"` literal (`type_facts.rs:3565`) |
| producer fact missing | **yes** — `calleeSources` on `ImplementationCall` (protocol 12 → 13) | **no** — every fact needed is already on the wire at protocol 12 |
| dependency claim needed | none (same file, same artifact case) | `createBranch: callbacks[{from:{arg:0}}] + invoke` — **which the pipeline's own rootless receipt does not contain** |
| rows the slice would certify | **2** (both timer rows) but only with mechanism B *and* composition | **1** (until) but only after rootless's contract stops being vacuous |
| measured ceiling | §D.1 | §D.2 |

**Correction 1 — mechanism A already landed; the operation-input diagnosis's
§4.4 "option 1" is done.** `require_reactive_operation_input`
(`type_facts.rs:4168`), `reactive_operation_input_role` (`:4011`),
`traced_source_proves_role` (`:4081`), `traced_result_slot` (`:4052`),
`solid_dialect::unambiguous_reactive_result_slot` (`solid-dialect/src/lib.rs:341`)
and `solid_dialect::exports_value_from` (`:369`) all exist, protocol is **12**,
and `ImplementationCall.argument_sources` is on the wire
(`typefacts/src/invocation.rs:409-410`). All three marker rows certify (§E.6).
The §4.7 refusal message landed too. What did **not** land is mechanism B (the
`read` arm) — `reactive_operation_input_role` gates on
`operation.kind != OperationKind::Invoke` at `:4092` and its doc comment says so
in as many words. Recorded at `docs/precision-backlog.md:11214-11221`.

**Correction 2 — the composition rule alone unlocks zero rows.** Skipping *only*
`createIntervalCounter`'s read demands moves timer's first refusal to
`createPolled:read-0`, demand
`sha256:11db83ae87bdfb7c7cf42b64749106a7134547b94bf0a967671e1e1f54fc7f3f`, same
class (§D.1). The brief's "with all non-parameter operation-input demands
skipped, both timer rows certify" is right, but it measures mechanism B **and**
composition together. Composition is strictly the *second* half.

**Correction 3 — the IR does not discard the composed read's provenance.**
`SummaryRead` (`interproc.rs:53-60`) carries `symbol`, `display`, `kind`,
`declaration: Location`, `origin: Location` and `origin_context: String`, and
`propagate_summary_deltas` (`solid-reactive-ir/src/lib.rs:1645-1694`) clones the
row **verbatim** across the call edge. Measured (§B.2):
`createIntervalCounter`'s summary read is byte-identical to `createPolled`'s —
same symbol, same `origin` (the `depSignal()` site *inside createPolled*), same
`origin_context: "createPolled"`. The loss happens one layer later, in
`contract_export_function` (`contracts.rs:1191-1213`), which projects it to
`ContractReactiveRead { kind, label, parameter: None, path: None }`. So option 2
does **not** need the IR to start computing provenance; it needs the projection
to stop dropping it. That is a smaller change than the operation-input diagnosis
§4.4 assumed.

**Correction 4 — until's failing predicate is not `require_parameter_callback_flow`.**
The brief and the 2026-09-01 scoping study both point at the
`target_module != "solid-js"` `continue` in `require_parameter_callback_flow`
(now `type_facts.rs:3261`, fn at `:3198`). That literal is on the `callable-path`
*fallback* path, not on the failing one. The failing chain is:

```
ProofFamily::OperationCardinality  (type_facts.rs:2703)
  → require_operation_evidence            (:3648)
  → OperationKind::Invoke arm             (:3658-3666)
  → require_parameter_flow                (:3164)
  → implementation_call_is_executed       (:3398)
  → implementation_call_is_executed_within(:3414)   captured=true → enclosing
  → callable_is_executed_within           (:3450)   argument route at :3492-3520
  → argument_slot_is_proven_invoking      (:3563)   ← ALL FOUR TIERS ANSWER NO
  → Err("callback parameter has no exact direct-call or resolved-argument flow") (:3191)
```

`argument_slot_is_proven_invoking`'s own doc comment (`:3544-3548`) already names
this exact row as the intended refusal: *"`@solid-primitives/until` hands its
condition to `createBranch` from `@solid-primitives/rootless`, and nothing in
this artifact's transcript can prove what that function does. It needs an
accepted dependency contract, not a well-known-name list."* So the literal to
displace is the one at **`type_facts.rs:3565`**, inside
`argument_slot_is_proven_invoking`, and the fix is a **fourth tier** there — not
an edit to `require_parameter_callback_flow`.

### 0.1 Method

Three env-gated `// TEMPORARY DIAGNOSTIC` blocks, all reverted:

1. `type_facts.rs`, top of `require_operation_evidence`: a JSON line per
   operation demand carrying `{artifactCase, export, demand, family, operation,
   kind, at, schedule, tracking, cardinality, inputs, floor, callbacks,
   transcript}` (the whole `ExportImplementationTranscript`, which already
   derives `Serialize`), under `SOLID_DIAG_OPINPUT`.
2. The same function plus `require_operation_recursive_subject`: skip the read /
   non-parameter-input demand for exports named in `SOLID_DIAG_SKIP_READ_EXPORTS`,
   and skip the invoke demand for `SOLID_DIAG_SKIP_INVOKE_EXPORTS`; plus a skip in
   `verify_export_value_family` for `ArgumentBinding` and
   `CallablePath`/`CallbackBinding` under `SOLID_DIAG_SKIP_FLOW_EXPORTS`. Skipping
   by *export name* out of `proof_artifact_export(&proof.subject)` is what makes
   the ceiling attributable per export rather than per class — that is how §D.1's
   correction 2 was found.
3. `solid-reactive-ir/src/contracts.rs`, top of `contract_export_function`: a
   JSON line per summary node carrying `{node, path, span, exported, reads:[{symbol,
   display, kind, origin, origin_context, declaration}]}`, under
   `SOLID_DIAG_IR_READS`.

Every byte offset below was read from the **installed** package in a
`--keep-temp` install, and every producer fact from a live census dump — not from
prose and not from the package's own source tree.

Installs used (kept): `.../T/solid-checker-ecosystem-9ID78W` (timer, with
`solid-js@2.0.0-rc.0` + `@solidjs/web@2.0.0-rc.0`),
`.../T/solid-checker-ecosystem-rqui1p` (until, with `solid-js@1.9.14` and
`@solid-primitives/{rootless@1.5.4,utils@6.4.1}`),
`.../T/solid-checker-ecosystem-PtnN6q` (rootless, with `solid-js@1.9.14` +
`@solid-primitives/utils@6.4.1`). Outputs: `...-out-Ht4Yvu` (timer),
`...-out-L3Scie` (until), `...-out-pypbzC` (rootless).

### 0.2 Baseline reproduction

Both target rows reproduce exactly, and the `|head` twin is digest-identical to
`|floor`:

| row | status | demand | family |
|---|---|---|---|
| `@solid-primitives/timer@1.4.5-next.1\|solid2\|floor` | refused | `sha256:1aee58a94efcff998c64340690189232b89ab9cdfbaf4bc6d816b428b80adf62` | `operation-reachability` |
| `@solid-primitives/timer@1.4.5-next.1\|solid2\|head` | refused | same digest | `operation-reachability` |
| `@solid-primitives/until@0.1.1\|solid1\|only` | refused | `sha256:15fde3fc57c55117d494a2ece2bd31fbd42fb424f58cf0e6b998eaa5e9878251` | `operation-cardinality` |

```
Type Facts demand sha256:1aee58a9… is unsupported: operation input
artifact-case:5b9787fbde29759dd77afc0974f51ce4558822de811cb1f1a1496c7a4b386390:createIntervalCounter:operation:read-0[0]
is reactive/accessor, and the implementation census binds only parameter-rooted
operation inputs (family=operation-reachability)
```

```
Type Facts demand sha256:15fde3fc… is locally open: operation-cardinality
(artifact-case:d24421345876c98f1f7e7b7062e5b995a9a01ac776887b3af1b28a555f532388:until):
callback parameter has no exact direct-call or resolved-argument flow
```

---

## A. Current state of the wire and the certifier at `79e71286`

### A.1 The wire: `ImplementationCall` at protocol 12

`rust/crates/typefacts/src/invocation.rs:358-475`. Per-argument callable and
value identity **both exist**:

| field | line | what it is |
|---|---|---|
| `argument_sources: Vec<Vec<ImplementationValueSource>>` | `:409-410` | per written argument slot, the traced value provenance — the same walk `ReturnSite::sources` uses. Doc: *"An empty list means the producer traced nothing… never a claim about the slot at all."* |
| `argument_callables: Vec<ImplementationArgumentCallable>` | `:431-432` | per slot, the exact source ranges of the callables the slot **carries by identity**. `ImplementationArgumentCallable { argument: usize, locations: Vec<Location> }` at `:516-521` |
| `enclosing_callable: Option<Location>` | `:422-423` | the *innermost* callable containing the call; `captured` is true exactly when present |
| `default_library_invoker: Arc<str>` / `invoked_arguments: Vec<usize>` | `:438-443` | reviewed standard-library member by default-library symbol identity, re-checked against the verifier's own `DefaultLibraryInvoker` enum (`:530-560`) |
| `callee_directly_called_parameters` / `callee_invoked_parameters` / `callee_strongly_invoked_parameters` | `:450-461` | the resolved local callee's own treatment of its parameters |
| `callee_pending_invocations: Vec<CalleePendingInvocation>` | `:473-474` | the same two claims with the dialect premise deferred; `CalleePendingInvocation { parameter, strong, requires: Vec<InvokingSlotPremise> }` at `:486-493`, `InvokingSlotPremise { module, name, slot, argument_count }` at `:499-507` |

`ImplementationValueSource { path, kind: DirectCallable|CallResult, target,
target_name, target_module, target_path }` at `:606-620`.

There is **no** `ArgumentBinding` type anywhere in `rust/crates`
(`rg ArgumentBinding rust/crates` → no matches). The brief's `ArgumentBinding.slots`
is `ImplementationArgumentCallable.locations`, and `argumentSources DirectCallable`
is `ImplementationValueSource { kind: DirectCallable }` — both present.

**What is NOT on the wire: `calleeSources`.** The callee expression's *value*
provenance is not traced. `ImplementationCall` resolves a callee to
`target`/`target_name`/`target_module`/`declaration`/`callee_parameter`, but for
`depSignal()` (§B.1) that gives `declaration.kind == "BindingElement"`,
`qualifiedName == "createPolled.depSignal"` and nothing about the binding's
initializer. This is the single missing producer fact for mechanism B, and the
2026-09-02 backlog entry states it explicitly (`docs/precision-backlog.md:11221`:
*"No `calleeSources` producer field was added."*).

### A.2 The certifier's execution and invoking-slot predicates

* `implementation_call_is_executed` (`:3398`) → `implementation_call_is_executed_within`
  (`:3414`). Bounds `MAX_EXECUTION_PREMISE_DEPTH = 8`, `MAX_EXECUTION_PREMISE_NODES = 256`
  (`:3411-3412`). Rejects on `!floor.admits(call.reach)`; accepts immediately when
  `!call.captured`; otherwise requires `enclosing_callable` to be present (absence
  refuses, `:3434-3436`) and hands off to `callable_is_executed_within` **without
  charging depth** (`:3440`).
* `callable_is_executed_within` (`:3450`) has three edges: carried by a reachable
  implementation return whose `carry_reach` the floor admits (`:3461-3473`);
  carried by another callable's reachable return, recursively (`:3474-3491`); and
  the **argument route** (`:3492-3520`) — some `outer` call whose
  `argument_callables` names this exact callable at slot *n*, where
  `argument_slot_is_proven_invoking(outer, n)` and `outer` itself is executed at
  the same floor.
* `argument_slot_is_proven_invoking` (`:3563-3577`) — four tiers, all
  membership-reviewed:
  1. `!call.target.is_empty() && call.target_module.as_ref() == "solid-js" &&
     solid_dialect::unambiguous_callback_argument(&call.target_name, argument,
     call.argument_parameters.len())` — **the literal at `:3565`**;
  2. `DefaultLibraryInvoker::from_wire(&call.default_library_invoker)` whose own
     `invokes(argument)` holds *and* `call.invoked_arguments.contains(&argument)`;
  3. `call.callee_invoked_parameters.contains(&argument)`;
  4. `callee_pending_invocation_holds(call, argument, false)`.
  Its doc (`:3550-3562`) pins it as a **"can execute"** premise:
  *"a consumer may read 'this argument is a callback this implementation runs' and
  may not read 'it is run at least once'."*
* `callee_pending_invocation_holds` (`:3368-3389`) requires a non-empty `requires`
  list (an empty one is *malformed*, not unconditional) and every premise to
  satisfy `premise.module.as_ref() == "solid-js"` (`:3378`) plus
  `unambiguous_callback_argument`.
* Tier-A table: `solid_dialect::unambiguous_callback_argument`
  (`solid-dialect/src/lib.rs:252-266`) is cross-dialect unanimity, unchanged.
* The five `"solid-js"` literals in `type_facts.rs` today, verified one by one:
  `:3261` in `require_parameter_callback_flow` (fn at `:3198`) — the `continue`
  guard the scoping study named; `:3378` in `callee_pending_invocation_holds`
  (`:3368`); `:3565` in `argument_slot_is_proven_invoking` (`:3563`); `:3858` in
  `require_owner_operation_call` (`:3834`); `:4572` in
  `require_return_callable_source` (`:4542`). **Only `:3565` is on until's
  failing path.** Note that `traced_source_proves_role` (`:4081-4092`), the newest
  of these, already uses `solid_dialect::exports_value_from` instead of a
  literal — that is the shape a fix should copy.

### A.3 The accepted-dependency lane: what it delivers into `contract certify` today

**Nothing semantic, and it is never engaged by the ecosystem runner.**

1. **The runner never passes `--accepted-contracts`.** `run.mjs:1585-1629`
   (`attemptCertification`) spawns `certify` with `--package-root --integrity
   --catalog <out> --issuer-configuration --trust-configuration-output
   --audit-output [--proposal-refusal-audit] [--proposal]`. `--catalog` is the
   catalog it **publishes**, not one it reads. Each probe is certified
   independently; a sibling probe's receipt is written to a *different* output
   directory and never handed over.

2. **`CertificationPlan` carries no accepted dependency contract.**
   `contract_certification.rs:322-338`: `snapshot, verified_resolution,
   verified_closure, verified_exports, selected_candidate, candidates,
   demand_graph, artifact_witnesses, import_request, resolved_import,
   certification_sources`. The dependency edges live in
   `verified_closure.manifest().dependencies` as `AcceptedDependencyEdge {
   specifier, package_name, artifact_case, accepted_contract_digest }`
   (`artifact_resolution.rs:87-92`) — **identity only, no semantics**.

3. **Cross-package claim composition fails closed, by design, since 11fdc29a.**
   `authenticate_dependency_receipt` (`dependencies.rs:2010-2018`) resolves
   `requirement.semantic_claim_id()` against
   `AuthenticatedPolicy2Receipt::contains_closed_claim_id`
   (`contract_semantics.rs:408-423`), which enumerates the *dependency's* closed
   claims and compares `SemanticClaimId`s. Claim identity binds package and
   artifact identity (`contract_semantics.rs:370-399`,
   `canonical::semantic_claim_id`), and the demand graph names the **parent's**
   claim ID, so the two can never be equal → `MissingClosedClaim`
   (`dependencies.rs:2107-2110`). The backlog says exactly this
   (`docs/precision-backlog.md:10832-10842`): *"the graph carries no authenticated
   mapping from that parent claim to a dependency export/path. Cross-package
   semantic composition therefore now fails closed with `MissingClosedClaim`."*
   Corroborated in the report: `demandCountsByFamily` never contains
   `dependency-contract` for any of the three rows.

4. **But a channel already exists in the graph lane, one parameter away.**
   `verify_live_export_value_answer` already holds `dependencies: &[&CertificationPlan]`
   and passes it to `verify_export_value_subject(plan, proof, transcript,
   dependencies)` (`type_facts.rs:2343`) — while
   `verify_export_value_family(plan, proof, transcript)` on the very next line does
   **not** receive it. And `authenticated_dependency_declaration_target`
   (`type_facts.rs:2578-2597`) is the exact precedent for the binding half of
   until's premise: it accepts a compiler-resolved declaration path + name when
   the path is inside a dependency plan's own snapshot files **and**
   `dependency.verified_exports.has_declaration_target(path, declaration_name)`
   holds. `GraphExportValueRequest { plan, dependencies, sources }`
   (`type_facts.rs:722-726`) is where those plans come from.

**Answer to A's question, plainly:** the certifier **cannot** read a dependency's
`callbacks[]` invoke claim by exact claim id for a call whose target resolves to
that dependency's export. No code path in `type_facts.rs` reads any dependency's
`CallClaims`. The only claim-aware consumer is
`authenticate_dependency_receipt`, it reads by *parent* claim ID, and it fails
closed. The available authority in the graph lane is a dependency
`CertificationPlan` whose `candidates.proposal()` is a **proposal**, not a
receipt; the authenticated form (`AuthenticatedPolicy2Receipt`) exists only on the
accepted lane, which no gate and no benchmark currently drives.

---

## B. timer: the exact mechanism

### B.1 The installed source and the census

`node_modules/@solid-primitives/timer/dist/index.js`, 4818 bytes.

```js
function createPolled(fn, timeout, value, options) {        // 3801‥4293, decl 3810‥3822
  if (isServer) { const v = fn(value); return () => v; }
  const [tick, setTick] = createSignal(0);                  // 3940‥3955
  const [depSignal] = createSignal((prev) => {              // 3978‥4061, binding 3965‥3974
    tick(); return fn(prev !== void 0 ? prev : value);
  });
  const [polled, setPolled] = createSignal(depSignal(), options);  // 4092‥4126
  //                                       ^^^^^^^^^^^ 4105‥4116   ← THE READ
  createEffect(() => depSignal(), (newValue) => { setPolled(() => newValue); });
  //                    ^^^^^^^^^^^ 4148‥4159 inside arrow 4142‥4159 at slot 0
  createTimer(() => setTick((t) => t + 1), timeout, setInterval);  // 4212‥4274
  return polled;
}
const createIntervalCounter = (timeout, options) => {       // 4592‥4712
  if (isServer) return () => 0;
  return createPolled((prev) => prev + 1, timeout, -1, options);   // 4655‥4709
};
```

`createIntervalCounter`'s implementation census has **exactly one call** (live
dump, `diag-timer.jsonl`):

```json
{"location":{"startByte":4655,"endByte":4709},"kind":"call","reach":"reachable",
 "target":"symbol:h:b46a3dbdef6accdeb87321a0","targetName":"createPolled",
 "declaration":{"kind":"FunctionDeclaration","name":"createPolled",
   "qualifiedName":"createPolled","symbol":"symbol:h:b46a3dbdef6accdeb87321a0",
   "sourceFile":".../@solid-primitives/timer/dist/index.js",
   "location":{"startByte":3810,"endByte":3822}},
 "argumentParameters":[null,{"parameterIndex":0},null,{"parameterIndex":1}],
 "argumentSources":[[{"kind":"directCallable"}],[],[],[]],
 "argumentCallables":[{"argument":0,"locations":[{"startByte":4668,"endByte":4686}]}],
 "calleeDirectlyCalledParameters":[0],"calleeInvokedParameters":[0],
 "calleeStronglyInvokedParameters":[0],
 "calleePendingInvocations":[
   {"parameter":1,"strong":true,"requires":[{"module":"solid-js","name":"createEffect","slot":0,"argumentCount":2}]},
   {"parameter":3,"requires":[{"module":"solid-js","name":"createSignal","slot":1,"argumentCount":2}]}]}
```

No `captured`, no `enclosingCallable`, no `targetModule` (a local declaration).
Its reachable return site at 4648‥4710 carries
`sources: [{kind:"callResult", target:"symbol:h:b46a3dbd…", targetName:"createPolled"}]`.

**So the composing call's own identity is already exact and already on the wire:
a reachable, uncaptured `CallKind::Call` whose `declaration` is a
`FunctionDeclaration` named `createPolled`, by symbol, inside the same artifact
case's runtime module.** No producer change is needed for the composition
premise's *call* half.

`createPolled`'s own census has 12 calls; the two that matter, both
`reach: reachable`, both `targetName: "depSignal"`, both with
`declaration.kind == "BindingElement"`, `declaration.qualifiedName ==
"createPolled.depSignal"`, `declaration.location == 3965‥3974`,
`declaration.owners == [FunctionDeclaration createPolled 3810‥3822]`:

| bytes | captured | enclosingCallable |
|---|---|---|
| 4105‥4116 | *absent* (false) | *absent* |
| 4148‥4159 | true | 4142‥4159 (the `() => depSignal()` at `createEffect` slot 0) |

Neither carries `calleeSources`; the field does not exist.

### B.2 The IR's composition of `read-0`, and what it retains

Discovery: `discover_typed_accessors` (`interproc.rs:230-292`) walks
`file.ast.calls`, takes the project index's `type_descriptor` at the callee and
asks `solid_accessor_declaration(descriptor, dialect)` (`owners.rs:2606-2613`) —
a **Type Facts alias declaration plus origin module** matched against the
dialect's type-export index, not a name. It then filters through
`read_escapes_synchronous_extent` (`owners.rs:2554-2589`), which drops any read
sitting inside a dialect callback slot whose `callback_semantics_at(...).execution`
is `Tracked | Deferred`. It emits

```rust
SummaryRead { symbol: "typed:<path>\0<start>\0<end>", display: <callee text>,
              kind: Some("accessor"), declaration: <accessor alias decl loc>,
              origin: <call loc>, origin_context: nodes[owner].name }
```

Propagation: `discover_interprocedural_graph` records a call edge
`createIntervalCounter → createPolled`; `propagate_summary_deltas`
(`solid-reactive-ir/src/lib.rs:1645-1694`) copies each `SummaryRead` **verbatim**
through `push_unique`, whose key is `(display, origin, declaration)`
(`interproc.rs:75-94`).

Measured (`ir-timer.jsonl`, from the `SOLID_DIAG_IR_READS` patch):

```json
{"node":"createPolled","span":[3801,4293],"exported":false,"reads":[
  {"symbol":"symbol:h:4eb20340424e4028a31a2552","display":"depSignal","kind":"accessor",
   "origin":".../timer/dist/index.js:4105:4116","origin_context":"createPolled",
   "declaration":".../timer/dist/index.js:3965:3974"}]}
{"node":"createIntervalCounter","span":[4592,4712],"exported":false,"reads":[
  {"symbol":"symbol:h:4eb20340424e4028a31a2552","display":"depSignal","kind":"accessor",
   "origin":".../timer/dist/index.js:4105:4116","origin_context":"createPolled",
   "declaration":".../timer/dist/index.js:3965:3974"}]}
```

Three facts follow, and each one matters:

1. **The composed read row retains the `(origin, origin_context, declaration,
   symbol)` of the site it was composed from.** `createIntervalCounter`'s row
   names `depSignal()` at 4105‥4116 inside `createPolled`. The IR does **not**
   discard the `(export, operation)` it was composed from; it discards the
   *projection* of it.
2. **Only ONE read survives, and it is the uncaptured one.** The captured
   `depSignal()` at 4148‥4159 is absent — `read_escapes_synchronous_extent`
   dropped it, because Solid 2's `CreateEffect` slot 0 is `Tracked`
   (`solid_2.rs:572-574`). So the deduplication-by-`(kind,label)` in
   `contract_export_function` never even has to collapse them here.
3. **`origin_context` is the *discovering owner's name*, not an export name.**
   Here it happens to be the export `createPolled`, but a read discovered in a
   private helper would name the helper. A sound consumer rule must resolve the
   owner to an export of the same artifact case, not read `origin_context` as one.

Projection (where provenance is lost), `contracts.rs:1191-1213`:

```rust
let reactive_read = ContractReactiveRead {
    kind: read.kind.clone().unwrap_or_else(|| "accessor".into()),
    label: /* bundled-return label or read.display */,
    parameter: None, path: None,
};
seen_reactive_reads.insert((kind, label)).then_some(reactive_read)
```

`ContractReactiveRead` (`solid-reactive-ir/src/lib.rs:1054-1070`) has only
`{kind, label, parameter, path}` — no site, no owner, no symbol. Then
`inferred_contract.rs:145-184` `normalize_export` maps `(parameter: None, _)` to
`ValueShape::Reactive { role: Accessor, resource: None, capabilities: Unknown }`
(`:168-172`) and drops the label; `operation()` (`inferred_contract.rs:344-368`)
stamps every inferred operation with `at: Some(Event::Call)`,
`schedule: Some(Schedule::SameStack)`, `tracking: Untracked`,
`cardinality { scope: Call, min: 0, max: Many }`.

The emitted rows (from the kept output document):

```json
createPolled            → {"id":"read-0","kind":"read","at":{"event":"call","schedule":"same-stack"},
                           "count":{"scope":"call","min":0,"max":"many"},"tracking":"untracked",
                           "inputs":[{"kind":"reactive","role":"accessor"}]}
createIntervalCounter   → identical read-0 row, plus callback-0 (invoke, queued, tracked, from arg 0)
                           and return (reactive/accessor)
```

### B.3 Is the composed read's `same-stack` stamp honest?

**Yes, for this row, and the reason is structural rather than lucky.**

* The surviving read is at 4105‥4116, `captured: false`, in `createPolled`'s own
  body — same-stack in `createPolled`.
* `createIntervalCounter` reaches it through a **direct, reachable, uncaptured
  call** (`return createPolled(...)` at 4655‥4709). A direct call is the same
  stack, so the composed claim `at: call / schedule: same-stack` is true of
  `createIntervalCounter` too.
* The read the rule **must not** carry — 4148‥4159, inside the closure
  `createPolled` hands to `createEffect` — is already excluded upstream, by
  `read_escapes_synchronous_extent`, before the summary is built. So the IR's
  summary for `createPolled` contains only same-stack reads *by construction*,
  and composing them through a direct call preserves that.

The measured demand attributes confirm it: floor `MayExecute` (because
`min == Some(0)`, `operation_reachability_floor` at `:3129-3135`), `at: Some(Call)`,
`schedule: Some(SameStack)` for both `createPolled:read-0` and
`createIntervalCounter:read-0`.

**Two caveats a rule must carry explicitly, not assume:**

* `read_escapes_synchronous_extent` is *only* a dialect-callback filter. A read
  inside a non-dialect closure the callee stores (an `addEventListener` handler, a
  returned closure) is **not** filtered, and it would be published `same-stack`
  wrongly — that is exactly the 2026-09-02 parameter-read diagnosis §4.A defect
  (`inferred_contract.rs:356` stamping a schedule it never derived), and it is
  *upstream* of composition. Composition inherits the defect rather than creating
  it; the composition premise must therefore be limited to a **direct, uncaptured,
  reachable call** at every hop, so it never *adds* a non-same-stack link of its
  own.
* Propagation is transitive over call edges with no bound visible at
  `propagate_summary_deltas`, so a composed row can be many hops deep. The
  consumer rule must bound its own recursion (the certifier already has
  `MAX_EXECUTION_PREMISE_DEPTH = 8` / `MAX_EXECUTION_PREMISE_NODES = 256` as
  precedent) and must prove *each* hop, not lexical or transitive containment.

### B.4 Option 2's wire footprint

The provenance is a **new positive claim** in the contract model: "this read
happens through my call to *E*". So it needs (a) a place on the wire, (b) a place
in the canonical digest, and (c) its own demand.

* **Model.** `Operation` (`contract_semantics.rs:1094-1107`) gains e.g.
  `via: Option<ComposedFrom>` where `ComposedFrom { export: String, operation:
  OperationId }`. `canonical::operation` (`canonical.rs:354-372`) gains one
  `self.option(operation.via.as_ref(), …)` line — which **moves every semantic
  digest and every claim ID of every contract that carries a composed operation**,
  hence every acceptance digest in the corpus, and `SEMANTIC_MODEL_VERSION`
  should be reviewed alongside it.
* **Schema.** `schema/solid-reactivity.schema.json`'s `$defs.operation` is
  `{type: object, additionalProperties: false, required: [id, kind], properties:
  {...}}`. Adding one optional property is an **additive** change to
  `schemaVersion: 1` — permitted by AGENTS.md ("keep version 1
  backward-compatible and update validation/tests for every additive field"). It
  is *not* a `schemaVersion` bump. But `additionalProperties: false` means an
  older validator rejects the new field, so bundled contracts, fixtures, the
  Rust/JS validators and `jq empty schema/...` all move together.
* **Type Facts protocol.** The composition premise itself needs **no** producer
  fact: `declaration` (with `owners` and `symbol`), `reach`, `kind`, `captured`
  and `enclosing_callable` are all already stated (§B.1). Protocol stays **12**
  *for composition*.
* **Mechanism B, which composition depends on, does need protocol 12 → 13**:
  `calleeSources: Vec<ImplementationValueSource>` on `ImplementationCall`, filled
  by the existing `returnValueSourcesLocked` applied to `node.Expression()`. Its
  identifier arm (already tightened — single declaration, no other assignment, no
  rest, no default, reference at/after the binding's declaration end; see
  `docs/precision-backlog.md:11259-11276`) would turn `depSignal()`'s
  `BindingElement` into `CallResult { target_name: "createSignal", target_module:
  "solid-js", target_path: [tuple 0] }`, which
  `solid_dialect::unambiguous_reactive_result_slot("createSignal",
  TupleItem(0)) == Some(Accessor)` answers. That costs
  `TYPE_FACTS_HANDSHAKE_PROTOCOL 12 → 13` (`typefacts/src/v3.rs`,
  `apps/solid-typefacts/internal/typefacts/protocolv3.go` + its test), a new
  `typefacts-v1.schema.json` digest, `scripts/build-typefacts.sh`, and a
  `bin/solid-typefacts.buildinfo` stamp change that escalates `verify-delta` to
  full `make verify`.

---

## C. until: the exact mechanism

### C.1 The installed source

`node_modules/@solid-primitives/until/dist/index.js` (513 bytes; `until`'s
implementation location is 503‥508):

```js
import { createBranch } from "@solid-primitives/rootless";
import { createComputed, createMemo, onCleanup } from "solid-js";
var until = (condition) => createBranch((dispose) => {   // call 183‥490, arrow 196‥489
  const memo = createMemo(condition);                    // call 226‥247, binding 219‥223
  const promise = new Promise((resolve, reject) => {      // construct 267‥439, arrow 279‥438
    createComputed(() => {                                // call 306‥410, arrow 321‥409
      if (!memo()) return;                                // 340‥346
      resolve(memo());                                    // 370‥385 / memo() 378‥384
      dispose();                                          // 393‥402
    });
    onCleanup(reject);                                    // 416‥433
  });
  promise.dispose = dispose;
  return promise;                                         // return 472‥487
});
```

The callback is not invoked inside the closure — `condition` **flows** into
`createMemo` at argument 0 inside the closure `until` hands to `createBranch`.

### C.2 The census, and exactly which premise fails

Live dump (`diag-until.jsonl`), `complete: true`, `openReasons: null`:

```json
{"location":{"startByte":183,"endByte":490},"kind":"call","reach":"reachable",
 "targetName":"createBranch","targetModule":"@solid-primitives/rootless",
 "target":"symbol:h:88abe4c12934a93ab440d257",
 "declaration":{"kind":"VariableDeclaration","name":"createBranch",
   "sourceFile":".../node_modules/@solid-primitives/rootless/dist/index.d.ts",
   "location":{"startByte":895,"endByte":907}},
 "argumentSources":[[{"kind":"directCallable"}]],
 "argumentCallables":[{"argument":0,"locations":[{"startByte":196,"endByte":489}]}]}

{"location":{"startByte":226,"endByte":247},"kind":"call","reach":"reachable",
 "captured":true,"enclosingCallable":{"startByte":196,"endByte":489},
 "targetName":"createMemo","targetModule":"solid-js",
 "argumentParameters":[{"parameterIndex":0}],"argumentSources":[[]]}
```

The chain (§0, correction 4) reaches
`argument_slot_is_proven_invoking(createBranch_call, 0)` and every tier answers
no:

| tier | why no |
|---|---|
| dialect (`:3565`) | `target_module == "@solid-primitives/rootless"` ≠ `"solid-js"` |
| default library | `default_library_invoker` empty |
| callee's own body | `callee_invoked_parameters` empty — the callee is an *external* `.d.ts` declaration; the producer has no body to census |
| pending | `callee_pending_invocations` empty |

`argument_callables[0] == [196,489]`, byte-identical to the `createMemo` call's
`enclosingCallable`, so the argument route's *identity* half is already exact and
already satisfied. Only the invoking-slot answer is missing.

The other two edges of `callable_is_executed_within` genuinely do not apply:
`until`'s only reachable return is the `createBranch(...)` expression itself,
which carries no callables; the arrow at 196‥489 returns `promise`, carrying
none (`callableReturns[0].returns[0].carriedCallables == []`).

### C.3 The rootless receipt — and the finding that falsifies the scoping study's acceptance row

`@solid-primitives/rootless@1.5.4|solid1|only` **certifies** today (verified
§E.6). Its published catalog is at
`<out>/…rootless….json.accepted-catalog/objects/9f13ec94….main.json`, with the
receipt at `…/488a7ad2….receipt.json` and the trust/issuer bytes under
`…accepted-catalog.authority/{trust,issuer}.json`.

**The signed main contract is vacuous:**

```json
"exports": {"createBranch":"summary-ee9d83c3…","createCallback":"summary-ee9d83c3…",
            "createDisposable":"summary-ee9d83c3…","createHydratableSingletonRoot":"summary-ee9d83c3…",
            "createRootPool":"summary-ee9d83c3…","createSharedRoot":"summary-ee9d83c3…",
            "createSingletonRoot":"summary-ee9d83c3…","createSubRoot":"summary-ee9d83c3…"},
"summaries": {"summary-ee9d83c3e1d5c3193749e139525d7cc17b44b7b263b49f6e4b97e20ee8b67a6b":
              {"call":{},"shape":"callable"}}
```

All eight exports share one `{"call":{}}` summary. Its proposal has
`positiveOperations: 0`, `unresolvedClaims: 80`, `closureCandidates: 0`. The
cause: rootless imports `@solid-primitives/utils@6.4.1`, which resolves External
with no accepted contract, so `artifact-resolution.mjs:1695-1705` pushes an
`unaccepted-external-dependency` hazard with `affectedDomains: [...DOMAIN_NAMES]`
— every domain of every export goes open. rootless therefore certifies
**because it claims nothing**.

The claim the scoping study quotes —

```json
"createBranch": {"call":{"callbacks":[{"from":{"arg":0,"path":[]},"operation":"callback-0"}],
  "operations":[{"id":"callback-0","kind":"invoke","at":{"event":"call","schedule":"same-stack"},
    "count":{"min":0,"max":"many","scope":"call"}, …}],
  "closed":["callbacks","reads","creates","returns"]}}
```

— is real, but it lives in `benchmarks/package-contract-v2/phase14/solid-v1-authority/rootless-root-default.json`
(summary `summary-5e5c4fa4…`), a **phase-14 authority document**, not in anything
this pipeline issues. And it is bound to a *different artifact case*: that
document's `artifact.closureSha256` is
`9fda42d28be8ce57177d1d050b38513a3a220ebd6cab09a4cb3dbf4ce47c7471` and its
`authority-index.json` case lists `@solid-primitives/utils@6.4.1` as a **closure
package**, whereas the ecosystem probe's closure digest is
`d1c21e51a8aca30c2927df712e542d00f50178d7610bed42eb9cfbffac722326` with utils as
an *unaccepted external dependency*. Same `package.integrity`, same runtime
`sha256:0b7d93cf…`, **different closure identity** — so the phase-14 document
could not authenticate against this install even if the accepted lane were wired
(`validate_package_identity` / `policy2_resolved_import_root`, scoping study §4
rule 4).

**Consequence for C: until's dependency premise has no producer today.** Stage 3
of the scoping study is blocked one level deeper than it recorded: not on the
join, but on rootless publishing the claim at all, which is itself blocked on
`@solid-primitives/utils` being accepted (a chain-depth-2 problem the study
identified for `locator` and missed here).

Also: `until`'s own install carries `@solid-primitives/utils@6.4.1` directly, and
its emitted contract has one positive operation (`until:callback-0`) with **15
unresolved claims** and no `closed` list — the study's §2.1 census reproduces
exactly.

### C.4 The sound premise, and every trap it must not clear

The premise the brief proposes is right in shape. Stated precisely as a fourth
tier of `argument_slot_is_proven_invoking`:

> Slot *N* of `call` invokes what it is handed when **all** of:
> 1. `is_call_expression(call)` (a construction is a different claim);
> 2. `call.declaration` is present and its `location.path`/`name` bind, by the
>    `authenticated_dependency_declaration_target` (`type_facts.rs:2578-2597`)
>    relation, to a dependency whose contract this plan **authenticated** — the
>    dependency's own snapshot files plus
>    `verified_exports.has_declaration_target(path, name)`; and the same
>    dependency's *runtime* binding resolves the same export name;
> 3. that dependency's accepted contract, for the artifact case its receipt
>    selected, carries `callbacks` as a **closed** domain (`KnowledgeSet::Complete`)
>    containing an entry with `from == ValueSource::Parameter { index: N, path: [] }`;
> 4. that entry's `operation` resolves in the same export to an operation with
>    `kind == Invoke`;
> 5. `call.argument_callables` names the demanded callable at exactly slot *N*
>    (identity, never containment);
> 6. `call.argument_parameters.len()` — the written argument count — is recorded
>    in the witness site, because a claim about "argument 0" of a 1-argument call
>    and of a 4-argument call are different claims about the same slot only if
>    the dependency's own signature agrees.
>
> The witness site records the dependency node digest, the receipt digest, the
> export name, the slot, and the claim id — so the receipt says *which*
> dependency proved it.

**`count.min: 0` is not an obstacle here, and the scoping study's rule 7 does not
bite.** `ProofFamily::OperationCardinality` (`type_facts.rs:2703-2723`) refuses
outright unless the operation's cardinality is *exactly*
`{scope: Call, min: 0, max: Many}` — "runtime implementation census cannot prove a
tighter operation cardinality" — and then runs the **same**
`require_operation_evidence` at floor `MayExecute`. Measured: until's
`callback-0` is `Cardinality { scope: Some(Call), min: Some(0), max: Some(Many) }`,
floor `MayExecute`. So until's `operation-cardinality` demand is a *zero-or-more
per call* demand, which is exactly the strength
`argument_slot_is_proven_invoking`'s doc (`:3556-3561`) says every tier may
supply, and exactly the strength rootless's `count {min: 0, max: many, scope:
call}` claim carries. **until can certify under this rule.** What must stay
refused is any *future* operation whose `min >= 1`: `operation_reachability_floor`
would return `Reachable`, and the tier must not be read as a lower bound. Add a
regression that pins that.

Must-not-clear traps (scoping study §4 / §5 stage 3, restated against the code):

1. **Closed `callbacks` with no entry for N.** Absence inside a closed domain is
   a *negative* claim about N, so the demand must open — and the refusal must say
   "the dependency's closed callbacks claim has no entry for argument N", not
   "unknown".
2. **Open `callbacks` (`KnowledgeSet::Unknown`).** Contributes nothing. This is
   *the* rootless case today (§C.3): `{"call":{}}`. The trap and the live row
   coincide, which makes it cheap to pin.
3. **`arg: 1` must not clear `arg: 0`.** Slot equality, not membership.
4. **A refused / absent receipt contributes nothing.** `contract_interface.rs:450-454`
   quarantine + `consumer.rs:88-93,139,167,196-197`; a lookup returns
   `SemanticQueryError::MissingImport`, an explicit finding, not silence.
5. **Version / integrity / closure mismatch refuses fatally.** The phase-14
   rootless document is the live example: right version, right runtime digest,
   wrong closure digest.
6. **A `construct` site is not a `call`.** `new D.something(cb)` is a claim no
   family here was reviewed for; `is_call_expression` first.
7. **The parent may not nominate its own issuer** (`contract_interface.rs:431-433,457`).
8. **until's 15 open claim domains must not all silently close.** Clearing the
   `UnacceptedExternalDependency` hazard is a *separate* consequence of accepting
   rootless; each domain still needs its own proof. Pin the count.
9. **A composed claim must not be inherited transitively.** rootless's own claim
   about `createBranch` depends on `@solid-primitives/utils`; if rootless ever
   publishes a real claim it must arrive with its own receipt, not by the parent's
   (`dependencies.rs:1814-1822`, `:1883-1891`).

---

## D. Ceilings, measured by diagnostic skip, then reverted

### D.1 timer

| run | skip | outcome | first refusal |
|---|---|---|---|
| baseline | — | refused | `sha256:1aee58a9…` `operation-reachability`, `createIntervalCounter:read-0[0]` is `reactive/accessor` |
| skip 1 | `createIntervalCounter` read demands, `require_operation_evidence` only | refused | `sha256:32a66cdd…` `recursive-value-shape`, **same** `createIntervalCounter:read-0[0]` — the third family goes through `require_operation_recursive_subject` (`:4347`), a second call site |
| skip 2 | `createIntervalCounter` read demands, **all three families** | refused | **`sha256:11db83ae87bdfb7c7cf42b64749106a7134547b94bf0a967671e1e1f54fc7f3f`** `operation-reachability`, `createPolled:operation:read-0[0]` is `reactive/accessor` |
| skip 3 | `createIntervalCounter` **and** `createPolled` read demands | **certified**, both `floor` and `head` | — |

**So the composition rule alone certifies nothing.** timer's ceiling is 2 rows
and it takes mechanism B *plus* composition. `createPolled` is the mechanism-B
row; `createIntervalCounter` is the composition row; neither alone moves a
verdict.

timer plans 7 `operation-reachability`, 7 `operation-cardinality`, 9
`recursive-value-shape`, 7 `callable-path`, 3 `rest-spread-coverage`, 3
`selected-signature`, 2 `argument-binding` demands over 1 artifact case; 6 of the
positive operations are `createPolled`/`createIntervalCounter`/`makeTimer` rows
and 87 claims stay unresolved.

### D.2 until

| run | skip | outcome | first refusal |
|---|---|---|---|
| baseline | — | refused | `sha256:15fde3fc…` `operation-cardinality` |
| skip 1 | `until` invoke-operation demands (`operation-reachability` + `operation-cardinality`) | refused | **`sha256:88e245af3c3618b040767cfbb1970b48566c6622ad9cde704fe70a4ce3d7538e`** `argument-binding`, **identical reason** — `require_parameter_flow` again, via `callback_parameter_source` + `callback_reachability_floor` (`:2630-2633`) |
| skip 2 | invoke demands **and** `argument-binding` + `callable-path`/`CallbackBinding` | **certified** | — |

until plans 14 demands. Four of them share the one failing premise:

* `sha256:15fde3fc57c55117d494a2ece2bd31fbd42fb424f58cf0e6b998eaa5e9878251` — `operation-cardinality`
* `sha256:8e91609d0465eb40e1b9f35b6a7c0732918000953a83395dc9ee4764b24dc9a2` — `operation-reachability`
* `sha256:88e245af3c3618b040767cfbb1970b48566c6622ad9cde704fe70a4ce3d7538e` — `argument-binding`
* one of `sha256:011ed77d3b9c8465326c6e3b4a0278950482ac0a98045bf27d4e98a486e96292` /
  `sha256:89b41ba08d23d96a2e5089b7c33726652b0e23f2b09a9de3dc228e445163062a` — `callable-path`
  (the `CallbackBinding` one; the other is the export-root `RecursiveValue`)

So **one premise clears all four, and until certifies — 1 row.** Nothing else in
until's plan is blocked.

---

## E. Implementation plan

The two rows are **two independent slices with no shared code**, and they should
not be sold as one "composition" feature. Slice 1 is the honest first commit;
slice 2 is blocked on a prerequisite outside itself.

### E.1 Slice 1 — mechanism B (`createPolled`), the prerequisite

*Not part of "composition", but timer certifies nothing without it.*

* **Producer** (`apps/solid-typefacts/internal/typefacts/tsgo/export_value_transcripts.go`,
  census builder at `:346`): add `calleeSources` filled by
  `returnValueSourcesLocked(node.Expression())`. Rust mirror
  `typefacts/src/invocation.rs` (`ImplementationCall`, `deny_unknown_fields`),
  with the same "absence is never authority" doc as `argument_sources`.
* **Protocol/schema**: `TYPE_FACTS_HANDSHAKE_PROTOCOL 12 → 13`
  (`typefacts/src/v3.rs`, `protocolv3.go` + `protocolv3_test.go`); one new property
  under `$defs.implementationCall` in `schema/typefacts-v1.schema.json`
  referencing the existing `$defs.implementationValueSource`; regenerate
  `TYPE_FACTS_SCHEMA_SHA256` (current protocol-12 digest
  `sha256:3d97fa9a3cb8d0b0ac1ca7f8b116a07cfa5774efdefcb7ddc1d8ab51d72f60e0`);
  `scripts/build-typefacts.sh`; the `.buildinfo` stamp escalates `verify-delta`
  to full `make verify`.
* **Certifier**: extend `reactive_operation_input_role` (`:4011`) to admit
  `OperationKind::Read` with a *separate* discharge function beside
  `require_reactive_operation_input` (`:4168`), and wire it into the `Read` arm of
  `require_operation_evidence` (`:3667-3675`) as well as the recursive arm.
  Reuse `traced_source_proves_role` (`:4081`) verbatim.
* **The rule, and it is weaker than mechanism A on purpose.** A `read` operation
  carries no span, so it cannot be matched to *a* call. The strongest sound rule
  is existential-plus-no-counterexample:
  > over the calls with `is_call_expression`, `floor.admits(reach)` and
  > `!captured`: **at least one** must have a `calleeSources` entry with
  > `path.is_empty()` that `traced_source_proves_role(_, role)` accepts, and
  > **none** may have one that traces to an unambiguous dialect result of a
  > *different* role. A call whose `calleeSources` is empty is irrelevant to the
  > role claim (it is not an accessor read at all); a call tracing to a `Setter`
  > refuses.
  Keep `!captured` — the parameter-read diagnosis §0.3 established that this veto
  is the *only* enforcement anywhere of the `at: call / schedule: same-stack`
  claim these operations publish, and `ContractReactiveRead` has no column in
  which a different schedule could be stated. Record the weakness in
  `docs/precision-backlog.md`.
* **Ceiling**: `createPolled:read-0` discharged; timer still refuses on
  `createIntervalCounter`. **0 rows.**

### E.2 Slice 2 — intra-package operation composition (`createIntervalCounter`)

* **IR (`solid-reactive-ir`)**
  - `interproc.rs:53-60`: add `owner: Option<SymbolId>` to `SummaryRead` (the
    discovering node's symbol), set at `discover_typed_accessors:277-290` from
    `nodes[owner].symbol`. Do **not** reuse `origin_context` — it is a name, and a
    name is not identity.
  - `contracts.rs:1191-1213`: carry it onto `ContractReactiveRead` as
    `composed_from: Option<...>`, populated only when the read's `owner` is a
    *different* node than the one being projected; `None` for a read the export
    performs itself. Extend the dedup key so two rows differing only in
    provenance do not collapse.
  - `contract_export_summaries` / `contract_export_fragment` (`contracts.rs:1841`)
    already own the node↔export-name mapping; resolve `owner` to an export name
    there, and **publish nothing** when the owner is not an export of this
    artifact case. An unresolvable owner publishes no provenance — never a
    guessed one.
  - **Fail-closed direction**: a read row with no provenance keeps today's
    behaviour (refused). Provenance may only *add* a discharge route.
* **Contract model + wire**
  - `Operation.via: Option<ComposedFrom>` (`contract_semantics.rs:1094-1107`);
    `canonical::operation` (`canonical.rs:354-372`) folds it in; `validate.rs`
    treats it as part of the read operation's claim.
  - `inferred_contract.rs:145-184` reads `composed_from` instead of dropping it;
    `contract_document.rs` emits `via`.
  - `schema/solid-reactivity.schema.json` `$defs.operation`: one optional
    `via: {export, operation}` property. **Additive to `schemaVersion: 1`** — not
    a version bump — but `additionalProperties: false` means every bundled
    contract, fixture, snapshot and validator moves in the same commit, and the
    canonical digest change moves every acceptance digest for a contract that
    carries a composed operation. `jq empty schema/...` plus
    `bun scripts/dialect-manifests.mjs validate` are in the universal set already.
* **Demand inventory (`contract_semantics/certification.rs:552-571`)**: the
  provenance is a positive claim, so the composed operation's demand must *be*
  the composition demand rather than the plain one — otherwise the emitted `via`
  is a fact carried without a witness, which is exactly what policy 2 exists to
  prevent.
* **Certifier (`type_facts.rs`)**: in `require_operation_evidence`'s `Read` arm
  and `require_operation_recursive_subject`, when the operation carries `via`,
  discharge through a new `require_composed_operation` requiring **all** of:
  1. some `ImplementationCall` in *this* export's census with
     `is_call_expression`, `floor.admits(reach)`, `!captured`, whose
     `declaration` is present and whose `declaration.location.path` +
     `declaration.name` resolve to `via.export`'s own snapshot-verified runtime
     binding **in the same artifact case** (`plan.verified_exports`,
     `plan.candidates.proposal().artifact_case(...)`);
  2. `via.operation` exists in that export's semantics with the **same kind, same
     inputs and same `at`/`schedule`/`cardinality`** as the composed row;
  3. that export's own demand for `via.operation` is **discharged in this same
     plan** — recorded as a witness dependency, so the receipt names it. Order
     the schedule so the composing target's demand is verified first, or make the
     rule recursive with an explicit depth bound.
  The witness site records `composed:<via.export>:<via.operation>:<call bytes>`.
* **Must-not-clear** (each needs a fixture that stays refused):
  1. the composing call resolves to an export of a **different** artifact case or
     a different package;
  2. `via.export`'s own demand for `via.operation` is **refused** — the composed
     row must refuse too, and the audit must name the target;
  3. the composing call is `captured` (inside a closure the export hands away or
     stores) — a read reached only through a deferred closure is not `same-stack`,
     so the premise must refuse rather than compose;
  4. the composing call is a `construct`;
  5. the composing call is `Unreachable`;
  6. `via.operation` has a *different* `schedule` or a tighter `cardinality` than
     the composed row;
  7. "some other export in this artifact case proves it" — provenance must be the
     exact named target, never a search;
  8. a cycle / an 8-deep chain — bound it, and refuse at the bound.
* **Ceiling with E.1**: both timer rows. **2 rows.**

### E.3 Slice 3 — cross-package invoke composition (`until`)

* **Prerequisite that is not in this slice, and gates it entirely**: rootless
  must publish `createBranch: callbacks[{from:{arg:0}}] + invoke + closed`, which
  it does not (§C.3). That needs `@solid-primitives/utils@6.4.1` to be an accepted
  dependency of the rootless probe, i.e. the scoping study's **stage 1** (route an
  issued receipt into the parent's `--accepted-contracts`) applied to a
  *chain-depth-2* graph. Until that lands, slice 3 has no acceptance row at all
  and should not be started.
* **Certifier, once the claim exists**:
  - Thread `dependencies: &[&CertificationPlan]` (or, on the accepted lane, the
    authenticated receipts) from `verify_live_export_value_answer`
    (`type_facts.rs:2343`) into `verify_export_value_family` (`:2599`) — it is
    already in scope on the line above, passed to `verify_export_value_subject`
    (`:2423`) — and down through `require_operation_evidence` (`:3648`),
    `require_parameter_flow` (`:3164`), `require_parameter_callback_flow`
    (`:3198`), `implementation_call_is_executed` (`:3398`),
    `implementation_call_is_executed_within` (`:3414`),
    `callable_is_executed_within` (`:3450`) to
    `argument_slot_is_proven_invoking` (`:3563`).
  - Add the fourth tier of §C.4 to `argument_slot_is_proven_invoking` at `:3565`,
    replacing the literal with "dialect **or** an authenticated composed claim".
    Reuse `authenticated_dependency_declaration_target` (`:2578`) for the binding
    half. Do **not** touch `require_parameter_callback_flow`'s `:3261` literal in
    this slice — it is a different family (`callable-path`'s fallback) and a
    separate review; `require_owner_operation_call`'s `:3858` and
    `require_return_callable_source`'s `:4572` are out of scope entirely.
  - Decide and document which authority the tier reads. A dependency
    `CertificationPlan.candidates.proposal()` is **unauthenticated** — reading it
    would make an ecosystem row certify on a sibling proposal, which is the
    scoping study's §3.1 lane separation. The authenticated form is
    `AuthenticatedPolicy2Receipt` on the `--accepted-contracts` lane, whose
    `contains_closed_claim_id` (`contract_semantics.rs:408`) already exists but
    resolves by *parent* claim id. This tier needs a **new** query — "does this
    dependency's contract carry a closed `callbacks` entry for arg N of export E"
    — beside it, not instead of it, and the `MissingClosedClaim` repair of
    11fdc29a must stay as it is.
* **Producer/schema footprint: none.** Protocol stays at whatever slice 1 leaves.
  No public contract schema change.
* **Also unblocks**: `require_parameter_callback_flow`'s `callable-path` demands
  by the same premise, once threaded — but do not claim that until measured.
* **Ceiling**: **1 row** (until), and only behind the prerequisite.

### E.4 Acceptance digests

Slice 1 + 2 (timer certified):
`artifact-case:5b9787fbde29759dd77afc0974f51ce4558822de811cb1f1a1496c7a4b386390`;
the demands that must flip from refused to satisfied are
`sha256:1aee58a94efcff998c64340690189232b89ab9cdfbaf4bc6d816b428b80adf62`
(`operation-reachability`, `createIntervalCounter:read-0`),
`sha256:32a66cddfc8de242422238d0f12f2cc8181227e2e7855a5ed8bcb9097861e728`
(`recursive-value-shape`, same subject),
`sha256:11db83ae87bdfb7c7cf42b64749106a7134547b94bf0a967671e1e1f54fc7f3f`
(`operation-reachability`, `createPolled:read-0`), plus their
`operation-cardinality` twins from timer's plan of 7/7/9. **These digests will
move** the moment `Operation.via` enters the canonical digest, so pin the
*post-change* digests from the first green run and record the pre-change ones as
the baseline they replaced.

Slice 3 (until certified):
`artifact-case:d24421345876c98f1f7e7b7062e5b995a9a01ac776887b3af1b28a555f532388`;
demands `sha256:15fde3fc…` (`operation-cardinality`),
`sha256:8e91609d…` (`operation-reachability`), `sha256:88e245af…`
(`argument-binding`), and the `CallbackBinding` one of
`sha256:011ed77d…` / `sha256:89b41ba0…`. until's contract carries no composed
operation, so these digests are stable across slice 2.

### E.5 Fixtures

No fixture corpus in this repository certifies — `scripts/contract-corpus.mjs`
runs `contract generate` only, and the ecosystem benchmark is the sole
certification driver (`docs/precision-backlog.md:11340-11344`). So every trap in
E.1/E.2/E.3 is pinned **where it is decided**: producer traps in
`apps/solid-typefacts/internal/typefacts/tsgo/export_value_transcripts_test.go`,
certifier traps as unit tests in `type_facts.rs` beside
`reactive_operation_input_refuses_every_unproven_provenance` and its two
siblings, IR/generation traps as `fixtures/package-contracts/` rows registered in
`corpus.json`.

Two new generation fixtures:

* `fixtures/package-contracts/composed-operation-provenance` — an export whose
  read is composed from a sibling export through a direct call (positive: `via`
  published); a composed read reached through a closure the export returns
  (negative: no `via`, and no `same-stack` read row at all); a read composed from
  a **non-exported** private helper (negative: no `via` — the owner is not an
  export); a two-hop chain (positive, with the chain recorded); a read whose owner
  node is ambiguous (negative).
* `fixtures/package-contracts/dependency-invoke-composition` — a dependency whose
  contract carries a closed `callbacks` entry for arg 0 with an `invoke`
  operation (positive) paired with: closed `callbacks` and **no** entry for arg 0
  (negative); open `callbacks` (negative — the live rootless shape); an entry for
  `arg: 1` against a demand about `arg: 0` (negative); a refused/absent receipt
  (negative); a `construct` site (negative); a `min >= 1` demand against a
  `min: 0` claim (negative).

A `dialect-solid-1x` / `dialect-solid-2` pair is needed for slice 1 only, and
only where the dialect answers differ — `unambiguous_reactive_result_slot`
currently agrees on `createSignal` and `createMemo` across both
(`solid_1x.rs:407-414`, `solid_2.rs:476-483`), so the pair should pin a primitive
where they *don't*, or record that none of the rows in scope differ.

### E.6 Controls that must stay put

Measured on the reverted `79e71286` build (`controls.json`), all `class: success`:

| row | status |
|---|---|
| `@solid-primitives/marker@0.2.2\|solid1\|only` | **certified** |
| `@solid-primitives/marker@2.0.0-next.2\|solid2\|floor` | **certified** |
| `@solid-primitives/marker@2.0.0-next.2\|solid2\|head` | **certified** |
| `@solid-primitives/jsx-parser@0.2.0\|solid1\|only` | **certified** |
| `@solid-primitives/scheduled@1.5.3\|solid1\|only` | **certified** |
| `@solid-primitives/i18n@2.2.1\|solid1\|only` | **certified** |
| `@solid-primitives/rootless@1.5.4\|solid1\|only` | **certified** (vacuously — §C.3) |
| `@solid-primitives/timer@1.4.5-next.1\|solid2\|floor` | refused, `sha256:1aee58a9…` |
| `@solid-primitives/timer@1.4.5-next.1\|solid2\|head` | refused, same digest |
| `@solid-primitives/until@0.1.1\|solid1\|only` | refused, `sha256:15fde3fc…` |

`jsx-parser` is the load-bearing one for slice 1: it is the only consumer of
`require_return_callable_source` (`:4542`), which shares
`returnValueSourcesLocked` with the new `calleeSources`. `marker` ×3 are the
load-bearing ones for anything touching `argument_sources` /
`require_reactive_operation_input`. `rootless` must keep certifying **and keep
publishing `{"call":{}}`** — if slice 3's prerequisite changes that, it is a
deliberate semantic movement and needs its own before/after record, not a
silent one.

Also required before handoff: the `solid-js@1.9.14|solid1|only` row must stay
refused on
`sha256:5463f0ed0af9a202b45f80b731fdba6d9048d2898f3ab5847973ec4156370f7c`
(`recursive-value-shape`, `ErrorBoundary:read-0`) — mechanism B would move that
class for 123 read demands across 18 exports, and per the 2026-09-02 backlog it
additionally needs the **self-artifact premise** (`createSignal` is declared
locally in `dist/solid.js`, so `target_module` is empty and
`exports_value_from` answers `false`) *and* the `argument-binding` blocker at
`createReaction`. Do not let slice 1 assume it moves.

### E.7 Honest yield estimate, with the first-blocker caveat

| slice | rows certified | first-blocker caveat |
|---|---|---|
| E.1 mechanism B | **0** | clears `createPolled:read-0` only; timer still refuses on `createIntervalCounter`. Also the *first* blocker for solid-js's 123 read demands, which then hit the self-artifact premise and `createReaction` |
| E.2 composition | **0 alone / 2 with E.1** | measured directly (§D.1): skipping only `createIntervalCounter` lands on `createPolled` |
| E.3 dependency invoke | **1 (until), gated** | the dependency claim does not exist; the prerequisite is chain-depth-2 acceptance of `@solid-primitives/utils` into rootless |

**Total confidently reachable: 3 of the 418 rows** — timer ×2 behind two slices
that must land together, until ×1 behind a prerequisite outside this work. Cost
ordering is the reverse of yield: E.3 is the smallest change (no producer, no
public schema) and the most blocked; E.2 is a public-schema and canonical-digest
change that moves acceptance digests corpus-wide; E.1 is a Type Facts protocol
bump and a full `make verify`.

**Unmeasured, and not claimed**: how many of the ~50 other exact refusals share
mechanism B (the class is emitted for every non-parameter `read` input, and
solid-js alone plans 123 of them) or share
`argument_slot_is_proven_invoking`'s missing dependency tier. The scoping study
left the same split unmeasured; this round did not close it either.

### E.8 Remaining fail-closed / uncertifiable after all three slices

* Every `read` whose callee traces to nothing — a computed callee, a
  reassigned binding, a plain `const c = createMemo(fn)` (the identifier arm hops
  only through an array binding element), `(options.storage || createSignal)(…)`.
* `createResource`, `useTransition`, `createDeferred`, `createSelector`,
  `createOptimistic` and the store family carry **no**
  `reactive_result_slot` row at all, in either dialect.
* The **self-artifact premise** is still unimplemented, so solid-js certifying
  itself stays refused regardless.
* `target_module` remains the *written import specifier*, not a resolved package
  identity — the approximation `require_return_callable_source` and
  `traced_source_proves_role` already make, and one a `@solidjs/signals` import
  would expose.
* `inferred_contract.rs:344-368` still **stamps** `at: call / schedule:
  same-stack` on every inferred operation rather than deriving it. Composition
  inherits that; the 2026-09-02 parameter-read diagnosis §4.A owns the repair, and
  the `!captured` veto stays the only enforcement until it lands.
* The accepted-catalog **freshness hole**: nothing re-hashes the installed tree at
  load time, and `PackageContractIssueKind::{Stale, StaleBundled,
  IntegrityMismatch}` remain unreachable.
* `count.min >= 1` operations remain undischargeable by any tier of
  `argument_slot_is_proven_invoking` — by construction, and correctly.
* rootless (and any package with an unaccepted external dependency) keeps
  certifying **vacuously**. That is sound but it is not information, and a gate
  that counted it as a win would be measuring the wrong thing.
