# Solid 2.0 rule catalog audit

> **Remediation status (2026-08-15, same branch):** every confirmed finding in this
> report has been fixed on this branch, in six commits following the audit commit.
> Mapping:
>
> - §1.1, §1.2, §2.1, §1.8-SC2003, §4-leaf-owner-example → *"Match the write-scope
>   and leaf-owner model to the rc.0 runtime"*
> - §1.3, §2.2, §4-inventory (async half) → *"Model the rc.0 async hydration options
>   and close the SSR client-source hole"* (new rule **SC5005**; probes showed
>   pending throws return after the declared first-flight window, so SC5001/SC5002
>   remain with conditional-truth messages and an opaque-options uncertifiable path)
> - §1.4, §1.5, §2.3, §2.4-SC7001, §3-SC4004, §4-SC9001-hint → *"Anchor the
>   API-shape rules on the probed rc.0 runtime"*
> - §1.6, §1.8-SC1004/SC1006, §2.4-SC1002/SC1004, §4-SC1002-page/span,
>   §3-SC1003 (via kind reclassification) → *"Prove props reactivity from callers
>   and sharpen the tracking rules"* (also resolved two declared upstream parity
>   deviations)
> - §1.7, §2.4-SC8019/keyed, §1.8-SC6001, §5-fixtures → *"Teach the JSX rules
>   parser scope and stop fabricating keyed accessors"* (`draggable={true}` was
>   probe-confirmed broken and is now flagged)
> - §2.5, plus `resolve()` and the SC1005 edges → *"Cover the server surface,
>   resolve(), and the uncalled-accessor edges"* (new rules **SC7005 SC7006 SC7007
>   SC2004**)
> - §3-SC8018/SC9011, §4-README-table/SC9005 → *"Document certification gates, the
>   rc pin, and convention-rule scope"*
>
> The catalog is now 43 Solid 2.0 rules. Remaining known boundaries are documented
> on the relevant rule pages rather than left silent: SC8020 svg-breakout and
> option-children refinements, `draggable={false}` on draggable-by-default
> elements, WeakMap/WeakSet and nested rich types in SC7007, named-function
> `untrack` callbacks, and dynamic-extent leaf-scope helpers. Upstream issues worth
> filing against solidjs/solid remain: RFC 01/08 claim writes in `untrack` are
> allowed (the rc.0 guard throws), and `ACTION_CALLED_IN_OWNED_SCOPE` is missing
> from the RFC 08 diagnostics table.

**Date:** 2026-08-15
**Audited:** the 38-rule Solid 2.0 catalog (`docs/rules/`, `rust/dialects/solid-v2/`, shared engine crates, fixtures) at commit `a74fc81`.
**Ground truth:** the current `solidjs/solid@next` `documentation/solid-2.0/` set (all 15 files, fetched 2026-08-15 — note it now includes `10-server-functions.md`, `11-server-components.md`, `12-ssr-http.md`), the pinned `solid-js@2.0.0-rc.0` npm tarball, and the `@solidjs/signals@2.0.0-rc.0` dev bundle it re-exports. Where docs and runtime disagreed, ~25 empirical Node probes against the dev bundle settled it.

**Confidence labels:** *confirmed* = backed by a runtime probe, runtime source, or code-reading pinned by the checker's own fixtures; *plausible* = inferred from docs/spec, runtime source not available or not probed.

---

## Verdict

**Are the rules perfect? No — but they are unusually good for a tool of this ambition.** The catalog maps every diagnostic in the official dev-diagnostics table, its deliberate runtime-only exclusions hold up, and large parts (keyed callback shapes, cleanup-return validation, the `refresh`/`affects` shape checks, the ownership rules, the `reactive-read-after-await` dominance guards) match the rc.0 runtime exactly. But the audit found **~10 confirmed false-positive classes** — several pinned by the checker's *own fixtures*, i.e. deliberate spec choices that deviate from the runtime, not implementation slips — **~8 confirmed false negatives**, and a systematic blind spot: the entire `loadingValue`/`seedLoadingValue`/`ssrSource` option surface that ships in the pinned rc.0 is invisible to the analysis (zero repo hits).

