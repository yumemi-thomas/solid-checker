# Accepted-dependency-contract composition — scoping study

Read-only study. Repo `/Users/thomas/Documents/Github/solid-checker`, branch
`codex/phase19a-authenticated-proof-policy`, **HEAD `9c7326f1`** (verified).
No tracked file modified. No cargo build/test run; `rust/target/debug/solid-checker-rust`
(2026-09-01 10:28) and `bin/solid-typefacts` (2026-09-01 09:28) used as they are —
neither refused on a build digest.

Reproductions used `bun scripts/ecosystem-benchmark/run.mjs --probe … --keep-temp
--attempt-certification` with `SOLID_CHECKER_NATIVE_BIN` pointed at the fresh debug
binary, writing reports into this scratch directory (never `benchmarks/ecosystem/`).

---

## 0. Headline

Two things called "composition" are conflated in the ecosystem report, and they have
opposite readiness.

| | **Binding composition** | **Semantic-claim composition** |
|---|---|---|
| Claim composed | "dependency D's export E resolves to *this* runtime binding" | "dependency D's export E **invokes its argument N**" |
| Refusal family | `accepted dependency D has no exact <axis> binding for export E` | `callback parameter has no exact direct-call or resolved-argument flow` |
| Status | **Already implemented and already working**, via the *private, unauthenticated* proposal-dependency lane inside `contract certify` | **Not implemented at all.** The witness never reads the dependency's `CallClaims` |
| Ecosystem effect today | 9 rows certified through it | 0 rows |

The single biggest surprise of this study: **moving binding composition onto the
authenticated accepted-contract lane unlocks zero additional ecosystem rows**, because
the private lane already clears every row whose dependencies are certifiable. Its value
is trust, graph pruning, and receipt reuse — not new proof power. The new proof power is
entirely in semantic-claim composition, and that one needs a **new producer fact**, not
just new plumbing.

Corroborating measurement — across all 418 rows, `demandCountsByFamily` never once
contains `dependency-contract` (the `main.rs:2060` label for
`ProofFamily::AcceptedDependencyComposition`), and `refusalCountsByFamily` is empty on
every row. The family is emitted in **zero** of 418 rows.

---

## 1. Census of the 25 not-attempted rows

### 1.1 What "not attempted" actually means

It is **not** a generation failure class. It is one runner gate,
`scripts/ecosystem-benchmark/run.mjs:1113-1119`:

```js
if (
  attemptCertification &&
  (result.class === "success" ||
    (result.dependencyPlan?.complete === true &&
      (result.dependencyPlan?.roots?.length ?? 0) > 0))
) {
  certificationQueue.push(item);
}
```

A row is never certified exactly when generation was not fully successful **and** the
dependency planner could not produce a complete graph. (`certificationAttempt` is
*omitted*, not `null`, in the JSON — `run.mjs:486` — which is a live trap for any
analysis script written in JS rather than jq.)

So 16 of the 25 are `partial-success` rows that produced a usable contract and were
skipped by policy; 9 hard-failed generation.

### 1.2 Normalized refusal taxonomy across all 25 rows

Occurrences (artifact-case refusals, not distinct rows):

| n | Refusal |
|---|---|
| 61 | `contract emission batch target N names source outside its configured project: <path>` |
| 60 | `emit package contract: entry file <path> has no runtime ESM exports` |
| 37 | `resolved target <path> is not a file` |
| 2 | `emit package contract: entry file <path> exports "X", whose runtime kind no closed type answers (Unknown, Unknown); publishing kind "value" would certify it invokes no caller-supplied callback` |
| 2 | `accepted dependency virtual:solidbase/components has no exact runtime binding for export mdxComponents` |
| 2 | `accepted dependency @tanstack/ai-solid has no exact runtime binding for export useChat` |
| 2 | `./virtual-solid-manifest selects no active package-export condition` |
| 2 | `./boundary-modules selects no active package-export condition` |
| 2 | `no declaration target exists for <path>` |
| 1 | `wildcard export branch "X" has no published target` |
| 1 | `local closure module ./types.js from <path> was not found` |
| 1 | `local closure module ../node_modules/solid-js/types/reactive/signal.js from <path> was not found` |
| 1 | `contract identity does not match the resolved import: resolved artifact has no exact runtime/declaration binding for export "Anchor"` |

### 1.3 Per-row census, tagged

Bucket counts below are **distinct refusal reasons** per row (a row's `refusedCases` can
be larger — the same reason across several artifact cases).

