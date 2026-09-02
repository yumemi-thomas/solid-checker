# "Root observation is open" — diagnosis

HEAD verified `9c7326f10958fa228e51ef0a37cade6078305a08` on
`codex/phase19a-authenticated-proof-policy`. No tracked file modified. No cargo
build/test run. Checker binary `rust/target/debug/solid-checker-rust`
(2026-09-01 10:28) and `bin/solid-typefacts` (09:28) used as found.

Scratch: `/private/tmp/claude-501/-Users-thomas-Documents-Github-solid-checker/4adf7833-51b7-4c1e-95da-c24558fa3dff/scratchpad/next-rounds/roots/`
(`targets.json`, `probes.json`, `repro.json`, `oracle-out.txt`, `oracle*.mjs`,
`atloc*.mjs`).

---

## 1. Target set and reproduction gate

24 refused rows in the checked-in 418-probe report match the target reason
strings, over **19 distinct Type Facts demand digests** (floor/head twins and the
two TanStack rows share a digest).

Batch probe (all 20 distinct probes; the 4 head twins deduped by digest):

```
SOLID_CHECKER_NATIVE_BIN=".../rust/target/debug/solid-checker-rust" \
SOLID_TYPEFACTS_BIN=".../bin/solid-typefacts" \
bun scripts/ecosystem-benchmark/run.mjs --timeout 600 --attempt-certification --keep-temp \
  --probe ... (20 ids) --json probes.json
```

**20/20 reproduce exactly** — same demand digest, same proof family, same
artifact case, same export name, same refusal tail. Zero exclusions. The four
deduped head rows (`cookies|head`, `favicon|head`, `i18n@3.0.0-next.4|head`,
`websocket|head`) carry byte-identical demand digests to their reproduced floor
twins and are therefore covered, not skipped silently.

### Method for "why open"

The producer's root observation is `invocationValueFactLocked`
(`apps/solid-typefacts/internal/typefacts/tsgo/invocation_transcripts.go:490`).
Its four questions, and only these, decide `require_verifiable_root_premise`
(`rust/crates/solid-facts-backend/src/contract_certification/type_facts.rs:3511`):

| producer field | set open by |
| --- | --- |
| `OpenReasons += "openType"` | `Any \| Unknown \| IncludesError` |
| `OpenReasons += "unresolvedGeneric"` | `Instantiable` |
| `OpenReasons += "openIndex"` | `len(GetIndexInfosOfType(v)) != 0` |
| `Callability` / `Constructability` | `Unknown` from `callabilityOfType` / `invocationConstructabilityOfType` |
| `Primitive.Unknown` | `primitiveValueDomainOfType(...).Unknown()` |

`Partitions` cannot contribute: `finitePartitionsLocked` only ever appends
partitions with `Complete: true`, so `require_closed_value`'s partition arm is
unreachable at the root. `Alternatives` never carry open reasons from this
producer either. **The root premise is decided entirely by the five rows above.**

I replicated those five questions against each subject's *real published
typings* with the TypeScript 5.9.3 checker API (`oracle.mjs`, `oracle2.mjs`,
`atloc.mjs`), including one run (`atloc2.mjs`) that reproduces the producer's
private project shape verbatim from `type_facts.rs:833` — runtime `.js` roots,
`moduleResolution: bundler`, `skipLibCheck: false`, `allowJs`, `checkJs: false`,
`maxNodeModuleJsDepth: 100`, `types: []`, no `jsxImportSource`.

---

## 2. Per-row table

Root address column: `out` = `ValueRoot::OperationOutput` (return), `in[n]` =
`ValueRoot::OperationInput` index n, `export` = `ValueRoot::Export`. Where the
proposal's domain claims pin the address it is quoted; where they do not, the
address is derived by elimination (every other candidate root measures CLOSED,
so it cannot be the one that refused) and marked *(elim)*.