**"Only a few undeterminable cases"? Depends on the project.** The `SC9xxx` uncertifiable family is honestly designed — narrow triggers, real escape paths, tested fail-closed behavior. For a self-contained project depending only on `solid-js`/`@solidjs/web` at exactly `2.0.0-rc.0`, uncertifiable findings will indeed be few. But certification is adoption-heavy in practice: only two contracts ship bundled, so **every third-party Solid package** without a contract fails certification (SC9005), any solid-js version other than the exact pinned RC makes the **whole project** uncertifiable (silently skipped bundled contract → SC9005; documented in `docs/package-contracts.md` but not on the rule page), and the docs-canonical exported-action idiom (`affects(param, "key")` on a store record received as a parameter) is uncertifiable-by-default (SC9003).

---

## 1. Confirmed false positives (violations/warnings on runtime-legal code)

### 1.1 Writes and actions inside `createTrackedEffect` — SC2001, SC2002 *(confirmed, empirically probed)*
The rc.0 write guard exempts children-forbidden scopes: `!(context._config & CONFIG_CHILDREN_FORBIDDEN)` (signals `dev.js:3157`, refresh `:3319`, action `:4312-4400`), with the runtime's own comment "leaf imperative scopes (tracked effects, onSettled) stay legal" (`dev.js:4392`). Probes: `setSignal(2)` / `refresh(x)` / `action()` inside `createTrackedEffect` → **OK at runtime**. The checker models the callback as `Execution::Tracked` (`solid-dialect/src/solid_2.rs:422-432`) and fires SC2001/SC2002 — pinned by its own snapshots (`fixtures/reactive-ir/write-scope/App.tsx:51`, `owned-leaf-extended-invalid.tsx:30`). The finding text "Solid throws REACTIVE_WRITE_IN_OWNED_SCOPE here in dev" is factually false there.

### 1.2 Out-of-band `onSettled` treated as a leaf owner — SC3001, SC3002, SC3003 *(confirmed, probed)*
`onSettled` called from an event handler or other unowned scope runs its callback as a plain enqueued function, not a leaf owner (probes: `onCleanup`, `createMemo`, `flush()` inside it → no throw; the runtime docstring endorses the pattern, `dev.js:4855-4864`). The checker's detection is lexical (`cleanup.rs:73-89`) with no call-site owner gate, so it emits **errors** on code that at worst warns (or leaks) at runtime. SC4004 already owns the real out-of-band defect (returned cleanup).

### 1.3 `loadingValue` / `seedLoadingValue` computations — SC5001 (error!), SC5002, SC5003 *(confirmed)*
A declared node "is born committed … never suspends readers, never trips a `Loading` boundary" (RFC 05 §declared first paint; machinery present in rc.0 `dev.js:181, 464-553`). Async provenance in the checker inspects only argument 0 (`source_discovery.rs:508-517`); options are parsed only for `sync`/`ownedWrite` (`static_api.rs:104-110`); `loadingValue|seedLoadingValue|ssrSource` have zero occurrences repo-wide. Consequence: error-severity SC5001 "throws at runtime" on reads that cannot throw during the declared window, and SC5003 warns with a wrong-directional hint ("wrap in `<Loading>`") on computations whose entire point is to not need one.

### 1.4 `sync: true` on store-family constructors — SC7002 *(confirmed)*
`options.sync` reaches `CONFIG_SYNC` only via `computed()` and `createEffectNode`. `createProjectionInternal` rebuilds options with only `loadingValue`/`name` (`dev.js:5666-5672`), so `sync` is inert on `createStore(fn,…)`, `createProjection`, `createOptimisticStore` — and `sync` isn't even in their option types. The checker flags all three (snapshot offsets 304/375/500 in `reactive-ir__static-api.json`); `docs/semantic-inventory.md:12-14` makes the same untrue claim.

### 1.5 Docs-canonical `affects(state.user, "key")` — SC7003 *(confirmed)*
Store child proxies carry the `$TARGET` brand (trap at `dev.js:6610`); the official types, the runtime's own error text, and RFC 06 all recommend targeting nested records directly (`affects.d.ts:22-25`, `06-actions-optimistic.md:83`). The checker requires `ArgumentValueKind::Identifier` (`static_api.rs:168-185`), so member-expression targets are flagged as "wrapper, read value, or literal" — while the checker's *own fix hint* on the neighboring check recommends exactly this form (`static_api.rs:159`). Also flags `refresh(x, extra)` arity, which the runtime silently ignores.