| # | Row | Class | Distinct refusal buckets | Tag |
|---|---|---|---|---|
| 1 | `@kobalte/core@0.13.13` s1 | partial-success | no-esm-exports ×52, identity-mismatch ×1 | different-owner: artifact-case enumeration |
| 2 | `@kobalte/solidbase@0.6.13` s1 | partial-success | outside-project ×56, target-not-a-file ×1, dep-binding ×1 | different-owner: contract-emission project scoping |
| 3 | `@kobalte/themes@0.0.1-next.0` s1 | unavailable-published-target | target-not-a-file | correct refusal |
| 4 | `@kobalte/utils@2.0.0-alpha.0` s2 | partial-success | no-esm-exports (`src/types.ts`) | different-owner: artifact-case enumeration |
| 5 | `@solid-devtools/babel-plugin@0.3.1` s1 | no-exported-surface | no-esm-exports (CJS) | correct refusal |
| 6 | `@solid-devtools/debugger@0.28.1` s1 | partial-success | target-not-a-file ×27, wildcard-no-target, no-declaration-target, **carried-value-kind** | correct refusal (publisher-defect dominant) |
| 7 | `@solid-devtools/ext-adapter@0.17.0` s1 | no-exported-surface | no-esm-exports (CJS) | correct refusal |
| 8 | `@solid-devtools/locator@0.16.7` s1 | partial-success | **carried-value-kind (sole)** | **unlocked-by-composition** |
| 9 | `@solid-devtools/shared@0.20.0` s1 | partial-success | no-esm-exports ×2, no-declaration-target | correct refusal |
| 10–11 | `@solid-primitives/animation@1.0.0-next.1` s2 ×2 | unavailable-published-target | target-not-a-file | correct refusal |
| 12 | `@solid-primitives/composites@1.1.1` s1 | unavailable-published-target | target-not-a-file (`.cjs`) | correct refusal |
| 13 | `@solid-primitives/context@0.3.2` s1 | missing-closure-module | missing-closure-module | correct refusal |
| 14–15 | `@solid-primitives/controlled-props@1.0.0-next.3` s2 ×2 | partial-success | target-not-a-file (`dist/index.jsx`) | correct refusal |
| 16–17 | `@solid-primitives/virtual@1.0.0-next.4` s2 ×2 | partial-success | target-not-a-file (`dist/index.jsx`) | correct refusal |
| 18 | `@solid-primitives/workers@0.4.3` s1 | missing-closure-module | missing-closure-module | correct refusal |
| 19 | `@solidjs/diagnostics@2.0.0-rc.3` s2 | partial-success | no-esm-exports (`dist/vitest.js`) | different-owner: artifact-case enumeration |
| 20 | `@solidjs/h@2.0.0-rc.3` s2 | partial-success | outside-project ×2 | different-owner: contract-emission project scoping |
| 21 | `@solidjs/image@0.1.0` s1 | partial-success | outside-project (`env.d.ts`) | different-owner: contract-emission project scoping |
| 22 | `@solidjs/universal@2.0.0-rc.3` s2 | partial-success | outside-project ×2 | different-owner: contract-emission project scoping |
| 23–24 | `@solidjs/vite-plugin@3.0.0-next.34` s2 ×2 | partial-success | no-active-condition ×2 | correct refusal |
| 25 | `@tanstack/ai-solid-ui@0.7.20` s1 | dependency-contract-obligation | **dep-binding (sole)** | **unlocked-by-composition** (blocked, see 1.5) |

**Group totals: unlocked-by-composition 2 · different-owner 7 · correct refusal 16.**

Different-owner breakdown:
- **contract-emission project scoping** (4 rows: 2, 20, 21, 22) — owner is
  `packages/cli` generation. A `.d.ts` the package ships outside its own tsconfig
  `include` refuses the batch. No dependency contract is involved.
- **artifact-case enumeration** (3 rows: 1, 4, 19) — owner is the probe/entrypoint
  selection that turned `src/**/*.test.tsx`, `src/types.ts` and `dist/vitest.js` into
  certifiable artifact cases. `@kobalte/core` is the extreme case: 52 of its 53 refusals
  are its own test sources.

### 1.4 Reproductions

Two representatives reproduced with the runner (`--keep-temp`), both bit-identical to
the recorded row.

**`@tanstack/ai-solid-ui@0.7.20|solid1|only`** — exact obligation, both artifact cases:

```
accepted dependency @tanstack/ai-solid has no exact runtime binding for export useChat
```

Emitted by `packages/cli/scripts/artifact-resolution.mjs:1294-1299`:

```js
if (!result) {
  fail(
    "accepted-dependency-binding",
    `accepted dependency ${externalDirect.specifier} has no exact ${axis} binding for export ${externalDirect.name}`
  );
}
```