| probeId | demand digest | export | root | root type (from the .d.ts) | why open | class |
| --- | --- | --- | --- | --- | --- | --- |
| `@solid-primitives/reducer@0.0.101\|solid1\|only` | `d01d9421…` | `createReducer` | out | `[Accessor<State>, (...args: ActionData) => void]` | `openIndex(number)` — tuple, fixedLen 2, no rest | **a** |
| `@solid-primitives/selection@0.1.3\|solid1\|only` | `59c2bdc4…` | `createSelection` | out | `[Accessor<HTMLSelection>, Setter<HTMLSelection>]` | `openIndex(number)` — tuple, fixedLen 2 | **a** |
| `@solid-primitives/share@2.2.5\|solid1\|only` | `cca72a93…` | `createSocialShare` | out | `[share: (network: string \| undefined) => void, close: () => void, isSharing: Accessor<boolean>]` | `openIndex(number)` — tuple, fixedLen 3 | **a** |
| `@solid-primitives/db-store@1.1.4\|solid1\|only` | `e837e2d5…` | `createDbStore` | out | `[Row[], SetStoreFunction<Row[]>, { refetch: () => void; }]` | `openIndex(number)` — tuple, fixedLen 3 | **a** |
| `@solid-primitives/cookies@1.0.0-next.2\|solid2\|floor+head` | `16f840b9…` | `createServerCookie` | out | `Signal<T>` = `[SourceAccessor<T>, Setter<T>]` | `openIndex(number)` — tuple, fixedLen 2 | **a** |
| `@solid-primitives/controlled-props@0.1.4\|solid1\|only` | `2275b99f…` | `createControlledProp` | out | `TestPropReturn<T>` = `[Accessor<T>, Setter<T>, Component]` | `openIndex(number)` — tuple, fixedLen 3 (6-overload set; every overload's return is this tuple, and `in[0] name: string` is also `openIndex(number)`) | **a** |
| `@kobalte/utils@0.9.2\|solid1\|only` | `ad951b04…` | `debugPolygon` | in[0] *(elim)* | `Polygon = Point[]` | `openIndex(number)` — array. Output `HTMLElement \| null` measures CLOSED, so the input is the only candidate | **a** |
| `@solid-primitives/i18n@2.2.1\|solid1\|only` | `5d48e2da…` | `identityResolveTemplate` | in[0] or out | `<T extends string>(template: T, ...args: ResolveArgs<T,string>): string` — `in[0] T`: `unresolvedGeneric` + `openIndex(number)`; `in[1] ResolveArgs<T,string>`: `unresolvedGeneric` + `openIndex(number)`; `out string`: `openIndex(number)` | all three roots open; `string`'s index info is the intrinsic `String` wrapper's `readonly [index:number]: string` | **a** (out, `string`) / **b** (in, bare `T`) |
| `@solid-primitives/i18n@3.0.0-next.4\|solid2\|floor+head` | `2bd38ac7…` | `identityResolveTemplate` | ditto | identical declaration | ditto | **a**/**b** |
| `@solidjs/element@2.0.0-rc.3\|solid2\|only` (graph node `component-register@0.8.8`) | `563e1bd6…` | `hot` | in[0] *(elim)* | `hot(module: NodeModule & { hot?: any }, tagName: string): void` | `NodeModule` is an ambient `@types/node` global; the producer's project sets **`types: []`**, so it resolves to `any` → `openType`, callability `Unknown`, primitive `Unknown`. Output `void` measures CLOSED | **a** (project config) |
| `@tanstack/solid-store@0.11.1\|solid1\|only` (graph node `@tanstack/store@0.11.1`) | `9a32393c…` | `shallow` | in[0] *(elim)* | `declare function shallow<T>(objA: T, objB: T): boolean` | bare **unconstrained** type parameter `T` → `Instantiable` → `unresolvedGeneric`; `GetConstraintOfTypeParameter` is nil → primitive domain `Unknown`. Output `boolean` measures CLOSED | **b** |
| `@tanstack/solid-form@2.0.0-alpha.2\|solid1\|only` | `9a32393c…` (same) | `shallow` | same | same | same | **b** |
| `@solidjs/web@2.0.0-rc.3\|solid2\|only` | `c6644c38…` | `asyncArg` | out | `export declare function asyncArg<T>(value: PromiseLike<T> \| AsyncIterable<T>): T` | return type **is** the bare unconstrained `T` → `unresolvedGeneric`, primitive `Unknown` | **b** |
| `@corvu/utils@0.4.2\|solid1\|only` | `6435189c…` | `default` | out | package has **no default export**; `dist/index.d.ts` ends `export { dataIf, isButton, isFunction }`. `import x from '@corvu/utils'` has no declared value; TS only yields the module namespace via synthetic default | the IR inventoried an `operation:return` value root on an export the declaration does not have | **c** |
| `@corvu-next/utils@0.1.5\|solid2\|only` | `b381a04c…` | `default` | out | identical (same emitted chunk) | same | **c** |
| `@solid-primitives/favicon@1.0.0-next.1\|solid2\|floor+head` | `1df2f037…` | `FaviconLink` | export | `declare const FaviconLink: Component<FaviconLinkProps>`; `Component<P> = (props: P) => SolidElement`; runtime `const FaviconLink = (props) => …` | **UNEXPLAINED.** Under a faithful replica of the producer's private project (`atloc2.mjs`) TS answers `Callable` at *every* identifier (`components.d.ts:25` decl, `components.d.ts:27` export specifier, `index.js:5` import specifier, `index.js:8` export specifier), zero diagnostics. `require_root_callability` can only produce this message from `Callability ∈ {NonCallable, Mixed, Unknown}` — so the producer's `transcript.value` is almost certainly an **early-refusal transcript** (`valueUnavailable`, from one of `sourceUnavailable` / `identifierNotExact` / `symbolUnresolved` / `aliasUnresolved` / `declarationUnavailable` at `export_value_transcripts.go:89-125`), not a real observation | **needs instrumentation** |
| `@solid-devtools/ui@0.10.3\|solid1\|only` | `0edff50f…` | `SignalContextProvider` | export | `declare const SignalContextProvider: ContextProviderComponent<SignalContextState \| undefined>` imported from **`solid-js/types/reactive/signal`** | that deep path does not exist in the installed solid-js; `tsc` reports *Cannot find module 'solid-js/types/reactive/signal'* three times → error type → `openType`, callability `Unknown` | **b** (must-not-clear) |
| `solid-devtools@0.34.5\|solid1\|only` | `0e9d0363…` | `namePlugin` | export | `import * as babel from '@babel/core'; export declare const namePlugin: babel.PluginObj<any>` | `@babel/core` ships no typings and `@types/babel__core` is not installed; `tsc` reports *Could not find a declaration file for module '@babel/core'* → `any` → callability `Unknown` | **b** (must-not-clear) |
| `@kobalte/core@2.0.0-alpha.0\|solid2\|only` | `32c3e7f3…` | `createDomCollection` | out, path `[.items]` | `(props?) => { DomCollectionProvider: FlowComponent; items: SourceAccessor<T[]> }` | **depthLimit** — see §4. Root and `props` both measure CLOSED | **a** |
| `@solid-primitives/websocket@2.0.0-next.3\|solid2\|floor+head` | `5108bf30…` | `createWSStore` | out, path `[0]` or `[1]` | `[get: Refreshable<Readonly<S>>, set: StoreSetter<S>]` | **depthLimit** (§4). The tuple root *additionally* carries `openIndex(number)` (§3), so this row needs both fixes | **a** |

### Counts

| class | rows | distinct digests |
| --- | --- | --- |
| **a** — producer could answer; it asks the wrong question or asks under a project that cannot see the answer | 14 | 11 |
| **b** — genuinely unknowable statically (bare unconstrained type parameter, or published typings `tsc` itself rejects) | 6 | 5 |
| **c** — the demand should not exist (`default` export the declaration does not have) | 2 | 2 |
| unexplained (needs producer instrumentation) | 2 | 1 |

Some rows appear in two classes because more than one candidate root is open
(i18n; websocket). Where a row is listed under **a** and **b**, the **b** part is
the must-not-clear part.

---

## 3. Mechanism M1 — `openIndex` is a member-enumeration signal recorded on a shape fact

**This is the biggest recoverable subclass: 11 of 24 rows, 8 of 19 digests.**

`invocation_transcripts.go:522`

```go
if len(p.checker.GetIndexInfosOfType(value)) != 0 {
    fact.OpenReasons = append(fact.OpenReasons, "openIndex")
}
```

`GetIndexInfosOfType` answers on the **reduced apparent type**. Measured against
the real typings:

* a tuple `[A, B]` → `[number]` index info (from the `Array` base) — every
  tuple-returning primitive in the corpus,
* an array `Point[]` → `[number]`,
* the primitive `string` → `readonly [index: number]: string` from the intrinsic
  `String` interface.

None of these is an author-declared open key space. In all three the value's own
**shape** — callability, constructability, primitive domain — is fully
determined, and in the tuple case every member is already enumerated by the
callable-path walk as `[0]…[n-1]`.

The conflation is the defect: `InvocationValueFact.open_reasons` is consumed
**only** by root-shape questions (`require_closed_value`,
`require_root_callability`, `require_signature_parameter_callable`), while
`CallablePathFact.open_reasons` is the member-enumeration signal that
`require_all_callable_paths_closed` / `require_export_callable_paths_closed`
read. `openIndex` belongs to the second and is being written into the first.

### Spec M1 (bounded, over-proof-safe)

Producer side only. **The gate `require_verifiable_root_premise` is not touched
— an open observation must still never discharge.** The extension makes an
observation *closed* only where the type system genuinely answers.

Keep `openIndex` exactly as it is inside `walkCallablePathsLocked` (path facts).
In `invocationValueFactLocked`, ask a narrower question before stamping. Three
independently gateable arms, in increasing order of blast radius — ship them one
at a time and measure each:

* **M1a — exact-length tuple.** The producer already computes this:
  `tupleShapeOfType(...)` returns `ExactLengthKnown` (false as soon as any
  element flag is `ElementFlagsNonRequired`) and `HasRest`. When the value is a
  single non-union tuple with `ExactLengthKnown && !HasRest`, do **not** stamp
  `openIndex`. Justification: the sole index info is the synthesized numeric one
  whose type is the union of the fixed elements; there is no key the walk did
  not visit.
  Clears: `d01d9421`, `59c2bdc4`, `cca72a93`, `e837e2d5`, `16f840b9`,
  `2275b99f`, and the *root half* of `5108bf30`.
* **M1b — primitive apparent type.** When `primitiveValueDomainOfType` answers a
  non-`Unknown`, non-object domain (string/number/boolean/bigint/symbol), the
  index infos are the intrinsic wrapper interface's. Do not stamp.
  Clears the `out string` half of `5d48e2da` / `2bd38ac7`.
* **M1c — array / ReadonlyArray.** A single numeric index info whose value type
  is the element type. Do not stamp on the *root* fact. This is the widest arm:
  unlike a tuple, an array really does hide arbitrarily many members — which is
  precisely why it must stay stamped on **path** facts, where member enumeration
  is what is being asserted. Ship M1c only after M1a/M1b are measured.
  Clears: `ad951b04`.

**What stays fail-closed after M1.** A `string`-keyed or symbol-keyed index
signature (`Record<string, T>`, `{ [k: string]: T }`) keeps `openIndex` on the
root under all three arms. Union roots where any constituent is not an
exact-length tuple keep it. Every child path of an index-signature object
continues to fail with *"path is absent from the producer census"*, which is the
backstop that makes M1 safe: closing the root's *shape* never closes any claim
about a member.

### Bonus defect found in the same walk (fix in this round)

`invocation_transcripts.go:899-902`

```go
if len(p.checker.GetIndexInfosOfType(value)) != 0 {
    last := &(*paths)[len(*paths)-1]
    last.Complete = false
    last.OpenReasons = append(last.OpenReasons, "openIndex")
}
```

`last` is the last path appended *after* the property loop has recursed — i.e. a
descendant, not the node this index signature belongs to. The `depthLimit` and
`cycle` arms above it are correct because they `return` immediately; this one
does not. Effect: for any value with **both** properties and an index signature,
`openIndex` is stamped on an unrelated deeper path and the node that actually has
the open key space is left `Complete`. That is an error in the **permissive**
direction — a latent over-permission independent of everything else here. It
should be `fact`'s own entry (capture the index before recursing, or re-find by
position).

---

## 4. Mechanism M4 — `depthLimit` marks the demanded node open for having children

2 digests (`32c3e7f3` kobalte/core, `5108bf30` websocket), 3 rows.

`export_value_callable_depth` (`type_facts.rs:1199`) requests
**`depth == path.0.len()`** — exactly the demanded path's own length, capped at
`MAX_INVOCATION_CALLABLE_DEPTH = 8`. `walkCallablePathsLocked` therefore arrives
at the demanded node with `remaining == 0`, and:

```go
if value == nil || remaining == 0 {
    if value != nil && (len(p.checker.GetPropertiesOfType(value)) != 0 || checker.IsTupleType(value)) {
        last.Complete = false
        last.OpenReasons = append(last.OpenReasons, "depthLimit")
    }
    return
}
```

So the demanded node is stamped incomplete **purely because it has children the
walk was not asked to visit** — even though the node's own `Complete` flag was
already computed from its own answers one statement earlier, and came out true.
Measured:

* kobalte `createDomCollection` → `.items: SourceAccessor<T[]>` — reported
  `presence=Required, callability=Callable, complete=false, reasons=["depthLimit"]`.
  Callable *is* the answer the demand wanted; `SourceAccessor` has properties, so
  the flag was cleared anyway. Depth requested: **1**.
* websocket `createWSStore` → tuple element — `callability=NonCallable`,
  same shape. Depth requested: **1**.

Neither is a depth *shortage*. The depth is exactly right for what the demand
asserts; the traversal conflates "I did not enumerate this node's subtree" with
"I do not know this node".

**Raising the depth is not the sound fix.** `depth = path.len() + 1` would clear
both rows, but (i) `callableDepth` is hashed into the demand digest, invalidating
every cached transcript, and (ii) `require_all_callable_paths_closed` and
`require_export_callable_paths_closed` require **every** path in the census to be
closed — a deeper walk *adds* paths that can be open, so deepening can turn
currently-certified rows into refusals. It is not monotone and must not be done
blindly.

### Spec M4 (over-proof-safe)

Split the two meanings on the wire instead of moving the budget:

* Producer: at `remaining == 0`, stop clearing `Complete` and stop appending
  `depthLimit` to `OpenReasons`. Record the fact on a **new additive field**,
  e.g. `SubtreeEnumerated bool` (or an open reason on a separate
  `SubtreeOpenReasons` list), set false at the depth cut and at the `cycle` cut.
  The node's own `Complete` keeps meaning exactly what it means today: the node
  is not any/unknown/error and its callability and constructability are answered.
* Consumer: `require_operation_recursive_signature` /
  `require_export_recursive_subject` — which assert the shape *at* the demanded
  path and nothing about its members — ignore `SubtreeEnumerated`.
  `require_all_callable_paths_closed` and `require_export_callable_paths_closed`
  — which assert the whole census is closed — refuse when it is false, exactly
  as they refuse `depthLimit` today.
* Protocol: additive field, schema digest moves, both halves ship together.

This never turns an Unknown answer into a known one; it stops one gate from
reading the other gate's premise.

---

## 5. Mechanism M6 — `types: []` hides ambient globals the real project has

1 digest (`563e1bd6`, `@solidjs/element` via `component-register@0.8.8` `hot`).

The producer's private project is written with `"types": []`
(`type_facts.rs:844`). `component-register`'s `hot(module: NodeModule & { hot?: any }, …)`
names the ambient `NodeModule` from `@types/node`; with `types: []` it resolves
to `any`. In a consumer's real project with `@types/node` installed, `tsc`
answers this fine.

This is **class a** but it is a policy question, not a bug: loading ambient
`@types` into the private project means certifying against typings that are not
part of the authenticated artifact closure. Given the "Certify against
authenticated dependency typings" work already in this branch, the honest
options are (i) leave it fail-closed and document it, or (ii) admit only
`@types/*` packages that are themselves authenticated members of the closure.
**Do not** relax it as a side effect of M1 — flag it as its own decision with its
own acceptance digest.

---

## 6. Class c — the demand should not be asking

2 rows, 2 digests (`6435189c` `@corvu/utils`, `b381a04c` `@corvu-next/utils`).

Both packages' `dist/index.d.ts` end in `export { dataIf, isButton, isFunction }`
and declare no `default`. The IR nevertheless inventoried an
`…:default:operation:return` value root (confirmed in each probe's
`*.proposal.json`: `subject.export == "default"`, `path.root.kind ==
"operation-output"`, `path.path == []`). Under the producer's own resolution the
only thing `default` can name is the synthetic module namespace — which is not
callable and has no return operation at all.

Note the certifier already refuses `default` explicitly on the neighbouring
selected-signature path: `verify_declaration_export_identity`
(`type_facts.rs:2934`) returns *"Type Facts display names cannot authenticate a
default-export alias"*. The recursive-value path has no such guard and instead
produces a misleading "root observation is open".

**Route to the demand owner** (`inventory_value_shape` /
`recursive_value_callability` in
`rust/crates/solid-reactive-ir/src/contract_semantics/certification.rs`): either
stop inventorying value roots for a `default` the declaration census does not
carry, or make the refusal say so — an *UnsupportedDemand* naming the missing
default export, not an openness claim. Either way these two rows convert from a
misdiagnosed refusal to an honest one; neither converts to a certification.

---

## 7. Acceptance digests

A mechanism is done when **all** of its digests certify and **none** of §8 moves.

| mechanism | acceptance digests (probe → demand) |
| --- | --- |
| **M1a** exact-length tuple | `d01d9421…` reducer/`createReducer` · `59c2bdc4…` selection/`createSelection` · `cca72a93…` share/`createSocialShare` · `e837e2d5…` db-store/`createDbStore` · `16f840b9…` cookies/`createServerCookie` (floor+head) · `2275b99f…` controlled-props/`createControlledProp` |
| **M1b** primitive apparent type | `5d48e2da…` i18n@2.2.1/`identityResolveTemplate` and `2bd38ac7…` i18n@3.0.0-next.4/`identityResolveTemplate` — **only if** the failing root is the `string` output. If either still refuses after M1b, the failing root is `in[0] T`/`in[1] ResolveArgs<T,string>` and the row is class **b**, not an M1b acceptance |
| **M1c** array | `ad951b04…` @kobalte/utils/`debugPolygon` |
| **M4** subtree/answer split | `32c3e7f3…` @kobalte/core/`createDomCollection` |
| **M1a + M4 together** | `5108bf30…` websocket/`createWSStore` (floor+head) — needs both; neither alone clears it |
| **M6** (separate decision) | `563e1bd6…` @solidjs/element via component-register/`hot` |
| **class c** (refusal-quality only) | `6435189c…`, `b381a04c…` — must change *reason*, must **not** certify |
| **favicon** (instrumentation first) | `1df2f037…` — do not attempt a fix until the producer's actual transcript is dumped |

Best case for the whole round: **9 digests / 13 rows** recovered by M1+M4, taking
the corpus from 320/418 certified to ~333/418. `@solidjs/element` adds 1 more if
M6 is decided in favour.

---

## 8. Must-not-clear rows

Every one of these is a case where `tsc` itself has no answer, or has an error.
If a change certifies any of them, the change is a false certification and must
be reverted, not tuned.

1. **`@solid-devtools/ui@0.10.3|solid1|only`** — `0edff50f…`. The published
   `index.d.ts` imports `solid-js/types/reactive/signal`, which does not exist in
   the installed solid-js. `tsc` (skipLibCheck off): *Cannot find module
   'solid-js/types/reactive/signal'* ×3. The export root is an error type.
2. **`solid-devtools@0.34.5|solid1|only`** — `0e9d0363…`. `namePlugin:
   babel.PluginObj<any>` where `@babel/core` ships no typings and
   `@types/babel__core` is absent. `tsc`: *Could not find a declaration file for
   module '@babel/core' … implicitly has an 'any' type.*
3. **`@solidjs/web@2.0.0-rc.3|solid2|only`** — `c6644c38…`. `asyncArg<T>(…): T`
   returns a bare **unconstrained** type parameter. There is no callability,
   constructability or primitive domain to observe.
4. **`@tanstack/solid-store@0.11.1|solid1|only`** and
   **`@tanstack/solid-form@2.0.0-alpha.2|solid1|only`** — both `9a32393c…`.
   `shallow<T>(objA: T, objB: T)` — bare unconstrained `T` inputs, no constraint
   for `GetConstraintOfTypeParameter` to reach.
5. **The `in[0] T` / `in[1] ResolveArgs<T, string>` roots of i18n** — if M1b does
   not clear `5d48e2da…` / `2bd38ac7…`, that is the correct outcome, not a
   shortfall. `T extends string` does have a constraint, so the primitive domain
   resolves; but `Instantiable` is still set and `unresolvedGeneric` is the
   honest answer for a type the caller instantiates.

Structural invariant to assert alongside these: after any M1 arm,
`require_verifiable_root_premise` must still refuse a value carrying **any**
remaining open reason. The gate's text and the F3 lesson stand unchanged — the
producer stops manufacturing an openness that is not one; it does not gain a way
to discharge one that is.

---

## 9. Control probes (must stay certified)

Run with the same batch. All five are currently `certified` in the checked-in
report and all five exercise tuple/array/string-shaped roots that M1 touches.

1. **`@solid-primitives/scheduled@1.5.3|solid1|only`** — the row the backlog
   names as recovered by the F3 repair (`createScheduled` clears every
   callable-path demand and used to die on the root shape). The single most
   important regression sentinel for anything near
   `require_verifiable_root_premise`.
2. **`@solid-primitives/map@0.7.4|solid1|only`** — reactive `Map`/`Set` wrappers;
   class types with real `string`-keyed and iterator members. Confirms M1 did not
   leak into genuinely open key spaces.
3. **`@solid-primitives/refs@1.1.4|solid1|only`** — array-valued and
   element-collection returns; the direct sentinel for M1c.
4. **`@solid-primitives/event-listener@2.4.6|solid1|only`** — dense callable-path
   census with optional/union parameters; the sentinel for M4's split not
   loosening `require_all_callable_paths_closed`.
5. **`@solid-primitives/storage@4.4.0|solid1|only`** — tuple returns *plus*
   index-signature options objects in one export; catches an M1a arm written too
   wide (a union of tuple-and-object must stay open).

A control that flips from certified to refused is as much a failure as a
must-not-clear row that certifies: M4's wire split in particular can move
`require_all_callable_paths_closed` in either direction if the new field is
defaulted wrong on an old transcript.

---

## 10. Open item requiring instrumentation before any code change

`@solid-primitives/favicon` `FaviconLink` (`1df2f037…`, 2 rows) is the one
subject whose refusal I could not explain from the declaration. Every static
measurement says `Callable`:

* `dist/components.d.ts:25` — `declare const FaviconLink: Component<FaviconLinkProps>`,
  and `Component<P> = (props: P) => SolidElement` in the installed solid-js
  (`solid-js/types/client/component.d.ts:6`);
* `dist/components.js:21` — `const FaviconLink = (props) => (() => {…})`, callable
  even with no declarations at all;
* under the producer's own project shape (`atloc2.mjs`, runtime `.js` roots,
  bundler, `skipLibCheck: false`, `types: []`) TS reports **zero diagnostics** and
  `call=Callable` at all four identifier positions.

Two hypotheses I tested and **falsified**:

* *`@ts-self-types` unsupported by the pinned tsgo* — falsified by
  `@solid-primitives/websocket`, which carries the same pragma and whose refusal
  quotes a typed answer (`callability=NonCallable` at a tuple element) that could
  only come from reading its `.d.ts`.
* *the materialized snapshot omits non-entry declaration files* — falsified:
  `ArtifactSnapshot::from_archive` (`contract_certification.rs:1186`) snapshots
  the whole published tarball.

Remaining hypothesis, and the next step: the transcript is an **early refusal**
(`export_value_transcripts.go:89-125` — `sourceUnavailable` /
`identifierNotExact` / `symbolUnresolved` / `aliasUnresolved` /
`declarationUnavailable`), which fabricates
`Callability: Unknown, Constructability: Unknown, Primitive.Unknown: true,
OpenReasons: ["valueUnavailable"]` and produces *exactly* this message. Dump
`transcript.OpenReasons` for this demand (a one-run temporary print in
`exportValueTranscriptLocked`, removed immediately per the fast-loop rule) before
proposing anything. If it is `identifierNotExact`, the bug is in the byte range
the Rust declaration census hands the producer for a re-exported binding, and it
plausibly affects more rows than this one.

---

## 11. Implementation addendum (2026-09-01)

The implementation round byte-reproduced every targeted digest before changing
the producer, then exposed two corrections to this diagnosis.

First, the two Corvu demands do not describe the package-root `default`. Their
artifact case is the exported subpath `./create/controllableSignal`; its selected
declaration (`dist/create/controllableSignal.d.ts`) and both selected runtime
files explicitly default-export the same callable. The table's class-c premise
was therefore attached to the wrong declaration file. Both rows legitimately
certify after the tuple root becomes closed; retaining an invented missing-export
refusal would contradict the authenticated artifact-case identity.

Second, the favicon transcript is not an early refusal. A temporary producer
dump, removed immediately after the one reproducer, showed a completed export
observation whose root is `openType`, with unknown callability and
constructability. The remaining mismatch is therefore inside the producer's
type observation for that exact export, not the Rust declaration span or an
early `valueUnavailable` path. Both favicon rows remain refused; no speculative
fix was made.

M4 also revealed that `createDomCollection.items` is locally callable and its
subtree is enumerated after the wire split, but the next exact requested union
alternative is absent. Its demand moved from `32c3e7f3…` (`depthLimit`) to
`1808f351…` (`presence=Absent`) and correctly remains refused. This supersedes
the M4 acceptance claim above without weakening the consumer.