### 1.6 Props over-approximation reported as proven violation — SC1001, SC1003, SC1007 *(confirmed at code level)*
Every proven component's props parameter is enrolled as signal-backed unconditionally, with no caller analysis (`source_discovery.rs:1452-1476`). The runtime strict-read warning fires only for signal-backed props; a component whose every call site passes literals is permanently correct. Three consequences:
- `function Card(props){ const t = props.title; … }` used only as `<Card title="Hello"/>` → SC1001 "violation".
- `onClick={props.onSave}` → SC1007 + SC1001 on the same expression (pinned in `shared-reactivity-v2/App.tsx:76`) — among the most idiomatic Solid patterns; freezing only occurs if a caller passes a reactive expression, which the checker never proves.
- Destructuring props inside an **event handler**, `onSettled`, or `untrack` callback → SC1003 **error**; all three read fresh values at call time and are legal (exemption is `TrackedJsx`-only, `static_rules.rs:373-381`).
Under the project's own charter these belong in the `uncertifiable` bucket (or need caller proof), not `violation`.

Additional SC1001 sub-case: module-scope store/member reads are reported (`execution_role.rs:383-387` + `lib.rs:93`) though the runtime installs strict-read contexts only in component/effect bodies — a deliberate module-scope snapshot is legal, undiagnosed Solid. The message even mislabels the context "rendering function" (`findings.rs:238-244`).

### 1.7 Nested HTML lists (and other scope-boundary cases) — SC8020 *(confirmed in code)*
`invalid_html_ancestor` (`static_rules.rs:267-290`) scans **all** intrinsic ancestors with no WHATWG scope-boundary stops. `li` search doesn't stop at `ul`/`ol`, so every `<ul><li><ul><li>…` — the most common list markup on the web — is an **error**. Same class: nested `<dl>`, `button` reached through a `td` scope marker, `<p><button><div/></button></p>` (p not in button scope → parser preserves the tree). This violates the rule's own "only when the parser changes the tree" policy.

### 1.8 Smaller confirmed FPs
- **SC1004:** ternary-return containment matches *any* conditional test inside the returned expression (`static_rules.rs:580-585`) → spurious second finding on a tracked ternary in a JSX attribute under a ternary return.
- **SC1006:** a derived helper bound and called inside a tracked compute or event handler is flagged, though reads there track (or are legitimately fresh-at-event-time); the module's own call-site exemption logic never applies when binding and call share the body.
- **SC2003:** writing through the original store proxy *inside its own setter draft* is runtime-legal (write-enabled during the setter; probed) but flagged — no setter-scope exemption in `shared_reactivity.rs:680-749`.
- **SC6001:** RFC 07 says the directive apply phase "should not" create primitives — a convention, no runtime diagnostic exists. The checker ships it as error/violation and flags value-form `createSignal`, which needs no owner and misbehaves in no way; internally inconsistent with SC4001 treating the equivalent unowned-effect leak as a warning.

---

## 2. Confirmed false negatives

### 2.1 `untrack(() => setCount(...))` in an owned scope *(confirmed, probed — upstream doc contradiction)*
The rc.0 guard keys on **owner context, not tracking**: `untrack` clears `tracking` but not `context` (`dev.js:2928-2942`), so writes/refresh inside `untrack` within a memo/component **throw** `REACTIVE_WRITE_IN_OWNED_SCOPE` (probed three ways). Both the official RFC 01 text ("untrack blocks" allowed) and the checker's rule page repeat the opposite; the checker models untrack callbacks as deferred and stays silent (`write-scope/App.tsx:49` produces no finding). This is simultaneously a checker FN and an **upstream docs-vs-runtime bug worth reporting to solidjs/solid**.

### 2.2 Bare `ssrSource: "client"` read outside a `Loading` boundary *(confirmed)*
rc.0's server runtime hard-errors: `throw new Error('ssrSource: "client" read during SSR outside a <Loading> boundary …')` (`dist/server.js:202`; RFC 05 "bare client sources must sit under a boundary"). The checker has zero ssrSource awareness; worse, a **synchronous** client-source compute has no async provenance, so no SC5xxx rule can ever see it — a guaranteed server render error with total silence. Checkable today with the exact Loading-dominance machinery SC5003 already runs.

### 2.3 `refresh(plainStore)` *(confirmed)*
Only projection-family stores are `Refreshable`; a plain `createStore(obj)` store has no refresh node, and `refresh(s)` throws `INVALID_REFRESH_TARGET` in dev (`dev.js:5889` vs `:5647`). The checker accepts any resolved Store binding (`ReactiveSourceKind` = `Accessor|Store` only) and records the call as a valid ReactiveWrite.