where `result = acceptedExternalBinding(acceptedDependencies, specifier, name, axis)`,
i.e. `acceptedDependencies[specifier]?.exports?.[name]?.[axis]`. **An accepted contract
for the dependency discharges this directly** — the lookup already exists.

**`@solid-devtools/locator@0.16.7|solid1|only`** — sole obligation:

```
emit package contract: entry file <package-root>/dist/index.js exports
"addClickInterceptor", whose runtime kind no closed type answers (Unknown, Unknown);
publishing kind "value" would certify it invokes no caller-supplied callback
```

This is the **carried-value-kind** shape. `addClickInterceptor` is destructured from
`var exported = createInternalRoot(() => { … return { addClickInterceptor, … } })`, and
`createInternalRoot` comes from `@solid-devtools/debugger`. The kind is `Unknown` because
the checker cannot see through the dependency call. A contract for `@solid-devtools/debugger`
saying "`createInternalRoot` returns the return value of argument 0" would carry the object
literal's property kinds through — exactly the claim the `carried-value-kind` fixture
asserts, and exactly the fixture `docs/precision-backlog.md:9154-9165` records as the
unregistered exception because `contract generate` never loads the catalog.

### 1.5 Why both composition-shaped rows are blocked anyway

- **Row 8 (`locator`)**: its dependency `@solid-devtools/debugger@0.28.1` is itself in
  the not-attempted set (row 6, 30 refusals, publisher-defect dominant). No receipt can
  exist for it. Chain depth 2 with a broken link.
- **Row 25 (`ai-solid-ui`)**: three independent blocks.
  1. `@tanstack/ai-solid@0.19.1` is itself `dependency-contract-obligation / refused`
     (it needs `@tanstack/ai-client@0.28.0`, which is not a corpus package at all).
  2. **Version mismatch**: the installed dependency here is `@tanstack/ai-solid@0.19.4`,
     not the `0.19.1` the corpus probes. An exact-version rule must refuse this.
  3. The graph planner blew its budget: `dependencyPlan.status = "resource-refusal"`,
     `complete = false`, 14 cycles, and leaf kinds
     `{node-budget: 24, module-loading-frontier: 390, authenticated-receipt-unavailable: 420,
     dependency-identity: 98, artifact-resolution: 3}` against `maxNodes: 512`.
     This is the one place where composition has *unique* leverage: an accepted receipt
     collapses a whole subtree to one node, where the proposal lane must re-plan it.

---

## 2. The `until` row and the semantic-claim gap

### 2.1 Reproduction

`@solid-primitives/until@0.1.1|solid1|only` reproduced with an identical demand digest
(`sha256:18a2f9abace0ffd926890bd8e8de01e24f495cb6c985dab6f343dad8e225e1e7`):

```
policy-2 proof finalization failed: Type Facts certification failed during live
export-value verification: Type Facts demand sha256:18a2f9ab… is locally open:
callable-path (artifact-case:d244213…:until): callback parameter has no exact
direct-call or resolved-argument flow
```

Generation succeeded (`class: success`). Its proposal (`…proposal.json`) has exactly one
positive operation — `until:operation:callback-0`, i.e. *"until invokes argument 0"* —
and `closureCandidates: []`, with **15 unresolved claims** covering every claim domain
(`cleanups`, `creates`, `throws`, `writes`, `disposals`, `invalidates`, `reads`,
`returns`, `callbacks`, `guard-partition`, and five `owner-*` axes on `callback-0`).
Those 15 are open because `@solid-primitives/rootless` is an unaccepted external
dependency, which raises a `ClosureHazardKind::UnacceptedExternalDependency` that opens
every domain. So the missing edge degrades the contract *across the board*, not just at
one demand.

### 2.2 The source, and the exact premise that is missing

```js
// node_modules/@solid-primitives/until/dist/index.js
import { createBranch } from "@solid-primitives/rootless";
import { createComputed, createMemo, onCleanup } from "solid-js";
var until = (condition) => createBranch((dispose) => {
  const memo = createMemo(condition);          // ← argument-0 flow into a dialect primitive
  …
});
```

`condition` **does** flow into `createMemo` at argument 0 with arity 1, and
`solid_dialect::unambiguous_callback_argument("createMemo", 0, 1)` is `true`
(pinned at `rust/crates/solid-dialect/src/lib.rs:2505`). The demand still fails, because
of the executed-ness filter in
`rust/crates/solid-facts-backend/src/contract_certification/type_facts.rs:2356-2373`:

```rust
fn implementation_call_is_executed(
    implementation: &typefacts::ExportImplementationTranscript,
    call: &typefacts::ImplementationCall,
) -> bool {
    if call.reach != Reachability::Reachable { return false; }
    if !call.captured { return true; }
    implementation.control_flow.as_ref().is_some_and(|flow| {
        flow.returns.iter()
            .filter(|site| site.reach == Reachability::Reachable)
            .flat_map(|site| site.carried_callables.iter())
            .any(|carried| location_contains(carried, &call.location))
    })
}
```

Its own doc comment states the premise (`type_facts.rs:2341-2348`):

> "A call *inside a nested callable* is executed only if something invokes that callable,
> and the one such thing the census proves is the value the export returns."

The arrow holding `createMemo(condition)` is **not** returned by `until` — it is passed as
**argument 0 to `createBranch`**. So `captured == true`, no `carried_callables` range
contains it, the call is filtered out, and `require_parameter_flow` (`type_facts.rs:2258-2288`)
finds nothing and refuses.

**The missing premise is exactly: "the callable passed as argument N to this call is
executed, because dependency D's contract says D's export E invokes argument N."**

There is a second, adjacent hard-coding worth naming — `type_facts.rs:2311-2313`, inside
`require_parameter_callback_flow`:

```rust
if call.target.is_empty() || call.target_module.as_ref() != "solid-js" {
    continue;
}
```

Only the literal module `"solid-js"` may supply an "invokes argument N" claim today. That
is the string a composed dependency claim would have to displace.

### 2.3 The claim shape already exists — and rootless already publishes it

`benchmarks/package-contract-v2/phase14/solid-v1-authority/rootless-root-default.json`,
package `@solid-primitives/rootless@1.5.4`, export `createBranch`:

```json
"call": {
  "callbacks": [ { "from": { "arg": 0, "path": [] }, "operation": "callback-0" } ],
  "operations": [
    { "id": "callback-0", "kind": "invoke",
      "at": { "event": "call", "schedule": "same-stack" },
      "count": { "min": 0, "max": "many", "scope": "call" },
      "owner": { "requires": "required", "cleanup": "supported", "children": "allowed", … },
      "tracking": "untracked", "trigger": { "event": "call" } },
    { "id": "return", "kind": "return", "output": "unknown", … }
  ],
  "closed": ["callbacks", "reads", "creates", "returns"]
}
```

That **is** "createBranch invokes argument 0". Backed by the Rust model
(`rust/crates/solid-reactive-ir/src/contract_semantics.rs:802-813, 863-883`):
`CallClaims { callbacks: KnowledgeSet<CallbackInvocation>, reads, writes, creates,
invalidates, throws, returns, cleanups, disposals }`, with
`CallbackInvocation { from: ValueSource, operation: OperationId }` and
`ValueSource::Parameter { index: u16, path: Vec<String> }`; `OperationKind::Invoke`
exists. Schema-visible at `schema/solid-reactivity.schema.json:187, 253, 274`.

And critically: **`@solid-primitives/rootless@1.5.4` is `class: success`,
`certificationAttempt.status: "certified"`** in the ecosystem report. Both halves of the
`until` composition exist and are green. Only the join is missing.

### 2.4 The join is missing — verbatim

`ProofFamily::AcceptedDependencyComposition` exists and is fully wired at the *authority*
level. It has exactly one producer and one consumer.

**Producer** — `rust/crates/solid-facts-backend/src/contract_certification.rs:652-666`,
fed from `verified_closure.manifest().dependencies`. The predicate is purely
module-resolution: an edge exists iff a replayed closure module resolves External **and**
the caller supplied exactly one matching `AcceptedDependencyEdge`
(`module_closure.rs:278-298`). Nothing about what the callee does. Expansion at
`certification.rs:456-477` is a cartesian product: `1 + |closure_candidates|` demands per
edge, with subjects `DependencyArtifact { dependency }` and
`DependencyClosure { dependency, parent, semantic_claim_id }`.

**Consumer** — `VerifiedDependencyComposition::authenticate`,
`contract_certification/dependencies.rs:1759-1881`. The witness is built entirely from
identity digests (`dependencies.rs:1830-1842`):

```rust
witnesses.push(WitnessBinding::new(
    ProofWitnessVariant::AcceptedDependencyComposition,
    requirement.demand_id(),
    evidence_root,
    vec![
        format!("graph:{graph_root}"),
        format!("parent-case:{}", parent.selected_artifact_case_id()),
        format!("dependency-node:{}", dependency.digest()),
        format!("dependency-receipt:{}", receipt.receipt_digest()),
    ],
));
```

