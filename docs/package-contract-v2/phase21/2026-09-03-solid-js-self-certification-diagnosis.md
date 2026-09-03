# What it would take to certify `solid-js@1.9.14|solid1|only` — and why it must not be certified as generated

Read-only diagnosis, worktree `codex/phase19a-authenticated-proof-policy` at
`dc2ac4d5`. Every measurement made with a debug checker built by
`make build-checker-debug` (a bare `cargo build` produces a binary that refuses
with *"verifier build has no configured Type Facts executable digest"*).
`bin/solid-typefacts` reused as found — stamp already at source
`7de2f0157f9899bb15bc2f8f1e6532d8958512e2afb7899a17f7c4cc6251ed2b` (build id
`dev`).

**All temporary diagnostic patches have been reverted** and the debug checker
rebuilt from the reverted source. `git diff --stat` is empty; `git status
--short` shows only the pre-existing untracked
`packages/cli/solid-reactivity.json.refusals.json`. No
`benchmarks/ecosystem/report-probes-*.md` was created.

Scratch: `<scratch>/solidjs/` — `base.json`/`base.md` (unpatched reproduction),
`census2.log` (full demand-refusal census, no premises), `censusS.log`
(self-artifact identity only), `censusSC.log` (self-artifact + captured),
`skip1..6.json` (ceiling iterations), `web.json.refusals.json` (the `./web`
experiment of §4), `controls.json` (must-not-clear controls at the reverted
build), `type_facts.rs.orig` (the pre-patch file used to revert).

---

## 0. Reproduction and the headline

`base.json` reproduces the committed `benchmarks/ecosystem/report.json` row
**exactly**: `class: partial-success`, `refusedArtifactCases: 38`,
`inapplicableArtifactCases: 17`, and certification refused at
`stage: witness-acquisition`, `owner: certifier`, family
`recursive-value-shape`, demand
`sha256:5463f0ed0af9a202b45f80b731fdba6d9048d2898f3ab5847973ec4156370f7c`, byte-identical
reason.

**The answer, stated first, because it changes what the question is worth
asking.** The audited bundled contract for the *exact bytes under
certification* already exists in this repository, and it asserts — **closed** —
that the demands the row refuses on **do not exist**.

`pkg/contracts/bundled/solid-v1/solid-root-browser-production.json` names
`solid-js@1.9.14`, `package.integrity`
`sha512-sAEXC0Kk0S1EDg+8ysEWJDbYhA3RRoEjwuySUGlKIemeo0I5YZfOyumNjNs9Sv3y2nmhD+0rW66ag2HsMuQiGQ==`,
`package.manifest.sha256`
`52ee61ea826e59a5ec36f6ce91bafb7d38bf99ab04b3a6217993fb17af6037c0`, runtime
`./dist/solid.js` `sha256:b0525eede2c4209fb444831f3bb54f5fdb1a8d748aa7d597cd392880c761c11d`,
declarations `./types/index.d.ts`
`sha256:6e70da1429948b2ca8201ccc248bd2cb77cd6bbc4cf33f639c4b0f8f6a35cbd1`. Every
one of those is byte-equal to the probe's own artifact case `.` (caseIndex 0/1)
and to `dependencyPlan.rootIdentity.integrity`. Content match, not coordinate
match.

For those bytes the audit says, across **all 12 summaries covering all 54
exports**:

- `reads: []` with `reads` in `closed` — every summary, no exception;
- `creates: []` with `creates` in `closed` — every summary, no exception;
- **no `owner` field anywhere in the document**;
- only `invoke` (callback) and `return` operations exist.

The generated proposal for the same bytes invents 158 `read` operations and 22
`owner-requirement` create operations. **413 of the 614 refused demands (67%)
assert something the audited authority for those exact bytes closes as
absent.** This is ADR 0005's disqualifying finding for `@solidjs/signals`
`onSettled`, reproduced at 413 demands instead of 1 — and this time the audited
authority is checked in, digest-matched, and unambiguous.

So: **`solid-js@1.9.14` cannot certify, and the self-artifact identity premise
must not be the reason it does.** The defect is on the generator side, exactly
where ADR 0005 pointed.

---

## 1. Inventory of the walls

### 1.a The 38 generation refusals, classified

Read from the kept `--keep-temp` install's own
`solid-js@1.9.14--solid1--only.json.refusals.json` (the authority), not from
`report.json`. Install: `<T>/solid-checker-ecosystem-wzrjnM`; output:
`<T>/solid-checker-ecosystem-out-YptWgD`.

| # | class | refusals | applicability | entrypoints |
| --- | --- | --- | --- | --- |
| A | `no declaration target exists for <root>/…​.cjs` | **13** | `unsupported-artifact-shape` | `./dist/{dev,server,solid}.cjs`, `./h/dist/h.cjs`, `./html/dist/html.cjs`, `./store/dist/{dev,server,store}.cjs`, `./universal/dist/{dev,universal}.cjs`, `./web/dist/{dev,server,web}.cjs` |
| B | `accepted dependency solid-js has no exact runtime binding for export ErrorBoundary` | **11** | `runtime-module` | `./web` ×7 condition sets (`[]`, `[development]`, `[browser]`, `[browser,development]`, `[node]`, `[deno]`, `[worker]`), `./web/dist/{dev,server,web}.js`, `./web/types/index.d.ts` |
| C | `emit package contract: entry file <root>/dist/solid.js has no runtime ESM exports` | **2** | `runtime-module` | `./jsx-runtime`, `./jsx-dev-runtime` |
| D | `local runtime module ./x.js from <ep> resolves only to declaration file …; the package ships no runtime module under that specifier` | **11** | `unavailable-published-target` | `./types/index.d.ts`, `./types/reactive/{array,observable,signal}.d.ts`, `./types/render/{flow,hydration}.d.ts`, `./types/server/rendering.d.ts`, `./store/types/{index,mutable}.d.ts`, `./web/types/{client,server}.d.ts` |
| E | `contract emission batch target 45 names entry file <root>/types/jsx.d.ts as its own fact source; its suffix makes it a TypeScript declaration file, for which the Type Facts producer reports no source facts` | **1** | `runtime-module` | `./types/jsx.d.ts` |

Total 38. Plus 17 recorded **inapplicable**: 16 `non-emitting-module-target` /
`verifier-proved-type-only` (all `.d.ts` runtime targets) and 1
`non-module-target` (`./package.json`).