### 2.4 Others (confirmed unless noted)
- **SC1002:** only accessor **calls** after await are checked (`static_rules.rs:448-455`); member reads (store paths, props) after await are missed — the rule page explicitly claims them.
- **SC1004:** `return props.user && <Profile/>` and `switch`-statement returns are the same frozen-branch defect and are invisible (only `ConditionalExpression`/`if` are modeled).
- **SC7001:** `createEffect(fn, null)` or a non-function second argument crashes at runtime (`null.effect`) but only literal-`undefined`/absent is flagged.
- **SC3001/SC3003:** dynamic-extent misses — `onCleanup`/`flush` in a helper called synchronously from a leaf callback throws at runtime, checker is lexical-only (standard static limitation, worth documenting).
- **SC8019:** `draggable={true}` plausibly serializes the same presence-only attribute (→ `auto`) as the shorthand the rule flags; the fixture blesses it as correct (*plausible* — `@solidjs/web` dist not audited).
- **Dynamic-boolean `keyed={cond()}`:** RFC 03 warns against it; no rule advises, and worse the dialect maps any non-literal `keyed` to `CustomKey` and claims **both** `<For>` params as accessors (`solid-dialect/src/lib.rs:288-297`, `solid_2.rs:328-345`) — fabricating an accessor for what is a raw value when the boolean is truthy (the v1 dialect's own comments refuse to do exactly this).

### 2.5 Catalog-level gaps (the pinned `@solidjs/web` surface the checker itself contracts)
The reviewed contract `solid-dialect/contracts/solid-v2/solidjs-web.json` attests `httpStatus`, `httpHeader`, server-function machinery, etc. shipped in the pinned package — yet nothing constrains them:
- `httpHeader`/`httpStatus` below a `Loading` boundary that settles post-flush are **committed no-ops by contract** (RFC 12) — a silent header drop, statically checkable with existing boundary dominance.
- Module-level `"use server"` with a wrapped export is silently dropped from the client build; RFC 10 says "Minimum: a diagnostic". No rule.
- Rich-argument transport (Date/Map/Set/cyclic args to a server function) throws unless `enableRichArguments()` is imported — invisible to TS (the declared signature accepts a `Date`); checkable via type facts + import presence.
- `resolve(fn)` inside a reactive scope is documented forbidden (RFC 05); the contract only marks the callback deferred; no rule (*moderate confidence* — enforcement lives in `@solidjs/signals`).

---

## 3. Severity & classification mismatches

| Rule | Issue |
| --- | --- |
| SC4004 no-owner-settled-cleanup | Runtime `SETTLED_CLEANUP_UNOWNED` is a dev **error** (throws); catalog ships **warning**. The checker's own `runtime_mirrored_severities_match_solid_two` test (`rules.rs:291-302`) mirrors SC5001–SC5003 but conspicuously omits SC4004; the rule page describes only the prod behavior and never mentions the dev throw. Semantics otherwise match the runtime precisely. |
| SC1003 component-props-destructure | Runtime analog is a **warn** (`STRICT_READ_UNTRACKED`); checker escalates to error with no documented adoption-policy exception (unlike SC1004, which documents its escalation). |
| SC8018 prefer-component-syntax | A convention rule (calling a lowercase JSX-returning function in JSX works fine at runtime) shipped as kind=`violation` — contradicts the charter "violation = proven runtime misbehavior". Warning severity limits damage. |
| SC6001 primitive-in-directive-application | Convention ("should not", RFC 07) shipped as error/violation; see §1.8. |
| SC9011 reactive-source-uncaptured | Labeled "advisory warning", but status computation looks only at kind, so it **blocks `--certify`** exactly like the SC9xxx errors — neither README nor page says so. Behavior is consistent; labeling invites the wrong conclusion. |
| SC9005 package-contract-missing | The rc-pin consequence (any solid-js other than exactly 2.0.0-rc.0 → bundled contract silently skipped → whole project uncertifiable) is documented in `docs/package-contracts.md` but **not on the rule page** where users hitting it land. Note also solid-js declares `"@solidjs/signals": "^2.0.0-rc.0"` (caret) — the actual throw-site runtime can drift under the pin; the integrity pin covers solid-js only. |

---

## 4. Documentation defects (checker-side)

- `docs/rules/primitive-in-leaf-owner.md:23-26` shows `createSignal(false)` in `onSettled` annotated "Throws" — the runtime does **not** throw for value-form signals (probed) and the checker itself correctly does not flag it. The implementation is right; the doc example is wrong.
- `docs/semantic-inventory.md` is stale against rc.0: no mention of `loadingValue`/`seedLoadingValue`/`ssrSource` in the async proof obligations; claims optimistic-store variants participate in `sync: true` checks (untrue, §1.4).
- `docs/rules/README.md` v1 table lists 39 of 42 rules — `v1/prefer-component-syntax`, `v1/no-implicit-draggable`, `v1/valid-jsx-nesting` have pages and manifest entries but are missing from the table.
- `reactive-read-after-await.md` claims store-path/props coverage the engine doesn't have (§2.4); finding span is off by one (`static_rules.rs:476-480`).
- SC2001's evidence string lists four allowed regions; its rule page lists six.
- SC9001's fix hint for removed 1.x APIs (`batch`, `onMount`, …) says "write a contract entry" — a migration pointer would serve users better. (Behavior itself is good: v2 projects using removed APIs fail closed, pinned in `reactive-ir__solid-1x-leftovers.json`.)

**Upstream (solidjs/solid) doc bugs found while auditing:** RFC 01/08 say writes in `untrack` blocks are allowed — the rc.0 runtime throws (§2.1). `ACTION_CALLED_IN_OWNED_SCOPE` exists in the runtime but is absent from the 08-dev-diagnostics table.

---

## 5. Test hygiene

- **SC8019** and **SC8020** have zero public fixture coverage (crate-internal tests only) — notable given SC8020 carries the audit's most user-visible FP.
- **SC9004** execution-map-incomplete has no test anywhere.
- All other 35 rules appear in at least one snapshot suite.

---

## 6. What is verifiably right (non-exhaustive)

- Full mapping of the official dev-diagnostics table; the runtime-only exclusions (`RUN_WITH_DISPOSED_OWNER`, infinite loops, `REACTIVITY_HALTED`, `INVARIANT_VIOLATION`) survive scrutiny.
- Control-flow callback shapes match rc.0 types **verbatim** (keyed raw / `keyed={false}` accessor / custom-key both / `Repeat` plain number; `Show`/`Match` keyed raw), with fixtures pinning each.
- SC3004 invalid-cleanup-return matches the runtime's validation sites exactly, including `return null` ≠ `undefined` and async-callback rejection; `unobserved` correctly excluded. **(Superseded 2026-08-17: the rule was removed — `EffectFunction`'s real return type makes every one of those sites a `tsc` error. See docs/precision-backlog.md.)**
- SC7004, zero-arg `refresh()`, thunk-`refresh` rejection, `affects` arity — all anchored on real dev throw sites.
- SC2001's `refresh()`-invalidation half **is** covered (fixtures prove it), and `ownedWrite` is modeled end-to-end, matching `CONFIG_OWNED_WRITE`.
- SC1002's dominance guards (conditional awaits, loops, nested closures exempt; both-branch and try/finally fire) are precise and pinned.
- Ownership rules SC4001–SC4003 faithfully model 2.0's parent-owned `createRoot` and `runWithOwner(null)` detachment; the documented exported-function uncertifiable escalation works as stated.
- All `refresh`/`affects`/`isPending` rule pages already describe the **current** quiet-re-ask semantics — no stale beta material.
- The dual-name/shared-code scheme (SC7003, SC9003) is implemented, test-enforced, and documented coherently.