**`semantic_claim_id` and the parent export name are hash inputs only.** Nothing looks up
what the dependency claims about that claim path. A `DependencyClosure` demand for a
`callbacks` claim is discharged by exactly the same bytes as one for `throws`. No code
path anywhere reads a dependency's `CallClaims`.

`ProofAuthority::AcceptedDependencyContract` is declarative in the production enum
(`certification.rs:1124`, private, `Serialize`-only, contributing to the policy digest at
`certification.rs:1826-1830`). The enforced version lives in `proof.rs:82`, checked at
`proof.rs:347-354` inside `replay_proof_rule`, **which has no production callers**.

### 2.5 And a producer fact is missing too

To say "the callable at bytes X..Y is argument N of this call", the census must record it.
It does not. `rust/crates/typefacts/src/invocation.rs:337-354`:

```rust
pub struct ImplementationCall {
    pub location: Location,
    pub reach: Reachability,
    pub target: Arc<str>,
    pub target_name: Arc<str>,
    pub target_module: Arc<str>,
    pub declaration: Option<ResolvedDeclaration>,
    pub callee_parameter: Option<ParameterValueSource>,
    pub argument_parameters: Vec<Option<ParameterValueSource>>,
    pub captured: bool,
}
```

There is no `argument_callables`. The symmetric fact exists only on the *return* side —
`ReturnSite.carried_callables: Vec<Location>` (`invocation.rs:462-475`), whose doc comment
is the template:

> "Exact source ranges of the callables this returned value provably carries. A call
> inside a nested callable is reachable through the returned value exactly when its
> location lies within one of these ranges; an empty list is never proof that nothing is
> carried."

**Semantic-claim composition therefore requires a TypeFacts protocol change and a rebuild
of `bin/solid-typefacts`** (`scripts/build-typefacts.sh`), not merely Rust-side plumbing.
That is the single largest cost item in this study.

---

## 3. Existing machinery — what already exists, and where the flow breaks

### 3.1 Two lanes, strictly separated

| | **Accepted lane** | **Proposal lane** |
|---|---|---|
| Flag | `--accepted-contracts` (+ `--receipt-trust-configuration`) | `--proposal-dependencies` |
| Catalog format | `solid-checker-accepted-contract-catalog` v2 | `solid-checker-proposal-dependency-catalog` v1 |
| Authenticated | Yes — Ed25519 receipt, signed bindings, host trust store | **No** |
| Built by | `contract certify` (publishes), host supplies | `contract certify` internally, per graph node |
| Mutually exclusive | `main.rs:2505-2507`; JS mirror `generate-package-contract.mjs:945-954` | ditto |

The ecosystem runner uses **only the proposal lane**. Its `attemptCertification` hook
(`run.mjs:1474-1523`) spawns `contract certify` with `--package-root --integrity --catalog
--issuer-configuration --trust-configuration-output --audit-output [--proposal-refusal-audit]
[--entrypoint …]` and **never** `--accepted-contracts`. The `@solidjs/signals` graph nodes
in the intersection-observer rows are certified through `mergeProposalDependencies`
(`certify-contract.mjs:731-781`), which content-addresses each dependency's *proposal* and
hands it to the parent's generation as
`{ packageName, artifactCase, acceptedContractDigest, exports }`.

That is why `AcceptedDependencyComposition` demands are emitted zero times: the accepted
lane is never engaged, so `ClosureManifest.dependencies` is always empty.

### 3.2 What the system can already prove about a dependency

1. **Artifact identity** — exact name/version/integrity/manifest digest, per-artifact-case
   runtime + declaration + closure digests (`artifact_resolution.rs:809-856`).
2. **Export bindings** — `dependency.verified_exports.bindings` gives
   `runtime_path/runtime_export/runtime_snapshot_root/span` (`export_bindings.rs:625-659`),
   re-verified against the dependency snapshot's bytes (`export_bindings.rs:272-286`).
3. **Receipt authenticity** — 23 signed bindings, policy digest, issuer kind/scope,
   revocation epoch, verifier build digest, Ed25519 `verify_strict`
   (`policy2_receipt.rs:711-841`; `dependencies.rs:1921-1988`).
4. **Full semantic claims** — the dependency's own `CallClaims`, including
   `callbacks[].from.arg` + `operations[].kind = "invoke"`, are *present in the document*
   and are decoded and normalized on load (`contract_interface.rs:744`).

### 3.3 Where a proven dependency claim fails to flow into the dependent

Three distinct breaks, in order of increasing cost:

- **Break A — the accepted lane is never engaged by any gate.**
  `contract generate` (the only driver `scripts/contract-corpus.mjs` runs) defaults
  `acceptedDependencies` to `{}`; `contract certify` supplies it, and no repository gate
  runs `certify` over a fixture. This is precisely what `docs/precision-backlog.md:9154-9165`
  records as the reason `carried-value-kind` stays unregistered: *"It is not dead: … The
  claim needs a certification gate, or a policy-2 reissue of the fixture's catalog, before
  it asserts anything again."*

- **Break B — the composition witness ignores the claim it names.**
  `dependencies.rs:1830-1842` (§2.4). The demand carries a `semantic_claim_id`; the witness
  hashes it and never resolves it. Fixing this is Rust-only.

- **Break C — the Type Facts witness path has no channel for a composed invoke claim.**
  `implementation_call_is_executed` (`type_facts.rs:2356-2373`) accepts exactly one
  executed-ness premise (carried by a return), and `require_parameter_callback_flow`
  hard-codes `target_module == "solid-js"` (`type_facts.rs:2311-2313`). Closing this needs
  the new producer fact from §2.5 **plus** a new argument to these functions carrying the
  composed claims.

---

## 4. Fail-closed rules any composition must obey

Drawn from what the code already enforces; a new feature must not weaken any of them.

1. **Exact version, integrity, and digest — never a version string alone.**
   `check-contract-pins.mjs:13-14`: *"an absent integrity is a failure here, not a skip. A
   pin that cannot be falsified is not a pin."* `verifyPin` (`:253-277`) requires name
   identity, exact version, mandatory integrity, and live registry agreement; an array
   answer (range/tag) is rejected outright (`:60-61`).
2. **A refused/uncertifiable dependency contributes nothing — and its absence is loud.**
   `obsolete-policy1` entries go into a *disjoint* quarantine map
   (`contract_interface.rs:450-454`, `consumer.rs:88-93`), pruned against the accepted map on
   every mutation (`consumer.rs:139,167,196-197`), and a lookup returns
   `SemanticQueryError::MissingImport` which becomes an explicit finding
   (`contracts.rs:739-753`), not silence. `with_fallback` uses `or_insert`
   (`consumer.rs:171-199`) so a built-in can fill only a genuinely absent key — it can never
   launder a refused project entry.
3. **A project may not nominate its own issuer.** `contract_interface.rs:431-433, 457` —
   trust bytes are a separate host-configured input; without them the load is
   `ReceiptAuthenticationRequired`.
4. **Version mismatch refuses, fatally, for the whole run.** Two independent gates:
   `validate_package_identity` (`artifact_resolution.rs:809-843`) compares name/version/integrity,
   and `policy2_resolved_import_root` (`policy2_receipt.rs:649-666`) hashes the entire
   `ResolvedImport` into the signature, so editing the version to pass the first breaks the
   second. No call site has a `continue`-on-error branch.
5. **Claims are never assumed transitively.** Each dependency edge needs its own receipt;
   `authenticate_dependency_receipt` requires the receipt's policy digest to equal the
   parent's, one verifier build across the whole graph
   (`dependencies.rs:1814-1822 → VerifierBuildDisagreement`), and a parent-transplant check
   (`dependencies.rs:1883-1891`). A dependency-of-a-dependency claim must arrive with its own
   proof, not by inheritance.
6. **Total-or-refuse witness coverage.** `RECEIPT_WITNESS_FAMILIES` includes this family
   (`certification.rs:1505-1523`); a demand with no witness is `MissingWitness`. There is no
   `inapplicable` variant — `certification.rs:1348-1349`: *"There is intentionally no
   `inapplicable`, `other`, or caller-defined variant."* A composition that cannot prove a
   claim must open the demand, never skip it.
7. **`count.min` is not flow.** The rootless claim is `count: {min: 0, max: "many"}`. That
   supports a *flow/executed-ness* premise (the callback may be invoked) but must **not** be
   read as an `operation-cardinality` at-least-once premise. Keep these two demand families
   distinct when composing.
8. **A partially-understood trust artifact is discarded whole**
   (`check-contract-pins.mjs:158-177`), and derived/cached evidence may serve an *agreeing*
   answer but never produce a verdict of its own (`:202-220`).

**Known residual hole to name, not to widen:** nothing at accepted-catalog load time
re-hashes the bytes currently in `node_modules`. The comparison is document ↔ the catalog's
own `ResolvedImport`. Freshness is carried by whoever produced the catalog plus the daemon's
member hashing (`daemon.rs:737-744`). The user-facing stale wording exists
(`projection.rs:399-433`) but `package_contract_finding` has **no caller** — the `Stale`,
`StaleBundled` and `IntegrityMismatch` issue kinds are currently unreachable.