**No `Aliases`-class refusal appears in this row.** The brief's slice-2 premise
that `Aliases` is one of the 38 is not what the row records: the `Aliases` wall
is *masked* behind B, and I measured it directly (§4.b).

Classes D and E are both `.d.ts` **entrypoints**, i.e. the same population as
the 16 already recorded inapplicable — see §6, slice 1: the split between the
12 that refuse and the 16 that are inapplicable is decided by one mechanical
predicate, and it is arguably the wrong one.

Corpus-wide context (from the committed `report.json`, 356 artifact-case
refusals across 418 probes):

| class | corpus refusals | rows |
| --- | --- | --- |
| A `no declaration target exists` | 15 | 3 |
| B **self-package** cross-entrypoint binding | **11** | **1 — `solid-js@1.9.14` only** |
| B′ *external*-dependency binding (same message, different subject) | 102 | 34 |
| C `no runtime ESM exports` | 64 | 14 |
| D `resolves only to declaration file` | 19 | 3 |
| E `.d.ts` as its own fact source | 38 | 4 |
| `whose runtime kind no closed type answers` (`Aliases` class) | 3 | 3 |

The self-package variant of B was separated from the external variant by
comparing the refusal's named specifier against the row's own package name;
**solid-js@1.9.14 is the only row in the corpus that has it.**

### 1.b The certification demands and their refusals

`certificationAttempt.demandCountsByFamily` (identical in the committed report
and the reproduction), for the 31 artifact cases that generate:

| family | demands | of which artifact-satisfied |
| --- | --- | --- |
| `recursive-value-shape` | 936 | — |
| `callable-path` | 749 | — |
| `operation-reachability` | 475 | — |
| `operation-cardinality` | 475 | — |
| `rest-spread-coverage` | 297 | — |
| `selected-signature` | 297 | — |
| `argument-binding` | 183 | — |
| `artifact-declarations` / `export-identity` / `export-resolution` / `manifest-entrypoint` / `module-closure` / `package-identity` | 31 each = 186 | all 186 |

Type-Facts-owned demands: **3412**. The proposal's 475 `positiveOperations`
decompose as **183 `callback`, 158 `read`, 112 `return`, 22
`owner-requirement`** — the 183 callbacks match `argument-binding` 183 exactly,
and each positive operation carries one `operation-reachability` and one
`operation-cardinality` demand.

The unpatched certifier stops at the first refusal, so the committed report
shows one. A temporary census patch (log-and-continue at
`verify_export_value_subject` and `verify_export_value_family`,
`type_facts.rs:2560-2561`) produced the **complete** refusal inventory in one
run — `<scratch>/solidjs/census2.log`:

**614 of 3412 Type-Facts demands refuse (18%)**, in ten classes:

| class | demands | families | exports involved |
| --- | --- | --- | --- |
| read witness — *"read operation … has no reachable uncaptured call whose callee traces to an unambiguous dialect reactive/accessor"* | **369** | `recursive-value-shape` 123, `operation-reachability` 123, `operation-cardinality` 123 | 19: `ErrorBoundary`, `For`, `Index`, `Show`, `Suspense`, `Switch`, `catchError`, `children`, `createComponent`, `createComputed`, `createDeferred`, `createEffect`, `createMemo`, `createReaction`, `createRenderEffect`, `createResource`, `createSelector`, `mergeProps`, `onMount` |
| argument flow — *"callback parameter has no exact direct-call or resolved-argument flow"* | **110** | `argument-binding` 30, `operation-reachability` 30, `operation-cardinality` 30, `callable-path` 20 | `mapArray`, `indexArray`, `createReaction`, `onMount` |
| owner requirement — *"owner requirement has no exact dialect primitive call; observed [...]"* | **44** | `operation-reachability` 22, `operation-cardinality` 22 | `createResource` 12, `from` 20, `onMount` 12 |
| dialect callback flow — *"callback parameter has neither an exact direct call nor an exact dialect callback flow"* | **22** | `callable-path` | `reconcile` 6, `unwrap` 6, `mapArray` 4, `createReaction` 2, `onMount` 2, `indexArray` 2 |
| root shape — *"operation value root shape has no verifiable premise: the demand asserts no callability and the producer's root observation is open"* | **22** | `recursive-value-shape` | `onCleanup` 10, `createMutable` 4, `unwrap` 4, `Match` 3, `createDeferred` 1 |
| invoke input — *"reactive operation input N has no call of the exact callback parameter the implementation may reach"* | **12** | `recursive-value-shape` (input 0 ×6, input 1 ×6) | `mapArray` 6, `indexArray` 6 |
| value path locally open (`complete=true, presence=Absent`) | **13** | `recursive-value-shape` | `from` 7, `indexArray` 3, `mapArray` 3 |
| path absent from signature census | **11** | `recursive-value-shape` | `createResource` 3, `from` 3, `useTransition` 2, `indexArray` 1, `mapArray` 1, `updatePath` 1 |
| subject mismatch — *"resolved value declaration name disagrees with snapshot export replay"* | **8** | 7 families | `createRenderEffect` in artifact case `5c1d4df73349…` (`./dist/server.js`), where the export is `const createRenderEffect = createComputed;` (`dist/server.js:77`) — an intra-module alias whose resolved declaration name is `createComputed` |
| recursive input parameter absent | **3** | `recursive-value-shape` | `createResource` |

### 1.c The first refusal, predicate by predicate — and the brief's premise is wrong about it

Demand `sha256:5463f0ed…` is `recursive-value-shape` on
`artifact-case:331dfa4929983278c23d6479ad3858531982d1317cd6b3745f89c7ac26272ade`
= entrypoint `.`, runtime `./dist/solid.js`, export `ErrorBoundary`, operation
`…:ErrorBoundary:operation:read-0`.

Route: `require_operation_recursive_subject`
(`type_facts.rs:5134`) → the `ValueRoot::OperationInput` branch → `path`
empty and `callable == Unknown`, `composed_from` absent →
`reactive_read_operation_input_role` answers `Accessor` →
`require_reactive_read_operation_input` (`type_facts.rs:4418`).

The backlog entry *"A `read` operation's locally created accessor now has a
census witness (2026-09-03)"* states that solid-js's read demands are *"reachable
by this arm in shape and blocked by module identity: `createSignal` is declared
locally in `dist/solid.js`, so `target_module` is empty and `exports_value_from`
correctly answers false."* **That is not the predicate that fails.** A temporary
dump of the whole call census at the refusal (13 calls) shows:

```
45927 expr=true reach=Reachable captured=false  name=            calleeSources=[]
45945 expr=true reach=Reachable captured=false  name=getContextId calleeSources=[]
46007 expr=true reach=Reachable captured=false  name=createSignal calleeSources=[]
46059 expr=false …                              name=Set         calleeSources=[]
46073 expr=true reach=Reachable captured=false  name=            calleeSources=[]
46099 expr=true reach=Reachable captured=false  name=onCleanup    calleeSources=[]
46115 expr=true reach=Reachable captured=true   name=            calleeSources=[]
46152 expr=true reach=Reachable captured=false  name=createMemo   calleeSources=[]
46194 expr=true reach=Reachable captured=TRUE   name=errored      calleeSources=[{kind=CallResult,path=0,target=symbol:h:0515bc2e…,module=,name=createSignal,targetPath=1}]
46290 expr=true reach=Reachable captured=true   name=untrack      calleeSources=[]
46304 expr=true reach=Reachable captured=true   name=f            calleeSources=[]
46315 expr=true reach=Reachable captured=TRUE   name=setErrored   calleeSources=[{kind=CallResult,path=0,target=symbol:h:0515bc2e…,module=,name=createSignal,targetPath=1}]
46352 expr=true reach=Reachable captured=true   name=catchError   calleeSources=[]
```

The only call whose callee traces to a `createSignal` slot is `errored()` at
byte 46194, and it is `captured = true`. The published bytes make that
structural:

```js
const [errored, setErrored] = createSignal(err, undefined);
…
return createMemo(() => {
  let e;
  if (e = errored()) { … }
  return catchError(() => props.children, setErrored);
}, undefined, undefined);
```

`errored()` sits inside the `createMemo` callback. So **two independent walls
are stacked on this one demand**, and the *first* one is not module identity:

1. **`type_facts.rs:4433`** — `if !is_call_expression(call) ||
   !floor.admits(call.reach) || call.captured { continue; }`. The `captured`
   veto discards byte 46194 before any dialect table is consulted. It is
   load-bearing by design: `ContractReactiveRead` has no schedule column, so
   the uncaptured gate is the only enforcement of the row's `at: call /
   schedule: same-stack` stamp anywhere.
2. **`type_facts.rs:4874`** (in `traced_source_proves_role`, fn at `:4868`) —
   `solid_dialect::exports_value_from(&source.target_module,
   &source.target_name)` with `target_module == ""`, refused at
   `solid-dialect/src/lib.rs:370-372` (`if origin_module.is_empty() ||
   name.is_empty() { return false; }`).

**Measured consequence.** The self-artifact identity premise alone discharges
**zero** of the 369 read-witness demands, because the census never reaches the
module test. See §5.

The other three identity gates in this subsystem, for completeness (all
`target_module == "solid-js"` literals, all in `type_facts.rs`):

| site | function | family it gates |
| --- | --- | --- |
| `:3532` | `require_parameter_callback_flow` (fn `:3469`, refusal `:3558`) | `callable-path` |
| `:3649` | `callee_pending_invocation_holds` (fn `:3639`, `premise.module`) | reached from `argument_slot_is_proven_invoking` |
| `:3836` | `argument_slot_is_proven_invoking` Tier A (fn `:3834`) | `argument-binding`, and the `implementation_call_is_executed` chain at `:3669`/`:3685` |
| `:4165` | `require_owner_operation_call` (fn `:4141`) | owner requirements |
| `:5397` | the return-source arm reaching `unambiguous_callable_result_tuple_item` | `callable-path` |

---

## 2. The self-artifact identity premise, precisely

### 2.a The sound form

> When the artifact case under certification belongs to the **audited dialect
> package** — matching name, exact version **and** `package.integrity` against
> the audited tuple, all three read from the authenticated artifact snapshot
> (`SnapshotedPackage::package_name`/`package_version`/`package_integrity`,
> `contract_certification.rs:1478`/`:1483`/`:1487-1490`), never from a
> filesystem path, an import specifier, or a producer-reported module name —
> and a callee resolves **by declaration identity within this same
> authenticated archive** to an export `E` of this artifact case (the export
> census's declaration binding, not a name match), then that callee is treated
> as dialect module `solid-js`' export `E` for the module-identity gates at
> `type_facts.rs:3532`, `:3649`, `:3836`, `:4165`, `:4874`, `:5397`, with a
> witness naming the premise rather than an import edge.

The audited 1.x tuple is pinned in three checked-in places and they agree:

- `pkg/contracts/bundled/runtime-lock.json` — `solid-js` version `1.9.14`,
  integrity `sha512-sAEXC0Kk0S1EDg+8ysEWJDbYhA3RRoEjwuySUGlKIemeo0I5YZfOyumNjNs9Sv3y2nmhD+0rW66ag2HsMuQiGQ==`
  (as the audited peer of `@solid-primitives/{rootless,utils,debounce,scheduled}`);
- `pkg/contracts/bundled/solid-v1/*.json` (19 documents) — same integrity plus
  `package.manifest.sha256 52ee61ea…` and per-case artifact digests;
- `scripts/lib/audited-solid-runtime.mjs:7` — `const VERSION = "1.9.14"`, the
  audited runtime the contract tests and the tsc oracle install.

**Two of ADR 0005's five preconditions are already satisfied here, and one is
strictly better than for `@solidjs/signals`:**

- *Precondition 1 (integrity-bound tuple)* — reachable, same as for signals;
  the accessor and the field-naming comparison loop
  (`contract_certification/dependencies.rs:1222-1240`) already exist.
- *Precondition 2 (version keyed to the actually-audited bytes)* — **already
  correct for 1.x, unlike v2.** The v1 dialect tables cite `1.9.14`
  throughout: `rust/crates/solid-dialect/src/solid_1x.rs:4` (*"extracted from
  the published `solid-js@1.9.14` package"*), `:185`, `:236`, `:287`, `:328`,
  `:391`, `:416`, `:609` (*"Read from `solid-js@1.9.14` `dist/solid.js`, the
  artifact the oracle …"* — the very artifact under certification), `:1103`,
  and `exports/solid_v1_solid_js.rs:4` (*"Audited against the exact
  `solid-js@1.9.14` published declaration surface"*). No rc.0/rc.3 skew of the
  kind that made objection 2 disqualifying for signals.
- *Precondition 3 (`floor == MayExecute` only)* — reachable; `floor` is a
  parameter of every gated function.
- *Precondition 4 (a corpus fixture exercising `from_plan`)* — reachable;
  `scripts/contract-corpus.mjs:60` fabricates `fixture:sha256:<manifest
  digest>`, so a fixture named `solid-js@1.9.14` is refused by an
  integrity-bound gate and admitted by a name+version one, pinning both
  directions.

### 2.b Precondition 5, family by family: which demands are ill-formed for the defining package

This is where the premise dies, and it dies **for a subset of families only** —
which is more informative than "it's circular".

**Owner requirements (44 demands) — ill-formed. Excluded outright.** The
derivation is exactly the chain ADR 0005 traced for `onSettled`, with
`solid-js`' own names in it:

1. `solid_primitive_declaration` (`solid-reactive-ir/src/symbols.rs:589-595`)
   grants primitive identity to any declaration whose **filesystem path**
   carries a `solid-js` or `@solidjs` component
   (`declaration_path_is_solid_package`, `:597-601` — deliberately, *"Bootstrap
   analysis of Solid's own implementation, where there is no package import to
   establish provenance"*) and whose **name** the dialect declares. For
   `dist/solid.js` that is every local `createEffect`, `createRenderEffect`,
   `onCleanup`, `createMemo`, …
2. `find_missing_owners` (`solid-reactive-ir/src/owners.rs:845-870`) pushes an
   owner requirement at the `Primitive::CreateEffect | CreateRenderEffect |
   CreateTrackedEffect` arm and at `Primitive::OnCleanup`.
3. `apply_owner_requirement`
   (`solid-facts-backend/src/inferred_contract.rs:445-457`) sets
   `owner.requirements.owner = Required`, `source = AmbientAtCall`,
   `productions = Complete([])`, and maps `Effect`/`Boundary` →
   `child_owners = Required`, `Cleanup`/`SettledCleanup` → `cleanup =
   Required`.
4. `require_owner_operation_call` (`type_facts.rs:4141`, gate `:4165`) then
   demands a reachable uncaptured call with `target_module == "solid-js"` whose
   name `unambiguous_owner_requirement_role` maps to that role.

The demand's only evidence is *dialect name vocabulary plus a path heuristic*;
the axiom would answer it from `solid_1x.rs`' rows about the same names. There
is no artifact evidence in the loop the axiom would not itself be supplying.
The measurement confirms it mechanically: with the premise granted, the witness
the axiom supplies for the operation reached before the next wall is the string
`onCleanup` — the same name that created the demand.

And the audit contradicts the demand's existence: `creates: []` **closed** in
every summary, and **no `owner` field in the document at all**. The three
exports carrying owner requirements are `from` (20 demands — `onCleanup(clean)`
at `dist/solid.js:1101`/`:1104`), `onMount` (12 — `createEffect(() =>
untrack(fn))`) and `createResource` (12 — `onCleanup` at `:286`/`:444`). All
three are *the primitive's own guarded implementation*, not a consumer
obligation.

**Read operations (369 demands) — also ill-formed, on the same ground.** The
audit's `reads: []` is **closed** in every summary. `ErrorBoundary`'s audited
summary is exactly `{shape: callable, call: {callbacks: [], reads: [], creates:
[], returns: [], closed: [callbacks, reads, creates, returns]}}`. The generated
proposal's `ErrorBoundary:read-0` is a claim the audit denies for the same
bytes. Granting the premise here would prove a false claim, not a true one that
happened to lack a witness.

**Argument/callback flow (110 + 22 demands) — well-formed.** These correspond
to claims the audit *also* makes: `createReaction`'s audited summary carries
`callbacks: [{from: {arg: 0}, operation: callback-0}]` with a `queued`,
`untracked`, `min: 0` invoke operation; `mapArray`'s carries two. So an
argument-binding demand on `createReaction` slot 0 is asking about something
the audit agrees exists. **This is the one family where the premise both
applies and asks a real question** — and it is where it moves rows (§5).

**`selected-signature` / `rest-spread-coverage` / `callable-path` about
signatures — unaffected.** No module-identity gate; they already pass except
for the 8 `createRenderEffect` alias demands.

### 2.c Internal non-exported helpers: how much cannot be proved by any table

Measured, not sampled — the full census of the 110 argument-flow refusals with
the matching calls dumped:

| matching call in the census | events | export | why `implementation_call_is_executed` says false |
| --- | --- | --- | --- |
| `untrack(onInvalidate)` | 44 → 22 with the premise | `createReaction`, `onMount` | `createReaction`: the call sits inside `createComputation(() => { fn ? fn() : untrack(onInvalidate); … }, undefined, false, 0)`. **`createComputation` is a module-local, non-exported helper** — `unambiguous_callback_argument("createComputation", 0, 4)` is `None` for every dialect, so no table can prove slot 0 invoking, with or without any identity premise. `onMount`'s `createEffect(() => untrack(fn))` *is* dialect vocabulary and clears with the premise. |
| `mapFn(newItems[j], indexes[j])` | 66 (44 + 22) | `mapArray`, `indexArray` | The call is inside `mapper`, inside `untrack(() => {…})`, inside **the arrow `mapArray` returns**. The outermost link of the chain is a *return value*, not a call the export's body reaches. No identity premise can help: the invocation happens only when the consumer calls the returned accessor. |

Every one of the 154 matching-call entries across those 110 refusals had
`executed=false`; not one failed on source matching. So the class is uniformly
"the enclosing-callable chain does not terminate in a call this export's body
makes".

**Estimate for the brief's question:** of the 132 argument/callback-flow
demands (110 + 22), **22 clear with the premise** (all `onMount`), and the
residual **110 terminate in one of the two shapes above** — 44+2 in
`createComputation`, a non-exported helper, and 66+4+2 in a returned closure.
Neither is reachable by any table.

---

## 3. Cross-entrypoint binding (class B, 11 refusals)

### 3.a What happens today

`solid-js/web`'s runtime module `web/dist/web.js:2` is literally

```js
export { ErrorBoundary, For, Index, Match, Show, Suspense, SuspenseList, Switch,
  createComponent, createRenderEffect as effect, getOwner, mergeProps, untrack } from 'solid-js';
```

and its declaration module `web/types/index.d.ts:4` mirrors it. The generator's
`bindExport` (`packages/cli/scripts/artifact-resolution.mjs:2019`) reaches
`description.externalDirect`, calls `acceptedExternalBinding` (`:1999`), gets
`undefined` because no accepted contract for `solid-js` is in scope, and
refuses at `:2058-2063`:

```js
fail("accepted-dependency-binding",
  `accepted dependency ${externalDirect.specifier} has no exact ${axis} binding for export ${externalDirect.name}`);
```

`ErrorBoundary` is simply the alphabetically first of the 13 names. The
`externalEdges` census already records that this edge resolves to
`solid-js@1.9.14` — the row's own archive: `[{"specifier": "solid-js",
"package": "solid-js", "entrypoint": ".", "resolvedVersion": "1.9.14"}]`. The
`dependencyPlan` is `status: exact-leaf-refusal`, `complete: true`, with
exactly these 11 cases as its roots.

### 3.b The sound intra-archive binding

The precedent is `external_binding` /
`external_dependency`
(`rust/crates/solid-facts-backend/src/contract_certification/export_bindings.rs:945`
and `:924`): select the one planned dependency this package's own import of
the specifier resolved to, then read `dependency.verified_exports.bindings[name]`
per axis. The intra-archive form is the same shape with the archive itself as
the "dependency":

> When a module's external re-export specifier resolves — through the same
> authenticated snapshot's own `exports` map, at the same conditions — to a
> **sibling entrypoint of the archive under certification**, bind the name from
> that sibling artifact case's own export census by declaration identity
> (`verified_exports.bindings[name]`, per axis), and record the sibling case id
> in the binding target's provenance instead of a dependency plan node.

Where it goes: a third arm in `ArtifactSnapshot`'s binding resolver beside
`direct` / `externalDirect`, tried **before** `external_binding` and only when
the specifier's package name equals `snapshot.package_name()` **and** the
subpath resolves inside the same snapshot. Because the archive is one
authenticated snapshot with one integrity, no new trust surface is introduced —
the bytes are already replayed. The JS generation side needs the mirror in
`bindExport` so the two lanes agree.

What must stay fail-closed:

- a subpath the archive's own `exports` map does not export (or exports only
  behind unmatched conditions) — no binding, refuse as today;
- a name the sibling artifact case does not export — refuse (this is the
  `acceptedExternalBinding` `undefined` path, unchanged);
- the sibling's *condition set* must be the one the importing case resolves
  under (`./web` `[browser,development]` must bind against `.`
  `/exports/./browser/development/import`, not against `/exports/./import`) —
  otherwise a dev-only export shape would answer a production case;
- a `.d.ts`-only sibling for a runtime axis — refuse (class D's rule, unchanged);
- a cycle (`A` → `B` → `A`) — the existing `visiting` set at
  `artifact-resolution.mjs:2027` already refuses `export-cycle`; the new arm
  must join it.

### 3.c Measured yield: **zero**

See §4.b. With the self-package edges removed the `./web` cases refuse
immediately on `Aliases`. Slice B alone buys nothing.

---

## 4. `Aliases`-class runtime kinds

### 4.a The mechanism

`promote_entry_callable` (`main.rs:6410`) asks
`solid_reactive_ir::export_kind_proof_from_entity`
(`solid-reactive-ir/src/contracts.rs:2194`), which reads **only the runtime
entity**: its `callability`, `constructability`, and `runtime_binding_kind`
write census. `Callability::Unknown` (an `any`/`unknown`/`never`/error type) on
both axes yields `ExportKindProof::Unresolvable`, and
`reconcile_entry_export_kind` (`main.rs:6435`) refuses at `:6468-6471`:

```
whose runtime kind no closed type answers ({callability:?}, {constructability:?})
```

Verified with the CLI's own TypeScript 5.9.3 against the installed artifact:
`solid-js/web/dist/web.js`'s `Aliases` is `any` — 0 call signatures, 0
construct signatures — because `Object.assign(Object.create(null), {…})` is
`any & {…}` → `any` (`Object.create(o: object | null): any` in `lib.es5.d.ts`).
The declaration axis for the same export says
`export const Aliases: Record<string, string>;`
(`web/types/client.d.ts:2`, `web/types/server.d.ts:2`), reached from the
artifact case's declarations `web/types/index.d.ts` via `export * from
"./client.js"` — i.e. `NonCallable ∧ NonConstructable`, which publishes
`kind: "value"`.

### 4.b Measured: `Aliases` is the wall immediately behind class B

I copied the kept install, deleted the 7 self-package re-export lines
(`web/dist/{web,dev,server}.js` and `web/types/index.d.ts`) — nothing else —
and ran `generatePackageContract` on `./web` alone. Result
(`<scratch>/solidjs/web.json.refusals.json`): **all 7 `./web` condition cases
refuse on `Aliases`**, `(Unknown, Unknown)`, naming `web/dist/web.js`,
`web/dist/dev.js` and `web/dist/server.js` respectively. `Aliases` is a local
`const` in each bundle, so the deleted lines cannot have caused it.

So B and the declaration-axis kind premise are **jointly** required for `./web`
to generate at all, and neither is sufficient alone.

### 4.c The proposed rule and its soundness

> `ExportKindProof::Unresolvable(Unknown, Unknown)` — *both* axes `Unknown`, i.e.
> the runtime type is `any`/`unknown`/`never`/error — may be answered by the
> **declaration** axis' closed proof for the *same export of the same artifact
> case*, when that declaration binding is the authenticated one the case
> already carries (`verified_exports.declaration_binding(export)`), and the
> declaration's own proof is closed (`Callable` or `NonCallable`). `Mixed` on
> either axis stays refused; an absent fact (`Unanswered`) stays refused.

**Is it sound?** The brief's objection — an `any`-typed runtime value declared
as a non-callable record could still be a function at runtime — is real but
answered by the precision contract's own "different claim" test, in the
direction that matters:

- The declaration file is **the package's own published claim about these exact
  bytes**, authenticated in the same archive with the same integrity. It is not
  an inference; it is the artifact's other half. Consuming it is what every
  other axis of this subsystem already does.
- The rule is **symmetrical with one that already ships**, in the opposite
  direction: `export_kind_proof_from_entity` already lets a *closed runtime
  write census* override a **declaration-surface negative**
  (`contracts.rs:2219-2226`, *"Published packages commonly ship `index.js`
  beside `index.d.ts`; the configured compiler may attach the declaration
  module's non-callable answer to the runtime declarator…"*). The runtime side
  is preferred **when it is closed**. When it is `Unknown` on both axes it is
  not closed and has nothing to prefer.
- The dangerous direction is publishing `kind: "value"` for something that *is*
  a function — the `@solid-devtools/locator` `addClickInterceptor(fn)` case the
  doc comment at `main.rs:6396-6400` records. The rule does not do that: if the
  declaration says `Callable` it raises to `function` with `callbacks` unknown
  (`raised_function_export`), which is the conservative answer. It only
  publishes `value` when the package's own declarations say non-callable.
- The residual risk is a package whose `.d.ts` lies about its `.js` — a
  mis-declared export. That is a *different claim* than the one the checker
  makes (the checker would then be reporting the package's own stated contract,
  attributable to the package), and it is the same trust the declarations axis
  already receives everywhere else in the case set.

**Corpus reach, measured.** `grep`ing the committed `report.json` for *"whose
runtime kind no closed type answers"*: **3 refusals, 3 rows** —
`@solid-devtools/debugger@0.28.1|solid1|only` (`dist/types.js`,
`DebuggerModule`), `@solid-devtools/locator@0.16.7|solid1|only`
(`dist/index.js`, `addClickInterceptor`),
`solid-devtools@0.34.5|solid1|only` (`dist/vite.js`, `DevtoolsModule`) — all
`(Unknown, Unknown)`. Plus the **11 latent** solid-js `./web` cases measured in
§4.b, which the class-B refusal masks. Whether the three devtools rows' own
declaration axes are closed is **unmeasured** (their installs were not kept);
`addClickInterceptor` is the one where the answer is likely `Callable`, i.e. a
raise rather than a `value` publish. Note also that
`whose runtime kind no fact covers at all` (`Unanswered`, `main.rs:6478`) is a
*separate* message and this rule deliberately does not touch it.

---

## 5. Ceiling by diagnostic skip

Five env-gated diagnostic patches, all in
`contract_certification/type_facts.rs`, all reverted:

- `SOLID_DIAG_SELF_ARTIFACT` — `diag_self_artifact_module(module, name, site)`:
  a stand-in for the §2 premise that answers true when the producer's module is
  **empty** (a local declaration of the bundle under certification) and a Solid
  dialect exports that name in value position, wired into all six module gates
  (`:3532`, `:3649`, `:3836`, `:4165`, `:4874`, `:5397`).
- `SOLID_DIAG_READ_CAPTURED` — relaxes only the `captured` veto at `:4433`.
- `SOLID_DIAG_READ_SKIP` — records `diagnostic-skip:read-witness:<op>` and
  returns `Ok` at the read-arm refusal.
- `SOLID_DIAG_ARG_FLOW_SKIP` — the same at `require_parameter_flow`'s refusal
  (`:3463`).
- `SOLID_DIAG_DEMAND_CENSUS` — log-and-continue at both per-demand entry points
  (`verify_export_value_subject`, `verify_export_value_family`), which is what
  produced the complete inventory in §1.b.

### 5.a The four walls, in order

| # | premises granted | first refusal | family | demand | subject |
| --- | --- | --- | --- | --- | --- |
| 0 | none | *read operation … has no reachable uncaptured call whose callee traces to an unambiguous dialect reactive/accessor* | `recursive-value-shape` | `sha256:5463f0ed0af9a202b45f80b731fdba6d9048d2898f3ab5847973ec4156370f7c` | `331dfa49…:ErrorBoundary:operation:read-0` |
| — | **self-identity only** | **unchanged — `sha256:5463f0ed…`** | | | the `captured` veto fires first (§1.c) |
| 1 | self-identity + `captured` | same message | `operation-cardinality` | `sha256:14ce8d5e4846c6dfbd8c76cf777fdabdcc6426582a2f0ed4a128c3f90991ef3c` | `331dfa49…:For:operation:read-0` — census is 2 calls (`createMemo`, `mapArray`), **neither traces anything**: the read the row claims happens inside `mapArray`, not in `For`'s body |
| 2 | + whole read arm skipped | *callback parameter has no exact direct-call or resolved-argument flow* | `argument-binding` | `sha256:6a05e5b6fd79c4a55e16e73b2923a251e2d4f1243da5118bef2a8e54f979993d` | `331dfa49…:createReaction` — `untrack(onInvalidate)` inside `createComputation(…)`, a non-exported helper |
| 3 | + argument flow skipped | *operation value path is locally open (complete=true, presence=Absent, callability=Unknown, reasons=[])* | `recursive-value-shape` | `sha256:0cc49208c257cd58e3d7879b84e31515fa6baa5500623ce438f9118aee860301` | `331dfa49…:from` — a producer observation that the demanded path is **absent**, no identity question at all |

All four walls sit in **one** artifact case (`331dfa49…`, entrypoint `.` →
`dist/solid.js`) out of 31. Iterating the skip is not converging on
certification; it is walking one export at a time through a package whose whole
`reads`/`creates` surface is fabricated.

### 5.b Per-premise yield, measured against the full census

Three census runs, same build, same install:

| refusal class | base | + self-identity | + self-identity + `captured` |
| --- | --- | --- | --- |
| read witness | 369 | **369** | **297** |
| argument flow | 110 | 88 | 88 |
| owner requirement | 44 | **0** | **0** |
| dialect callback flow | 22 | 20 | 20 |
| root shape no premise | 22 | 22 | 22 |
| invoke input no call | 12 | 12 | 12 |
| value path locally open | 13 | 13 | 13 |
| path absent from census | 11 | 11 | 11 |
| subject mismatch | 8 | 8 | 8 |
| recursive input absent | 3 | 3 | 3 |
| **total refused demands** | **614** | **546** | **474** |

- **Self-artifact identity alone: −68 demands (11%).** All 44 owner-requirement
  demands (`from` 20, `createResource` 12, `onMount` 12 — every one of them),
  18 argument-flow demands (all `onMount`), 4 `callable-path` argument-flow and
  2 dialect-callback-flow (also `onMount`). It moves **zero** read demands.
- **`captured` relaxation alone: −0 demands.** Measured separately: with
  `captured` relaxed and the identity premise withheld the census is still
  **614**. The two are *jointly* necessary for the read family and neither is
  sufficient.
- **Both together: −140 (23%).** The extra 72 are the read demands of exactly
  **two** exports, `ErrorBoundary` and `createResource` (24 read operations ×
  3 families). The other 17 read-claiming exports still refuse, because their
  census contains no callee trace at all.
- **474 demands remain refused with both premises granted**, across 10 classes
  and 8 of them untouched by any identity question. Skipping the read *and*
  argument-flow families **whole** still leaves 89 refusals in 7 classes
  (22 root shape, 20 dialect callback flow, 13 value path locally open, 12
  invoke input, 11 path absent, 8 subject mismatch, 3 recursive input absent).

### 5.c Does the row's `class` become `success` with the cross-entrypoint cases bypassed?

**No, and it cannot.** `class` is the generation class, and generation is
`partial-success` iff any artifact case refused. Removing all 11 class-B
refusals leaves **27** in four other classes (A 13, C 2, D 11, E 1). Certification
is queued for this row not because generation succeeded but because
`dependencyPlan.complete === true && roots.length > 0`
(`scripts/ecosystem-benchmark/run.mjs:1178-1181`), and the certified case set
is the 31 that *did* generate — so the class-B refusals do not block
certification at all; they only shrink the published surface. `class: success`
requires all four other classes to clear too (see §6).

### 5.d Revert confirmed

`git diff --stat` empty; `git status --short` shows only
`?? packages/cli/solid-reactivity.json.refusals.json`. Debug checker rebuilt
from the reverted source, and the must-not-clear controls re-measured on it:

| control | result |
| --- | --- |
| `solid-js@2.0.0-rc.3\|solid2\|only` | `partial-success`, **certification `certified`** — unchanged |
| `@solidjs/signals@2.0.0-rc.3\|solid2\|only` | `success`, refused on `sha256:78a165588bd85e84dd94c2598e9271279c741475da2089d5d6e93f3164cbbbca` — unchanged |
| `solid-js@1.9.14\|solid1\|only` | `partial-success`, refused on `sha256:5463f0ed0af9a202b45f80b731fdba6d9048d2898f3ab5847973ec4156370f7c` — unchanged |

---

## 6. Plan

Ordered by yield per unit of blast radius. The framing changes because of §0:
**no slice here should aim to certify solid-js@1.9.14 as generated.** The first
slice is the generator defect; the rest are honest reductions of the refusal
surface that stand on their own.

### Slice 1 — Reconcile the generated proposal with the audited authority (the real defect)

*This is ADR 0005's precondition 5 answered with a much larger and much clearer
case than `onSettled`.* For `solid-js@1.9.14`'s `dist/solid.js` the generator
proposes 158 `read` and 22 `owner-requirement` operations where the audited
bundled contract for byte-identical bytes closes `reads: []` and `creates: []`
in every one of its 12 summaries and carries no `owner` field. The root cause is
named and singular: `declaration_path_is_solid_package`
(`solid-reactive-ir/src/symbols.rs:597-601`) grants dialect primitive identity
by **filesystem path** inside the defining package, and `OwnerRequirement` /
the read projection carry no field distinguishing a path-bootstrap recognition
from an import-edge one.

- **Yield**: 413 of 614 refused demands become non-demands rather than
  refusals; the row's certification attempt stops asserting claims the audit
  denies. Corpus: only rows whose analyzed archive *is* a `solid-js`/`@solidjs`
  package are affected — `solid-js@1.9.14`, `solid-js@2.0.0-rc.3`,
  `@solidjs/signals@2.0.0-rc.3`, `@solidjs/web@2.0.0-rc.3`,
  `@solidjs/element@2.0.0-rc.3`, `@solidjs/router`.
- **Blast radius**: high and *deliberate*. The path heuristic is what lets the
  checker analyze Solid's own repository; suppressing its *contract-generation*
  consequences must not change its *diagnostic* consequences. Threading
  provenance (`path-bootstrap` vs `import-edge`) through
  `solid-reactive-ir` is an owner-model change with its own fixtures, and
  `solid-js@2.0.0-rc.3|solid2|only` is **certified today** and must stay so.
- **Must not clear**: `solid-js@2.0.0-rc.3|solid2|only` certified; the
  fixtures under `fixtures/reactive-ir/` that rely on path-bootstrap primitive
  recognition unchanged.
- Note the cheaper half is *not* available: the generator cannot emit
  `reads: []`/`creates: []` **closed**, because a closed negative from "no
  requirement was derived" manufactures knowledge. Only a hand audit asserts
  that closure — and the one that does is already checked in.

### Slice 2 — `.d.ts` entrypoints: one predicate decides refusal vs inapplicable

Classes D (11) and E (1) are `.d.ts` **entrypoints**, the same population as
the 16 cases already recorded `non-emitting-module-target` /
`verifier-proved-type-only`. `artifactCaseDisposition`
(`generate-package-contract.mjs:125`, `nonEmittingModuleTarget` at
`:153`) runs **before** `prepareArtifact`, so the split is decided entirely
inside `moduleEmission`
(`artifact-resolution.mjs:657`). Measured on the kept install:

| entrypoint | `moduleEmission(…, "declaration-file")` |
| --- | --- |
| `types/render/component.d.ts`, `store/types/store.d.ts` (and 14 more) | `{verdict: non-emitting, statements: N}` → **inapplicable** |
| `types/index.d.ts`, `types/jsx.d.ts`, `types/reactive/{array,observable,signal}.d.ts`, `types/render/{flow,hydration}.d.ts`, `types/server/rendering.d.ts`, `store/types/{index,mutable}.d.ts`, `web/types/{client,server}.d.ts` | `{verdict: "emitting", kind: "value import"}` → **refused** |

The whole difference is a non-type-only `import … from "./x.js"` statement. The
`declarationFile` arm of `declarationFileStatement`
(`artifact-resolution.mjs:506-532`) special-cases `ExportDeclaration` and
`ExportAssignment` and then falls through to `statementEmits`, which calls a
plain `ImportDeclaration` a *value import*. In a `.d.ts` that is not true of
the file being classified: TypeScript never emits a declaration file, so no
import in it evaluates — including a bare `import "./side-effect.js"`.

- **Yield**: solid-js@1.9.14 38 → **26** refusals, +12 inapplicable. Corpus:
  19 class-D refusals (3 rows) + 38 class-E refusals (4 rows, of which
  `@kobalte/solidbase@0.6.13|solid1|only` alone has 30) = up to 57 refusals in
  5 rows.
- **Blast radius**: `artifactCaseDisposition` is a content premise that travels
  to certification as a `declaredApplicabilityClaims` row Rust re-proves
  (`generate-package-contract.mjs:177-190`), so the Rust `nonEmittingModuleTarget`
  equivalent must move in the same commit or the two lanes disagree. No new
  trust surface: the suffix premise is already accepted for the 16.
- **Must not clear**: the `@solid-devtools/shared` `export {}` / zero-byte
  cases stay *non-answers* (a module that declares nothing is not an answer);
  a `.js`-flavored target with a value import stays `emitting`.

### Slice 3 — Intra-archive cross-entrypoint binding (§3) **together with** the declaration-axis kind rule (§4)

Neither ships alone: measured, B unblocks `./web` and `Aliases` refuses one
step later, and the declaration-axis rule alone cannot be reached because B
refuses first.

- **Yield**: solid-js@1.9.14's 11 class-B refusals plus the 11 latent `Aliases`
  refusals behind them → the 7 `./web` condition cases and 4 `./web/*` cases
  generate; `class` would go 26 → 15 refusals with slice 2. The declaration-axis
  rule additionally reaches the 3 measured devtools refusals (3 rows).
- **Blast radius**: B is genuinely narrow — **the self-package
  cross-entrypoint class is 11 refusals in exactly 1 row corpus-wide**
  (solid-js@1.9.14); the 102 other `has no exact runtime binding` refusals are
  *external* dependencies and must be untouched. The §4 rule is broader: it
  changes `promote_entry_callable`'s answer for every `(Unknown, Unknown)`
  export in the corpus, and both callers of `reconcile_entry_export_kind` must
  move together.
- **Must not clear**: `@solid-devtools/locator@0.16.7`'s `addClickInterceptor`
  must **not** publish `kind: "value"` — if its declaration axis is `Callable`
  the rule must raise to `function` with `callbacks` unknown, and if the
  declaration axis is absent or `Mixed` the refusal must stand. `corvu@0.7.2`
  (18 external binding refusals) and `motion-solidjs@0.6.0` (6) must keep them.
- **Not measured**: whether the 3 devtools rows' declaration axes are closed.
  Measure before sizing.

### Slice 4 — Name the identity premise, or do not (§2)

If a self-artifact identity premise ships at all after slice 1, it ships for
the **argument/callback-flow families only** — the ones the audit agrees exist
— never for reads or owner requirements. Its measured yield in that restricted
form is **24 demands** (`onMount`, entirely), against ADR 0005's five
preconditions, of which 1, 3 and 4 are reachable and **2 is already satisfied
for 1.x** (the `solid_1x.rs` tables cite `1.9.14`, the exact bytes). That is a
poor trade on its own and should be judged only after slice 1 has removed the
ill-formed demands.

- **Must not clear**, if attempted: `solid-js@2.0.0-rc.3|solid2|only` stays
  certified; `@solidjs/signals@2.0.0-rc.3|solid2|only` keeps refusing on
  `sha256:78a16558…` **unless** its own ADR-0005 objections are separately
  discharged; a contract-corpus fixture named `solid-js@1.9.14` with the
  fabricated `fixture:sha256:<manifest digest>` integrity
  (`scripts/contract-corpus.mjs:60`) must be **refused** the premise, with the
  disagreeing field named; `@solidjs/signals@2.0.0-rc.3` must not gain the
  premise from a `solid-js` tuple — the audited-tuple table is keyed per
  package with its own integrity. Signals *does* have an audited tuple
  (`pkg/contracts/bundled/solid-v2/solidjs-signals.json` `package.integrity`
  `sha512-/yPhTf3xS1FRR4MX8kTYCd4MjsFxzwkO+KyOTfbu35lTEiaJ4Fxy+JL91XonDzt31GV1mYaZ9CGD2TQIzvXuNA==`,
  manifest `sha256:22d27a9e…`), so its admission is governed by ADR 0005's own
  objections — above all the same generated-versus-audited `creates`
  contradiction slice 1 addresses — and not by a missing integrity.

### Slice 5 — Two small honest fixes surfaced by the census

- **`createRenderEffect` in `./dist/server.js`** (8 demands, 1 export): the
  export is `const createRenderEffect = createComputed;`
  (`dist/server.js:77`), and `verify_export_value_subject` refuses *"resolved
  value declaration name disagrees with snapshot export replay"*. An
  intra-module alias of a local function is a well-defined identity question,
  not a missing fact; today the refusal does not name the export (I had to
  patch `proof_artifact_export` into the message to identify it). At minimum
  the message should name the subject.
- **Class C's message is misleading**: `./jsx-runtime` refuses with *"entry
  file `<root>/dist/solid.js` has no runtime ESM exports"*, but `dist/solid.js`
  plainly has 54. The entrypoint pairs runtime `dist/solid.js` with
  declarations `types/jsx.d.ts`, which declares only the `JSX` namespace, so
  the *intersection* is empty. 64 refusals in 14 rows carry this message
  (41 of them `@kobalte/core@0.13.13`); whether they are all the same shape is
  unmeasured.

---

## 7. Exact remaining refusals

Unchanged and honest at the reverted build:

- `solid-js@1.9.14|solid1|only` — `partial-success`, 38 artifact cases refused,
  17 inapplicable; certification refused at `witness-acquisition`,
  `recursive-value-shape`
  `sha256:5463f0ed0af9a202b45f80b731fdba6d9048d2898f3ab5847973ec4156370f7c`,
  `331dfa49…:ErrorBoundary:operation:read-0`.
- 614 of its 3412 Type-Facts demands refuse. **413 of them (the 369 read and 44
  owner-requirement demands) are demands the audited bundled contract for
  byte-identical bytes closes as absent** — those must be *withdrawn*, not
  proved.
- 201 refuse for reasons the audit does not contradict: 110 argument flow, 22
  dialect callback flow, 22 root shape, 13 value path locally open, 11 path
  absent from census, 8 subject mismatch, 3 recursive input absent, 12 invoke
  input no call. Of these, 24 clear with a sound self-identity premise; **110
  terminate in a non-exported helper (`createComputation`) or in a returned
  closure and are unreachable by any table**, with or without any premise.
- The whole census was taken with certification stopped only by the diagnostic
  patch. `@solidjs/web@2.0.0-rc.3|solid2|only` was **not** re-measured for the
  same self-package pattern (it has 11 `solid-js` binding refusals of the
  *external* kind and 7 class-D + 5 class-E refusals); slices 2 and 3 should
  measure it.
- The full 418-probe corpus was **not** re-run: this was a read-only
  diagnosis, and the only three rows measured at the reverted build are the
  §5.d controls.