---

## 7. Priority fix list

1. **Model the rc.0 option surface** (`loadingValue`, `seedLoadingValue`, `ssrSource`) in async provenance: kills the SC5001/5002/5003 FP class, enables the `ssrSource:"client"`-outside-`Loading` FN (§2.2), and unstales the semantic inventory.
2. **Fix the write-scope model:** children-forbidden scopes (`createTrackedEffect`, owner-backed `onSettled`) are *legal* write/action sites; `untrack` is *not*. Kills the SC2001/SC2002 FP class and the untrack FN in one change (and file the upstream doc bug).
3. **Gate leaf-owner rules on call-site ownership**, not lexical shape (out-of-band `onSettled` FPs across SC3001/3002/3003).
4. **Accept member-expression store-record targets in `affects`** (SC7003) and add the `refresh(plainStore)` dev-throw; drop the inert `refresh` arity check and the store-family `sync` targets (SC7002).
5. **Add WHATWG scope-boundary stops to SC8020's ancestor walk** (nested lists!) and give SC8019/SC8020 public fixtures.
6. **Reclassify the props over-approximation** (violation → uncertifiable, or prove signal-backing via callers) for SC1001-props/SC1003/SC1007-member; document or align the SC1003 severity escalation.
7. **Align SC4004 severity with the runtime error** and add it to the severity-mirroring test.
8. Doc pass: rule-page fixes above, SC9005 rc-pin note, README v1 table, semantic-inventory refresh.