---

## 5. Staged implementation sketch

### Stage 0 — make the accepted lane falsifiable (prerequisite, no new proof power)

Add a repository gate that runs `contract certify` over a fixture and re-registers
`fixtures/package-contracts/carried-value-kind` against it. Today the only driver
`scripts/contract-corpus.mjs` runs is `contract generate`, which hardcodes
`acceptedDependencies = {}`, so every claim in this study is asserted by nothing.

- **Acceptance rows**: `carried-value-kind` registered in
  `fixtures/package-contracts/corpus.json` with a reviewed snapshot, asserting the
  laundering claim rather than an `accepted-dependency-binding` refusal.
- **Must-not-clear**: registering it under `contract generate` (which would pin the
  wrong-driver refusal) must fail the gate.
- **Ecosystem rows unlocked: 0.** This buys falsifiability, nothing else.

### Stage 1 — authenticated binding composition across ONE already-certified dependency

Route an already-issued receipt for dependency `D` into the parent's
`acceptedDependencies` through the authenticated `--accepted-contracts` lane, so
`acceptedExternalBinding` (`artifact-resolution.mjs:1288`) resolves. This is *plumbing plus
trust*, not new proof: the lookup, the guardrails
(`generate-package-contract.mjs:933-954`), and the identity checks all exist.

- **Acceptance row**: `@solid-primitives/form@1.0.0-next.2` (solid2, `floor` and `head`).
  Exactly one refusal — `accepted dependency @solid-primitives/a11y has no exact runtime
  binding for export FormControlContext` — and `@solid-primitives/a11y@1.0.0-next.3` is
  `certified` in-corpus. Cleanest single-edge, single-refusal, dependency-green row in the
  whole corpus.
- **Second acceptance row (cascade)**: `corvu@0.7.2` — 9 distinct dep-binding refusals,
  all 9 dependencies certified in-corpus.
- **Must-not-clear traps**:
  - `@corvu-next/accordion@0.1.5` — `@corvu-next/disclosure@0.1.4` is **not** certified.
    A REFUSED/absent dependency must contribute nothing and the row must stay refused.
  - `@tanstack/ai-solid-ui@0.7.20` — installed dep is `@tanstack/ai-solid@0.19.4` while the
    corpus row is `0.19.1`. **Version mismatch must refuse**, via both
    `validate_package_identity` and `resolvedImportRoot`.
  - A receipt whose `policy_digest` differs from the parent's must refuse
    (`authenticate_dependency_receipt`).
  - A hand-edited catalog entry with a corrected `packageVersion` must fail the signature
    binding, not pass identity.
- **Ecosystem rows unlocked: 0 net new certifications.** All 6 rows whose refusals are
  purely dep-binding *and* whose dependencies are all certified
  (`@corvu/accordion`, `@corvu/drawer`, `@corvu/popover`, `@solid-primitives/form` ×2,
  `corvu`) **already reach `status: certified` through the private proposal-graph lane**.
  The measurable gains are: authenticated rather than unauthenticated evidence; graph
  pruning (directly relevant to `@tanstack/ai-solid-ui`'s 512-node budget exhaustion); and
  receipt reuse across runs.

### Stage 2 — read the dependency's claim in the composition witness

Close Break B. Make `VerifiedDependencyComposition::authenticate` resolve
`requirement.semantic_claim_id()` against the dependency contract's decoded `CallClaims`
and refuse when the named claim is absent, instead of hashing the id and moving on.
Rust-only; no producer change.

- **Acceptance rows**: a fixture pair — a dependency whose contract *does* carry the named
  claim (composition witnessed) and one that does not (demand stays open, row refuses).
- **Must-not-clear**: a `DependencyClosure` demand for `callbacks` must no longer be
  discharged by the same bytes as one for `throws`. Pin this as a regression test — it is
  the exact defect today.
- **Ecosystem rows unlocked: 0.** This is a soundness repair, and it may *reduce* the
  certified count if any current pass depended on the vacuous witness.

### Stage 3 — semantic-claim (invoke) composition: the `until` acceptance

Close Break C. Three parts, in order:

1. **Producer**: add `argument_callables: Vec<Option<Location>>` to
   `typefacts::ImplementationCall`, mirroring `ReturnSite.carried_callables`. Protocol
   version bump; rebuild `bin/solid-typefacts` via `scripts/build-typefacts.sh`; the
   `.buildinfo` stamp change escalates `verify-delta` to full `make verify`.
2. **Composed claim channel**: thread the accepted dependencies' `CallClaims` into
   `require_parameter_flow` / `require_parameter_callback_flow` /
   `implementation_call_is_executed`, so that a callable at `argument_callables[N]` of a
   call whose target module is dependency `D` and whose export `E` has
   `callbacks[].from.arg == N` with an `invoke` operation counts as executed. Replace the
   `target_module != "solid-js"` literal (`type_facts.rs:2311-2313`) with "dialect **or** an
   authenticated composed claim".
3. **Acceptance row**: `@solid-primitives/until@0.1.1|solid1` certifies. Both halves are
   green today — `@solid-primitives/rootless@1.5.4` is `certified` and publishes
   `createBranch: callbacks[{from:{arg:0}}, operation callback-0 kind invoke]`.
- **Must-not-clear traps**:
  - A dependency whose contract has `closed: ["callbacks"]` but **no** entry for argument N
    must leave the demand open (absence is not proof of non-invocation).
  - A `count: {min: 0}` invoke claim must satisfy a `callable-path` flow demand but **not**
    an `operation-cardinality` at-least-once demand.
  - An invoke claim from a dependency with a REFUSED receipt must contribute nothing.
  - A claim for `arg: 1` must not clear a demand about `arg: 0`.
  - `until`'s 15 currently-open claim domains must not all silently close: the
    `UnacceptedExternalDependency` hazard clearing is a *separate* consequence of Stage 1
    and each domain still needs its own proof.
- **Ecosystem rows unlocked: 1 confirmed (`until`)**, plus an unquantified share of the
  ~28 refused rows whose open demand is `callable-path` / `operation-cardinality` /
  `operation-reachability` / `argument-binding` — but only those whose blocking flow crosses
  a package boundary into a *certifiable* dependency. I did not measure that split, and it
  should not be claimed without measuring it.

---

## 6. Honest estimate

Of the **25 + 1** rows in scope:

| Stage | Rows unlocked (25 not-attempted) | The `until` row | Notes |
|---|---|---|---|
| 0 | 0 | no | Falsifiability only |
| 1 | **0** | no | 0 net new corpus-wide too; the proposal lane already clears all 6 dep-green rows |
| 2 | 0 | no | Soundness repair; may reduce the certified count |
| 3 | **0–1** | **yes (1)** | `locator` only if `@solid-devtools/debugger` first becomes certifiable |

**Total across all four stages: 1 of 26 rows confidently unlocked (`until`), with
`@solid-devtools/locator` a possible second behind a broken dependency chain.**

Of the 25 not-attempted rows: **16 are correct refusals** (published artifacts that are
absent, CJS-only, or declaration files referencing unpublished modules — no contract for any
dependency can conjure a missing file), **7 belong to other owners** (4 to `packages/cli`
contract-emission project scoping, 3 to artifact-case enumeration that turned test sources,
type-only modules and `dist/vitest.js` into certifiable cases), and **2 are
composition-shaped but blocked** — `@solid-devtools/locator` behind an uncertifiable
`@solid-devtools/debugger`, `@tanstack/ai-solid-ui` behind an uncertifiable *and*
version-mismatched `@tanstack/ai-solid` *and* a blown 512-node graph budget.

The largest single lever on the 25 is not composition at all: it is the runner's
`class === "success"` gate (`run.mjs:1113-1119`) plus artifact-case enumeration. Sixteen of
the 25 produced usable contracts and were skipped by policy; several were skipped over
refusals on their own test files.

### Remaining fail-closed / uncertifiable cases after all four stages

- Every `target-not-a-file`, `no-esm-exports`, `missing-closure-module` and
  `no-active-condition` row stays refused. These are publisher defects.
- `outside-project` rows stay refused until contract-emission project scoping changes.
- Any dependency that is itself uncertifiable still contributes nothing, by rule 2 of §4.
- The accepted-catalog freshness hole (§4, closing note) remains: no load-time re-digest of
  the installed tree, and `PackageContractIssueKind::{Stale, StaleBundled, IntegrityMismatch}`
  remain unreachable.
- `count.min: 0` invoke claims remain unable to discharge `operation-cardinality`.
- The share of the ~28 Type-Facts-refused rows that Stage 3 would reach is **unmeasured**.

### Method caveats

- All counts derive from the checked-in `benchmarks/ecosystem/report.json` (418 rows) plus
  three fresh single-probe reproductions. The full corpus was not re-run.
- `certificationAttempt` is omitted rather than `null` in that JSON; a JS `=== null` filter
  silently returns zero rows. Every count here uses `== null` or jq.
- "Certified in-corpus" means some row for that exact `name@version` reached
  `status: "certified"`. It is not the same as a receipt existing on disk.
