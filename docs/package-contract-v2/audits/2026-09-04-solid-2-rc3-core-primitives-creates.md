# Audit: Solid 2.0.0-rc.3 core primitives — the `creates` call domain

Date: 2026-09-04. Status: **draft for the repository owner's review**. No
audited JSON document was touched by it.

**Note on the working tree, added in review.** The paragraph that used to stand
here said "no row was added to `rust/crates/solid-dialect/src/solid_2.rs` […]
the negative table is still the 24 rows ADR 0007 shipped". That is no longer
true of the tree this document sits in. Everything this audit proposed has been
wired, and § 7.4's semantic question has been decided as this audit recommended:

- `semantic-model.md` § creates carries `[Decision 2026-09-04]` — "Handing a
  value to a per-request render context that writes it into the response is a
  `create`" — with a following paragraph that a guarded reach still counts and
  a flat `(package, export, domain)` row must therefore withhold. Signed off by
  delegation, 2026-09-04. That is § 7.4's **option (a)**.
- `docs/adr/0007-census-dialect-axiom-tier.md` has been amended with the "Two
  citation kinds, and they are not equally mechanical" section that F2 below
  asked for, so its old "a test […] re-derives the closure, so a row cannot
  drift" sentence is gone.
- `solid_2.rs` carries the `AuditedCitation::Implementation` variant, the R1–R5
  rows, and **the withdrawal of `(solid-js, createEffect, Creates)`**. Its count
  assertion reads 25 derivable from JSON, minus `hydrate` and `createEffect`
  (both withheld) = 23, plus the 5 this census closed = **28 shipped**.
- The 15 cited slices are checked in under
  `rust/crates/solid-dialect/audited-slices/`, and both `scripts/verify.sh` and
  the `Makefile` export `SOLID_CHECKER_RC3_ARCHIVE_ROOT` so the archive arm
  actually runs (§ 9.2 (a)–(d)).

So the § 0 table below should now be read as the **review record** — the
sentences the decisions were taken against — rather than as an open ask. Every
verdict in it still stands unchanged: R1–R5 granted, R6 and `hydrate` withheld,
`createEffect` withdrawn.

One mechanical caveat that follows from that wiring: **`solid_2.rs` line
numbers in this document are the pre-wiring file's** (§ 9.1's `:1754-1824` and
`:1835-1915`, § 9.4's insertion points, and the like). The rows and tests moved
when they landed — `every_negative_row_citation_resolves_to_the_bytes_it_claims`
is now at `:2162`, `the_negative_table_is_derived_from_the_audited_documents` at
`:2402` — and the *archive* citations (file, digest, byte range) are unaffected
because they name published tarball bytes, not repository lines. Only the
`solid_2.rs` cross-references need re-reading against the current file.

This is a hand implementation census, in the sense of
`docs/adr/0008-implementation-census-for-creates.md` and
`docs/package-contract-v2/phase21/2026-09-03-implementation-census-plan.md`
§ 3, performed over the exact published rc.3 bytes for the five primitives
that ADR 0008 names as the actual next blocker on real rows — `createSignal`,
`createRoot`, `onCleanup`, `untrack`, `getOwner` — plus a re-check of the
table's existing treatment of `render` and `hydrate` (§ 8). It decides only
what `semantic-model.md` § "What a closed call domain denies" § creates lets it
decide:

> `creates: [] closed` denies that one invocation of this export gives rise to
> any operation of kind X, where X is exactly *this export performing a
> published `create` operation* — […] **registering a version-1 resource into
> a runtime outside this invocation** — a browser document or a server runtime
> — so that the resource remains live there after the call returns, reachable
> by that runtime rather than only through a value the call handed back.

Reactive nodes, owners and computations coming into existence are **not**
`create` operations (§ creates, the three "not a `creates` item" homes, and the
census plan § 3 item 3). Every section below says so where it applies rather
than leaving it implied.

---

## 0. For the owner's sign-off

Each line is one row (or one withholding) and the single sentence the owner
must agree with to accept it. Section numbers point at the evidence.

Every sentence below is calibrated to be **exactly the claim the row makes and
no stronger**. In particular none of them says an export "does nothing" or
"performs no operation of any kind" — a `creates: [] closed` row denies one
operation kind, and each of these exports demonstrably performs others
(`returns`, `callbacks`, `cleanups`, `reads`). Where a bundle differs, the
sentence names the bundle; where a guard decides the answer, the sentence names
the guard. If a sentence still reads as broader than its section proves, it is
the sentence that is wrong, not the section.

| # | Proposed row / decision | The sentence to agree with | § |
| --- | --- | --- | --- |
| R1 | `(@solidjs/signals, createRoot, Creates)` — **grant** | `createRoot` allocates an owner object, links it into the in-memory owner tree, and runs the caller's `init` under it; it registers no version-1 resource into any runtime outside the invocation, and its one host reach is the scheduler's one-shot `flush` microtask on the dispose path. | § 3 |
| R2 | `(@solidjs/signals, getOwner, Creates)` — **grant** | `getOwner` is `return context` in all three bundles; it contains no call, `new`, or other invoking form; it performs no `create`. (Its `returns` is positive — § 4.4 — so this is not a claim that it does nothing.) | § 4 |
| R3 | `(@solidjs/signals, onCleanup, Creates)` — **grant** | `onCleanup` appends the caller's function to the current owner's `_disposal` field (or, in the dev bundle only, emits a diagnostic and `console.warn`s or throws); the registration is onto a reactive-graph owner, not a runtime outside the invocation, and is a `cleanups` item rather than a `create`. | § 5 |
| R4 | `(@solidjs/signals, untrack, Creates)` — **grant** | `untrack` toggles the module-level `tracking` flag around a call to the caller's `fn` (or to the external-source hook, when one is installed), and itself registers no version-1 resource into any runtime outside the invocation. | § 6 |
| R5 | `(@solidjs/signals, createSignal, Creates)` — **grant** | `@solidjs/signals`' `createSignal` allocates a signal or computed node, links it into the owner tree, runs the caller's compute synchronously for the derived overload unless `options.lazy` (`core.js:518`), and returns bound accessors; every host-runtime reach on `createSignal`'s paths is a `queueMicrotask(flush)` that registers no version-1 resource. | § 7 |
| R6 | `(solid-js, createSignal, Creates)` — **WITHHOLD** | `solid-js`'s own `createSignal` (the declaration a `solid-js` import resolves into) is a hydration-aware wrapper over R5 on the browser conditions, but on the `node`/`worker`/`deno` conditions it is a different implementation whose derived overload reaches `ctx.serialize(id, deferred.promise, deferStream)` — a registration of the pending computation with the per-request SSR serializer — and whether that is a version-1 `create` is a semantic decision this audit may not take, though it recommends the answer "yes" (option (a), § 7.4 — since **decided that way**, so R6 stays withheld permanently, as `render` is). | § 7.4 |
| H1 | `(@solidjs/web, hydrate, Creates)` — **keep withheld** | `hydrate` reaches `render` on every one of its four return paths in all four browser bundles, and `render` calls `registerDelegatedRoot(element)` on every path that reaches its own `createRoot`, so the audited `creates: []` on `hydrate` is contradicted by the bytes exactly as the table already says. | § 8 |
| F1 | Citation format | A row read from runtime bytes rather than from a summary object needs a second `AuditedCitation` variant that names the audit document section, the archive file, its pinned `files.json` digest, the byte range, and the range's own digest. *(Landed.)* | § 9.1–9.2 |
| F2 | What that citation proves | For an `Implementation` citation **nothing mechanical re-derives "no `create`"** — the slice digest pins only the *subject* the human read — so ADR 0007's then-current "a test re-reads that range and re-derives the closure, so a row cannot drift from the bytes" would be false for these rows and had to be reworded before they landed. *(Discharged: the ADR now carries "Two citation kinds, and they are not equally mechanical".)* | § 9.2 |
| C1 | Consistency flag (no row moves *here*) | The **existing** `(solid-js, createEffect, Creates)` row has the same server-condition exposure as R6: `solid-js/dist/server.js:868-870` routes `createEffect` to `serverEffect`, which reaches `processResult` at `:835` — and so `ctx.serialize` — when the caller passes a truthy `options.ssrSource`. The row was read from the `browser/development` audit only. | § 7.4 |
| C2 | The `createEffect` withdrawal decision | Under option (a) — which § 7.4 **recommends** and which `semantic-model.md` § creates has since **decided** — the existing `(solid-js, createEffect, Creates)` row is unsound on the guarded server path, so it must be **withheld** until `NEGATIVE_ROWS` can carry a condition/guard predicate: the JSON-derived half of the table goes `24 → 23`, and R1–R5 add 5 on top of that (28). Agreeing to (a) was agreeing to that withdrawal; it has been made. | § 7.4 |

Accepting R1–R5 also means accepting one stated approximation that the table
already carries for other rows and that this audit makes concrete (§ 1.4): a
consumer that imports these names from `solid-js` under the `node` condition
runs **`solid-js/dist/server.js`'s own implementations**, not
`@solidjs/signals`' bytes, while its declaration resolves into
`@solidjs/signals`. Those server implementations were read too and reach the
same verdict for R1–R4; the tier cannot see the split, so the audit carries it.

---

## 1. Inputs, identity, and method

### 1.1 The archives

The three archives are the audited tuples in `solid_2.rs` `AUDITED_ARCHIVES`
(`rust/crates/solid-dialect/src/solid_2.rs:113-132`). The bytes read here are
the installs under `rust/target/tsc-oracle/v2/node_modules/` (the tree the
tsc oracle and the fourth harness review used). Their identity was
re-established rather than assumed:

| Archive | `package.json` sha256 on disk | Equals tuple `manifest_sha256` | Registry integrity (`phase0/rc3/*/registry-metadata.json` `dist.integrity`) equals tuple `integrity` |
| --- | --- | --- | --- |
| `@solidjs/signals@2.0.0-rc.3` | `22d27a9ebdc7b4fbfc65b9857bbea96ea60d3617697fd628b42b6e1253ffdb76` | yes | yes (`sha512-/yPhTf3x…`) |
| `solid-js@2.0.0-rc.3` | `e703e7986516ac05ee91fdd64897c2d150aea948cb5bf77eae8673da5008ee4b` | yes | yes (`sha512-pmW6bRoT…`) |
| `@solidjs/web@2.0.0-rc.3` | `ee9b514b90b06b679d2376c5b5a993c0391aa66ec744e453ec3e534babd30e8e` | yes | yes (`sha512-5ckKgOje…`) |

Every file cited below — the 27 runtime bundles and the 11 declaration files
§ 1.2 resolves through — was additionally digested and compared with the
per-file `sha256` in the pinned manifest
`benchmarks/package-contract-v2/phase0/rc3/<archive>/files.json`; **all 38
cited files match**, and so do the three `package.json` digests in the table
above (the table in § 9.3 lists every one). So a line number in this document
is a line of the exact published tarball, not of a local rebuild.

### 1.2 Which file a bare import runs, per condition

The census terminator keys a row by the archive **the resolved declaration
lives in** (ADR 0007, `census_dialect_axiom_for_callee` gates 3–5,
`rust/crates/solid-facts-backend/src/contract_certification/type_facts.rs:4256-4300`).
The runtime a consumer actually executes is chosen by the `exports` condition.
The two do not always agree, so both were traced from each archive's
`package.json` `exports` map (read from disk; digests above).

**Declarations.** For `"."` of every archive, every condition's `types` entry
is one file: `solid-js` → `./types/index.d.ts` (`import`) /
`./types-cjs/index.d.cts` (`require`); `@solidjs/signals` →
`./dist/types/index.d.ts` / `./dist/types-cjs/index.d.cts`; `@solidjs/web` →
`./types/index.d.ts` / `./types-cjs/index.d.cts`. `solid-js/types/server/*.d.ts`
is reachable only through the `./types/*` glob, never through the bare import.

| Import written by the consumer | Declaration the compiler resolves to | Row archive |
| --- | --- | --- |
| `createRoot`, `getOwner`, `onCleanup`, `untrack` from `@solidjs/signals` | `@solidjs/signals/dist/types/index.d.ts:1` → `core/owner.d.ts:121` / `:61`, `signals.d.ts:41` (via `index.d.ts:5`), `core/core.d.ts:75` | `@solidjs/signals` |
| the same four from `solid-js` | `solid-js/types/index.d.ts:1` is `export { …, createRoot, …, getOwner, …, onCleanup, …, untrack } from "@solidjs/signals";` — a pure re-export, so the declaration is the `@solidjs/signals` file above (the ADR 0007 cross-archive finding, here working *for* the row) | `@solidjs/signals` |
| `getOwner`, `untrack` from `@solidjs/web` | `@solidjs/web/types/index.d.ts:3` `export * from "./client.js"`; `client.d.ts:1` imports them from `"solid-js"` and `:93` re-exports them → the `@solidjs/signals` files above | `@solidjs/signals` |
| `createSignal` from `@solidjs/signals` | `@solidjs/signals/dist/types/index.d.ts:5` → `signals.d.ts:262-264` | `@solidjs/signals` |
| `createSignal` from `solid-js` | `solid-js/types/index.d.ts:8` `export { …, createSignal, … } from "./client/hydration.js"` → `solid-js/types/client/hydration.d.ts:246-253` (`export declare const createSignal: { … }`, a **re-declaration**, not a re-export) | `solid-js` |

**Runtime**, per `exports` condition of `"."`:

| Archive · condition | Runtime file | The five names in it |
| --- | --- | --- |
| `@solidjs/signals` · `import` + `development`/`test` | `dist/dev.js` | own definitions (§ 3–7 cite them) |
| `@solidjs/signals` · `import` default | `dist/prod/index.js` → `dist/prod/core/owner.js` (`createRoot`, `getOwner`), `dist/prod/core/core.js` (`untrack`), `dist/prod/signals.js` (`onCleanup`, `createSignal`) — `index.js:3,7,23` | own definitions |
| `@solidjs/signals` · `require` | `dist/node.cjs` | own definitions |
| `solid-js` · `browser`/`development`/default `import` | `dist/dev.js` (dev), `dist/solid.js` (prod) | `createRoot`, `getOwner`, `onCleanup`, `untrack` **re-exported** from `@solidjs/signals` (`solid.js:2`, `dev.js:2`); `createSignal` is solid-js's own wrapper (`solid.js:751-753`, `dev.js:772-774`) |
| `solid-js` · `browser`/`development`/default `require` | `dist/solid.cjs`, `dist/dev.cjs` | the four forwarded by getter (`solid.cjs:1288-1291` `get: function () { return signals.createRoot; }`, `:1324`, `:1360`, `:1400`; `dev.cjs:1327`, `:1363`, `:1399`, `:1439`); `createSignal` own wrapper (`solid.cjs:752-754`, `dev.cjs:773-775`) |
| `solid-js` · `node`/`worker`/`deno` | `dist/server.js` (`import`), `dist/server.cjs` (`require`) | **all five are solid-js's own server implementations**: `getOwner` `server.js:91-93`, `onCleanup` `:97-102`, `createRoot` `:175-178`, `createSignal` `:285-305`, `untrack` `:1392-1394` (`server.cjs` +1 line each: `:92`, `:98`, `:176`, `:286`, `:1393`). `server.js:2` re-exports from `@solidjs/signals` only `$PROXY, $REFRESH, $TRACK, NotReadyError, enableExternalSource, enforceLoadingBoundary, flatten, isEqual, isWrappable, omit, snapshot, storePath` — none of the five. |
| `@solidjs/web` · browser conditions | `dist/web.js`, `dist/dev.js` (+ `.cjs`) | `getOwner`, `untrack` re-exported from `solid-js` (`web.js:2`, `dev.js:2`; `web.cjs:1947`, `:1955`; `dev.cjs:2017`, `:2025`) → the `solid-js` row above for that condition |
| `@solidjs/web` · `node`/`worker`/`deno` | `dist/server.js` (+ `.cjs`) | `getOwner`, `untrack` re-exported from `solid-js` (`server.js:2`; `server.cjs:3376`, `:3392`) → `solid-js/dist/server.js` |

### 1.3 What the method was

For each primitive: the declaration was located; every runtime file the name
is reachable from (table above) was opened at the definition; every call in
the body was followed until it resolved to one of:

- **module-local (read)** — a helper in the same file or the same archive whose
  body was read and is cited;
- **archive-cited** — a call into another audited archive whose section in
  this document is cited (`solid-js` → `@solidjs/signals`);
- **engine builtin** — a default-library member; the audit states what it
  transfers control to, if anything (`Function.prototype.bind` creates a bound
  function and invokes nothing; `Array.prototype.push` invokes nothing; …);
- **caller-supplied callable** — the census plan § 3.2 rule: the invocation is
  a `callbacks` item, the body is not this export's behavior;
- **installed hook** — a callable the export did not define and the caller did
  not supply, reached through module state (`DEV.hooks.*`, diagnostic
  listeners, `GlobalQueue._externalUntrack`/`_wireExternalSource`); by
  § callbacks' definition its invocation is an `invoke` and its body is not
  this export's behavior; each is **recorded**, because a `callbacks` closure
  would have to account for it;
- **host-runtime touch** — any reference to `document`, `window`, `globalThis`,
  a timer, `queueMicrotask`, `Promise`, `console`, or `fetch`; each is
  recorded with what it registers and for how long.

Where a transitive closure ran into the reactive scheduler (the derived
`createSignal(fn)` overload and the owner-disposal path), the walk is bounded
by an **archive-wide host-boundary census** (§ 1.5) instead of by an exhaustive
enumeration of every scheduler helper. That census is stated as a separate,
checkable fact, and the sections say which dispositions rest on it.

### 1.4 The declaration/runtime archive split the tier cannot see

For R1–R4 the row is keyed to `@solidjs/signals` because the declaration is
there. A consumer importing from `solid-js` under the `node` condition runs
`solid-js/dist/server.js`'s own bodies (§ 1.2), which are **not** bytes of the
archive the row names. `census_dialect_axiom_for_callee` binds only the
declaration's archive (gate 5), so it would grant the row there on the
strength of `@solidjs/signals`' bytes. This audit therefore read the server
bodies as well (§ 3.4, § 4.3, § 5.4, § 6.3) and reaches the same verdict, which
is what makes granting R1–R4 sound for that import path. It is the mirror
image of the "Four rows dead via cross-archive re-export" approximation in
`docs/precision-backlog.md`, and it should be recorded beside it: **the tier
binds the declaration's archive; the runtime archive can differ by
condition, and a row is sound only when both were read.**

### 1.5 Archive-wide host-boundary census of `@solidjs/signals@2.0.0-rc.3`

A `grep` over every `dist/prod/**/*.js` file, `dist/dev.js`, and
`dist/node.cjs` for `document`, `window`, `navigator`, `globalThis`,
`addEventListener`, `queueMicrotask`, `setTimeout`, `setInterval`,
`requestAnimationFrame`, `MessageChannel`, `process`, `performance`,
`localStorage`, `fetch`, `Promise`, `Date`, and `self` — excluding comment
lines — finds exactly these code references in the whole archive:

| File:line | Code | What it registers, and where |
| --- | --- | --- |
| `dist/prod/core/scheduler.js:158` | `if (!syncDepth && !globalQueue.sn && !projectionWriteActive) queueMicrotask(flush);` (inside `schedule()`, `:151-159`) | a one-shot microtask that drains the module's own queues; no version-1 resource, nothing addressable by the host afterwards |
| `dist/prod/signals.js:218` | `queueMicrotask(() => t(e));` (`MicrotaskQueue.enqueue`, `:216-220`, reached only by `onSettled`'s queue — not by any of the five) | same |
| `dist/dev.js:1224` | `queueMicrotask(flush)` (dev `schedule()`, `:1217-1226`) | same |
| `dist/dev.js:5892` | `queueMicrotask(() => fn(type));` (dev `MicrotaskQueue`) | same; not reached by the five |
| `dist/dev.js:174` | `const now = typeof performance !== "undefined" ? () => performance.now() : () => Date.now();` | a module-initialization read of a clock; not an operation of any export |
| `dist/dev.js:923` | `typeof globalThis !== "undefined" && !!globalThis.process?.env?.COMPANION_CENSUS;` | a module-initialization read whose value is discarded; not an operation of any export |
| `dist/node.cjs:521` | `queueMicrotask(flush)` (`schedule()`, `:514-522`) | same as prod |
| `dist/node.cjs:4577` | `queueMicrotask(() => t(e));` (`MicrotaskQueue`) | same; not reached by the five |
| `dist/prod/signals.js:243`, `dist/prod/core/action.js:77` | `new Promise((t, n) => {…})` in `resolve` (`signals.js:242-…`) and in the function `action` returns (`action.js:76-…`) | a promise handed straight back to that export's caller; not reached by any of the five |
| `dist/dev.js:5580`, `:5922`; `dist/node.cjs:4314`, `:4602` | the dev / CJS spellings of the same two `new Promise` sites | same; not reached by the five |
| `dist/dev.js:174` | `Date.now()` as the fallback arm of the module-init clock (same line as the `performance` entry above) | not an operation of any export |
| `dist/dev.js:371` | `const now = Date.now();` in `checkHotRuns` (`:367-…`) | a dev rerun-tracing clock read. Reached only from `recordRerun` (`:437`, calling `checkHotRuns` at `:468`), which the tracer calls at `:615` only when `frame.causes !== null`; a **creation** run has `causes === null` and takes the `checkDepWidth` branch at `:626` instead. Registers nothing either way |

There is **no** reference to `document`, `window`, `addEventListener`, a timer,
or a network API anywhere in the archive's runtime bytes, and every `Promise`
construction in the archive belongs to `resolve` or `action`. Consequence, used
below: no call that stays inside `@solidjs/signals` can register a version-1
resource into a browser document or a server runtime, because the archive has
no handle to either. The *only* reach outside the invocation is a microtask
that runs the archive's own `flush`. Whatever `flush` later does (running
queued effects — caller-supplied callables — and committing the graph) stays
inside the same archive and the same census.

`dist/prod/core/async.js` deserves its own sentence, because it is the file the
derived `createSignal` overload reaches through `handleAsync` (§ 7.2) and it is
where `Promise`/thenable machinery lives. It **constructs no promise**: the
name `Promise` appears in it only inside comments (`:185`, `:253`, `:382`).
Everything it does with a thenable or an async iterable it does *through the
caller's own value* — `isThenable(e)` is a duck-type check on `e.then`
(`:186-188`), and the reaches are `e.then(…)` (`:366`, `:387`, `:481`) and
`t[Symbol.asyncIterator]()` (`:195`, `:357`, `:458`) on the object the caller's
compute returned. So the file performs no `create`: it registers nothing, and
the only continuations it attaches are onto a value that reached it from the
caller.

`solid-js/dist/server.js` was censused the same way: **no** match for any of
the host names above. Its reach outside the invocation is through
`sharedConfig.context` — the per-request render context that `@solidjs/web`'s
`renderToString`/`renderToStream` install (`@solidjs/web/dist/server.js:1034`,
`:1325`) — and through `Promise`/`console`. § 7.4 is about exactly that reach.

---

## 2. Terms used in the tables

`disposition` column values: `local` (module-local helper, read; cited),
`archive` (call into a sibling audited archive; section cited), `builtin`
(engine default-library member; what it invokes is stated), `caller` (caller-
supplied callable, § 3.2), `hook` (installed hook, § 1.3), `host` (host-
runtime touch, § 1.3). `reach` is the reachability from the export's entry at
the `MayExecute` floor: `always`, `cond:<predicate>`, or `later:<event>` for a
call this invocation causes at a later `at` event. Line numbers are of the file
named in the row's first column unless another file is named.

---

## 3. `createRoot` — archive `@solidjs/signals@2.0.0-rc.3`

### 3.1 Declaration

`@solidjs/signals/dist/types/core/owner.d.ts:121-124`:

```ts
export declare function createRoot<T>(init: ((dispose: () => void) => T) | (() => T), options?: {
    id?: string;
    transparent?: boolean;
}): T;
```

Exported from the entry at `dist/types/index.d.ts:1` (and
`dist/types-cjs/index.d.cts:1` for `require`). `solid-js/types/index.d.ts:1`
re-exports it verbatim from `@solidjs/signals` (§ 1.2).

### 3.2 Implementation, `import` default condition — `dist/prod/core/owner.js:298-301`

```js
 */ function createRoot(e, t) {
    const n = createOwner(t);
    return runWithOwner(n, () => e(() => n.dispose()));
}
```

Transitive call table (14 rows):

| Callee | Site | Reach | Disposition | What it does |
| --- | --- | --- | --- | --- |
| `createOwner(t)` | `owner.js:299` → body `:243-273` | always | local | builds an owner object literal (`:246-261`); if a parent `context` exists, links it as the parent's first child (`:262-271`). Fields only; no host reference. |
| `inheritId(e, n, t)` | `owner.js:247` → `:145-147` | always | local | returns `options.id`, or the parent's id, or the parent's next child id |
| `getNextChildId(n)` | `owner.js:146` → `:137-139` | cond: parent has an `id` | local | `childId(e, true)` |
| `childId(e, t)` | `owner.js:138` → `:125-130` | cond | local | walks transparent parents, formats the counter, or `throw new Error("")` (`:129`) |
| `formatId(e, t)` | `owner.js:128` → `:158-161` | cond | local | string arithmetic (`toString(36)`, `String.fromCharCode` — builtins, invoke nothing) |
| `runWithOwner(n, …)` | `owner.js:300` → `core.js:859-870` | always | local | saves `context`/`tracking`, sets `context = e; tracking = false`, calls the arrow, restores in `finally` |
| `e(() => n.dispose())` | `owner.js:300` (inside the arrow `runWithOwner` invokes) | always | **caller** | the caller's `init`; its body is the caller's behavior (§ 3.2 of the census plan). The export supplies the `dispose` arrow — see next row. |
| `n.dispose()` | `owner.js:300` → `disposeRootSelf` `:232-234` | later: caller invokes the returned `dispose` | local | `disposeChildren(this, e)` |
| `disposeChildren(e, t, n)` | `owner.js:233` → `:44-107` | later | local, and **caller** at `:105` | sets `REACTIVE_DISPOSED`; walks children calling `deleteFromHeap`, `clearDeps`, `disposeChildren` recursively (`:59-81`); splices out of the parent chain (`:92-98`); `runDisposal` (`:99`); then, inside `if (t && e.At)` (`:102-106`), reads the effect-returned cleanup out of `e.At`, clears the field (`:104`), and **invokes it at `:105`** (`t()`). That invocation's target is the value the caller's effect function *returned*, so its disposition is **caller** — the body is the caller's, a `callbacks`/`cleanups` item of whatever disposes the owner, not this export's behavior (census plan § 3.2). Same shape as `runDisposal`'s registered disposals below |
| `GlobalQueue.un(t)` | `owner.js:55` | later, cond: node has a pending/latest companion (`t.o?.Ge \|\| t.o?.ge`) | hook (engine-internal, installed by the verdict module — `scheduler.js:313 static un=null`) | snaps `isPending`/`latest` companion signals of a disposed node; a reactive write inside the archive, no host reach (§ 1.5) |
| `deleteFromHeap`, `queueFor` | `owner.js:25-26`, `:77` → `heap.js:70-84`, `:7-9` | later | local | unlink from the module's heap arrays; field writes only |
| `clearDeps` → `unlinkSubs` → `unobserved` | `owner.js:78` → `graph.js:48-56`, `:10-31`, `:58-62` | later | local | unlink dependency edges; `unobserved` (`:58-62`) is `deleteFromHeap` + `clearDeps` + `disposeChildren` — the same three helpers, recursion bounded by the tree; `l.o?.ft?.()` (`:19`) invokes a caller-supplied `unobserved` option callback (**caller**) |
| `runDisposal(e, n)` | `owner.js:99` → `:109-123` | later | local | calls each registered disposal with `t.call(t)` — `Function.prototype.call` on **caller**-registered functions (`onCleanup`'s argument; § 5); the invocations are `callbacks` items, the bodies are the callers' |
| `markDisposal`, `insertIntoHeap`, `insertIntoHeapHeight` | `owner.js:14-31`, `heap.js:43-61`, `:63-68` | later, cond: zombie path (`n` set) | local | flag and heap bookkeeping only |

Nothing in the table touches a host runtime; the only outward reaches an
owner-tree teardown can cause are through the scheduler's `schedule()` →
`queueMicrotask(flush)` (§ 1.5) if a companion snap enqueues work. **No path
performs a `create`.** The owner coming into existence at `:299` and the child
link at `:262-271` are reactive-graph facts: § creates says a private module
structure "is neither" a version-1 resource kind nor a runtime that acts on it,
and the model has no domain in which an owner's coming-into-existence is a
closable positive fact ("That is a real model gap", § creates).

### 3.3 The other two `@solidjs/signals` bundles

- `dist/dev.js:2342-2345` is the same body (`createOwner(options)` then
  `runWithOwner(owner, () => init(() => owner.dispose()))`). Its `createOwner`
  (`:2275-2317`) adds two dev-only branches: a `PRIMITIVE_IN_FORBIDDEN_SCOPE`
  diagnostic and `throw` when the parent forbids children (`:2294-2304`), and
  `DEV$1.hooks.onOwner?.(owner)` (`:2315`) — an **installed hook**. `emitDiagnostic`
  (`:748-756`) iterates `diagnosticListeners`/`diagnosticCaptures` — installed
  hooks and module arrays. `runWithOwner` (`:4232-4260`) adds a
  `RUN_WITH_DISPOSED_OWNER` warning (`:4233-4245`, `console.warn` — **host**,
  writes to the console, registers nothing) before the same swap. `disposeChildren`
  (`:2065-2136`) additionally calls `clearSignals(node)` (`:2078`, clears a dev
  registry field) and `GlobalQueue._snapCompanions(n)` (`:2076`, the named form of
  prod's `un`). Same verdict.
- `dist/node.cjs:1597-1600` is byte-for-byte the prod body under different
  mangled names (`createOwner` `:1542-1572`, `runWithOwner` `:3140-3151`,
  `disposeChildren` `:1343-1406`, `runDisposal` `:1408-1422`, `cleanup`
  `:1506-1510`). Same verdict.

### 3.4 The `solid-js` server condition (§ 1.4)

`solid-js/dist/server.js:175-178`:

```js
function createRoot(init, options) {
  const owner = createOwner(options);
  return runWithOwner(owner, () => init(() => disposeOwner(owner)));
}
```

`createOwner` (`:44-81`) increments a module counter, pops a pooled owner or
builds a literal, links it under `currentOwner`; `nextChildIdFor` (`:26-33`,
throws without an id); `runWithOwner` (`:82-90`) swaps `currentOwner`;
`disposeOwner` (`:130-169`) runs registered disposals (`d[i]()` — **caller**),
`unlinkOwner` (`:119-129`), and returns the node to `ownerPool`. No host
reference in the file (§ 1.5). `server.cjs:176-178` is identical. **Same
verdict.**

### 3.5 Verdict

**`creates` closed** for `(@solidjs/signals, createRoot)` across all three
archive bundles, and across the `solid-js` re-export on every condition
including the server implementation. No other domain is decided here:
`callbacks` is positive (`init` at `:300`), `disposals` is positive on the
returned `dispose`, and the disposal path's companion snap is a reactive write
whose `writes` accounting this audit did not undertake.

---

## 4. `getOwner` — archive `@solidjs/signals@2.0.0-rc.3`

### 4.1 Declaration

`@solidjs/signals/dist/types/core/owner.d.ts:61`: `export declare function getOwner(): Owner | null;`
— entry `dist/types/index.d.ts:1`; `solid-js/types/index.d.ts:1` re-exports;
`@solidjs/web/types/client.d.ts:1,93` re-exports from `solid-js` (§ 1.2).

### 4.2 Implementation, all three bundles

| Bundle | Body |
| --- | --- |
| `dist/prod/core/owner.js:199-201` | `function getOwner() { return context; }` |
| `dist/dev.js:2230-2232` | `function getOwner() { return context; }` |
| `dist/node.cjs:1498-1500` | `function getOwner() { return context; }` |

Transitive call table: **0 rows** — the body contains no call, `new`, or
non-call invoking form; `context` is a module-level `let` (`core.js:41`).

### 4.3 The `solid-js` server condition

`solid-js/dist/server.js:91-93`: `function getOwner() { return currentOwner; }`
(`currentOwner` is `let` at `:18`). `server.cjs:92-94` identical. **Same
verdict.**

### 4.4 Verdict

**`creates` closed** for `(@solidjs/signals, getOwner)`. Also decided with the
same rigor, in every bundle and condition, because the body is a single
return of a module variable: `callbacks: []`, `reads: []` (a module `let` is not
a reactive source), `writes: []`, `invalidates: []`, `cleanups: []`,
`disposals: []` are all closable. Not decided: `returns` (positive — it yields a
value; and the table withholds the domain wholesale) and `throws` (no census
target under version 1). These extra closures are **offered, not proposed as
rows**: the table admits `creates` only and
`the_negative_table_is_derived_from_the_audited_documents` compares admitted
domains only, so widening it is a deliberate act of its own (`solid_2.rs:1876-1885`).

---

## 5. `onCleanup` — archive `@solidjs/signals@2.0.0-rc.3`

### 5.1 Declaration

`@solidjs/signals/dist/types/signals.d.ts:41`: `export declare function onCleanup(fn: Disposable): Disposable;`
— entry `dist/types/index.d.ts:5`; `solid-js/types/index.d.ts:1` re-exports.

### 5.2 Implementation, `import` default — `dist/prod/signals.js:55-57`

```js
 */ function onCleanup(e) {
    return cleanup(e);
}
```

| Callee | Site | Reach | Disposition | What it does |
| --- | --- | --- | --- | --- |
| `cleanup(e)` | `signals.js:56` → `owner.js:207-211` | always | local | `if (!context) return e;` else stores `e` in `context.he` as a value, an array push (`Array.prototype.push` — builtin, invokes nothing), or a two-element array; returns `e` |

**1 row.** The stored function is the caller's; it is not invoked here. It is
invoked later by `runDisposal` (§ 3.2) when the owner is disposed — that later
invocation is a `callbacks` item of *whatever disposes the owner*, and the
registration itself is `onCleanup`'s `cleanups` item, onto a reactive-graph
owner. Nothing reaches a host runtime.

### 5.3 The other two bundles

- `dist/dev.js:5686-5714`: guarded by two dev-only branches before the same
  `return cleanup(fn)` (`:5713`): no owner → `emitDiagnostic` (`:748-756`,
  installed hooks) + `console.warn` (**host**, console only) (`:5689-5698`);
  owner forbids children → `emitDiagnostic` + `throw new Error` (`:5699-5711`).
  `cleanup` is `:2238-2244`, same shape. Same verdict.
- `dist/node.cjs:4414-4416` → `cleanup` `:1506-1510`. Same verdict.

### 5.4 The `solid-js` server condition

`solid-js/dist/server.js:97-102`: reads `currentOwner`; `if (!o) return fn;`
else stores `fn` in `o._disposal` exactly as prod does. `server.cjs:98-103`
identical. **Same verdict.**

### 5.5 Verdict

**`creates` closed** for `(@solidjs/signals, onCleanup)`. Also decidable in
every bundle: `reads: []`, `writes: []`, `invalidates: []`, `disposals: []`.
**Not** decidable across conditions: `callbacks` — prod and node invoke
nothing, but dev's `emitDiagnostic` invokes installed diagnostic listeners on
the no-owner path (`dev.js:5692`), an `invoke` by § callbacks; a row would have
to be withheld for that domain. `cleanups` is positive by construction.

---

## 6. `untrack` — archive `@solidjs/signals@2.0.0-rc.3`

### 6.1 Declaration

`@solidjs/signals/dist/types/core/core.d.ts:75`:
`export declare function untrack<T>(fn: () => T, strictReadLabel?: string | false): T;`
— entry `dist/types/index.d.ts:1`; `solid-js/types/index.d.ts:1` re-exports;
`@solidjs/web/types/client.d.ts:1,93` re-exports from `solid-js`.

### 6.2 Implementation, `import` default — `dist/prod/core/core.js:600-610`

```js
 */ function untrack(e, t) {
    if (GlobalQueue.Gt === null && !tracking && true) return e();
    const n = tracking;
    tracking = false;
    try {
        if (GlobalQueue.Gt !== null) return GlobalQueue.Gt(e);
        return e();
    } finally {
        tracking = n;
    }
}
```

| Callee | Site | Reach | Disposition | What it does |
| --- | --- | --- | --- | --- |
| `e()` | `core.js:601`, `:606` | always unless an external-source hook is installed | **caller** | the caller's thunk |
| `GlobalQueue.Gt(e)` | `core.js:605` | cond: `enableExternalSource` installed a hook (`external.js:68` `GlobalQueue.Gt = externalSourceConfig ? externalUntrack : null`) | **hook** | the external source's own untrack wrapper, which is handed `e` |

**2 rows.** `tracking` is a module `let` (`core.js:21`); toggling it is not a
reactive write. Nothing reaches a host runtime. The `true` at `:601` is the
compiled-out dev `!strictRead && !strictReadLabel` guard (compare dev below).

### 6.3 The other bundles, and the `solid-js` server condition

- `dist/dev.js:3847-3861`: the same shape with `strictRead` saved/set/restored
  (`:3851`, `:3853`, `:3859`, module `let` at `:3819`) and the hook spelled
  `GlobalQueue._externalUntrack` (`:3848`, `:3855`). Same verdict.
- `dist/node.cjs:2881-2891`: prod body, hook slot spelled `GlobalQueue.Ee`.
  Same verdict.
- `solid-js/dist/server.js:1392-1394`: `function untrack(fn) { return fn(); }`
  (`server.cjs:1393-1395`). Same verdict.

### 6.4 Verdict

**`creates` closed** for `(@solidjs/signals, untrack)`. Also decidable in every
bundle and condition: `reads: []`, `writes: []`, `invalidates: []`,
`cleanups: []`, `disposals: []` — the export itself performs none; what `fn`
does is the caller's. `callbacks` is positive (`fn`, and the hook when
installed).

---

## 7. `createSignal` — two archives, two rows

### 7.1 Declarations

- `@solidjs/signals/dist/types/signals.d.ts:262-264` (three overloads:
  `()`, `(value: Exclude<T, Function>, options?)`, `(fn: ComputeFunction<T>, options?)`)
  — entry `dist/types/index.d.ts:5`. This is the declaration a consumer's
  `import { createSignal } from "@solidjs/signals"` resolves to → **row R5,
  archive `@solidjs/signals`**.
- `solid-js/types/client/hydration.d.ts:246-253` — `export declare const
  createSignal: { … }` with four call signatures, the last two taking
  `HydrationSignalOptions<T>`; entry `solid-js/types/index.d.ts:8`. This is the
  declaration `import { createSignal } from "solid-js"` resolves to → **row R6,
  archive `solid-js`**. It is a re-declaration, not a re-export, so the
  cross-archive re-keying of § 1.2 does **not** happen here and R5 does not
  cover a `solid-js` import.

### 7.2 R5 — `@solidjs/signals`' implementation, `import` default — `dist/prod/signals.js:65-73`

```js
function createSignal(e, t) {
    if (typeof e === "function") {
        const n = computed(e, t);
        n.T &= ~CONFIG_AUTO_DISPOSE;
        return [ accessor(n), setMemo.bind(null, n) ];
    }
    const n = signal(e, t);
    return [ accessor(n), setSignal.bind(null, n) ];
}
```

Transitive call table (12 rows):

| Callee | Site | Reach | Disposition | What it does |
| --- | --- | --- | --- | --- |
| `signal(e, t)` | `signals.js:71` → `core.js:528-559` | cond: first arg not a function | local | builds a signal object literal (`:529-547`); optional `ext(i).ft = t.unobserved` stores a **caller** option callback without invoking it (`:548`); optional firewall link (`:549-552`, `n === null` here); snapshot-capture bookkeeping into the module set `snapshotSources` (`:553-557`) |
| `accessor(n)` | `signals.js:69`, `:72` → `:59-63` | always | local | `read.bind(null, e)` — `Function.prototype.bind`, **builtin**, creates a bound function and invokes nothing; sets `t[$REFRESH] = e` |
| `setSignal.bind(null, n)` / `setMemo.bind(null, n)` | `signals.js:72` / `:69` | always | builtin | bound functions handed back to the caller; not invoked by this call |
| `computed(e, t)` | `signals.js:67` → `core.js:374-419` | cond: first arg is a function | local | builds a computed object literal (`:380-415`) with `inheritId(t, n, context)` (§ 3.2 row) and `ownerInSnapshotScope(context)`; `ext(u).ft = t.unobserved` stores a **caller** option callback (`:416`); then `setupComputedNode(u, t)` (`:417`) |
| `setupComputedNode(e, t)` | `core.js:417` → `:503-526` | cond | local | links the node under `context` as first child (`:506-515`); height (`:516`); `GlobalQueue.Rt(e)` if installed (`:517`) — **hook** (`external.js` wires external sources; `scheduler.js:302 static Rt=null`); `!t?.lazy && recompute(e, true)` (`:518`) — **the derived compute runs during this call** unless the caller passed `lazy`; snapshot bookkeeping (`:519-525`) |
| `recompute(e, true)` | `core.js:518` → `:108-348` | cond: derived overload, not lazy | local | `bumpNotifyEpoch()` (`:110`); with `t = true` the disposal branch `:112-126` is skipped; sets `context = e`, `tracking = true` (`:143`, `:153`); calls **`e.Se(c)`** = the caller's compute (`:187` under `CONFIG_SYNC`, else `:197`) — **caller**; if the compute returned an object, `handleAsync(e, n)` (`:200`, `async.js`) — attaches `then`/iterator handling to the **caller's** thenable/iterable; `clearStatus`, `notifyStatus`, `parkLoadingWindow`, `settleErroredDependents` (`async.js`), `trimStaleDeps` (`graph.js:33-42`), `insertSubs`, `queuePendingNode`, `runInTransition`, `enqueueSub` + `schedule()` (`:344-347`, `scheduler.js`/`heap.js`) — module-local graph and queue bookkeeping; `e.C.enqueue(n, …)` (`:276`) is unreachable here because a computed has no effect kind (`n === undefined`) |
| `GlobalQueue.je`, `.ze`, `.$e`, `.Je`, `.p`, `.Oe` | `core.js:166`, `:176`, `:217`, `:232`, `:236`, `:239`, `:305` | cond: optimistic/transition engine installed | hook (engine-internal slots, `scheduler.js:310-340`, installed by `optimistic.js`/`verdict.js` at module load) | lane and verdict bookkeeping inside the archive |
| `schedule()` | `core.js:346` (missed-wake path) and transitively from `insertSubs`/`queuePendingNode` | later: microtask | local → **host** `queueMicrotask(flush)` (`scheduler.js:158`) | the one host reach; registers no version-1 resource (§ 1.5) |
| `ext(e)` | `core.js:416`, `:521`, `:548`, … → `:424-446` | cond | local | lazily allocates the node's cold-field object |
| `inheritId` → `getNextChildId` → `childId` → `formatId` | `core.js:381` → `owner.js:145-147`, `:137-139`, `:125-130`, `:158-161` | cond: parent has an id | local | § 3.2 |
| `isEqual` | `core.js:383`, `:530` (stored as `Ue`), invoked at `:259` | cond | local (`:575-577`) or **caller** (`t.equals`) | value comparison |
| `NotReadyError` | `core.js:219` `instanceof` | cond | builtin `instanceof` on an archive class (`error.js`) | no invocation (§ callbacks names `Symbol.hasInstance` as a form; the class defines none) |

Every helper in the `recompute` row lives in `dist/prod/core/{core,async,graph,heap,scheduler,verdict,optimistic}.js`,
and § 1.5 establishes that none of those files references a browser document
or a server runtime. The derived overload's outward effects are therefore: the
caller's compute runs (excluded by § 3.2); the caller's returned thenable, if
any, gets `then` attached (a `callbacks` item; its resolution is the caller's);
and a `flush` microtask may be scheduled. **No path performs a `create`.** The
signal/computed coming into existence and being linked under an owner are
reactive-graph facts with no `creates` standing (§ creates).

Other bundles: `dist/dev.js:5720-5729` adds `registerGraph(node, getOwner())`
(`:5727` → `:790-797`): stores the node in the owner's `_signals` dev array and
calls `DEV$1.hooks.onGraph?.(value, owner)` — an **installed hook** (`hooks`
object at `dev.js:707`); `setupComputedNode` (`:3722-3757`) adds the
`PRIMITIVE_IN_FORBIDDEN_SCOPE` diagnostic/throw (`:3725-3735`) and
`DEV$1.hooks.onOwner?.(self)` (`:3746`); the hook slot is spelled
`GlobalQueue._wireExternalSource` (`:3748`). `dist/node.cjs:4424-4432` is the
prod body (`computed` `:2655`, `setupComputedNode` `:2784-2807`, `signal` `:2809`).
Same verdict in both.

**R5 verdict: `creates` closed** for `(@solidjs/signals, createSignal)` in all
three bundles. No other domain decided: `callbacks` and `reads` are positive
on the derived overload (the compute runs tracked during the call, unless
`options.lazy` — `core.js:518`), and `writes` on the disposal/companion paths
was not undertaken.

The existing `tracked_callback_timing` reading in `solid_2.rs` (the method at
`:1302-1323`, with the doc comment above it) reaches the same conclusion, but
it is **not** cited here as corroboration: its own doc comment says it was
"Read from `@solidjs/signals@2.0.0-rc.0` `dist/dev.js`", and every line number
in it is that bundle's. Whether rc.0 and rc.3 agree on this point is not
something this audit established, and rc.0 is not one of the three archives
§ 1.1 pins.

### 7.3 R6 — `solid-js`' own `createSignal`, browser conditions

`dist/solid.js:751-753` (`dev.js:772-774`, `solid.cjs:752-754`, `dev.cjs:773-775` identical):

```js
const createSignal = (...args) => {
  return (_createSignal || createSignal$1)(...args);
};
```

| Callee | Site | Reach | Disposition | What it does |
| --- | --- | --- | --- | --- |
| `createSignal$1(...args)` | `solid.js:752`; imported `solid.js:1` `createSignal as createSignal$1` from `'@solidjs/signals'` | cond: `_createSignal` unset (no `enableHydration()` yet) | **archive** → § 7.2 (R5) | the whole of R5 |
| `_createSignal(...args)` = `hydratedCreateSignal` | `solid.js:752`; assigned by `enableHydration()` `:682-691` at `:684` | cond: `enableHydration()` was called (by `@solidjs/web`'s `hydrate`, `web.js:1160`) | local `:588-591` | `if (typeof fn !== "function" \|\| !sharedConfig.hydrating) return createSignal$1(fn, second);` else `hydrateSignalLike(createSignal$1, fn, second)` |
| `hydrateSignalLike(coreFn, fn, options)` | `solid.js:590` → `:550-581` | cond: hydrating, derived overload | local | see next rows; every branch ends in `coreFn(prev => …, options)` = R5 with a wrapper compute the export supplies |
| `markTopLevelSnapshotScope()` | `solid.js:551` → `:55-62` | cond | local | walks to the root owner, `markSnapshotScope(owner)` (`@solidjs/signals`, `core.js:62-65` region — a module-set insert) |
| `withHydrationGate(create)` | `solid.js:554`, `:566` → `:542-549` | cond: `ssrSource === "client"` or `"hybrid"` | local | `createSignal$1(false, { ownedWrite: true })` (R5), invokes the export's own arrow, `setHydrated(true)` — a **reactive write** to a signal this call created (a `writes` item of R6, not a `create`) |
| `sharedConfig.has`, `.load`, `.done` | `solid.js:559`, `:575`, `:577`, `:161`, `:311-312` | cond | hook (installed by `@solidjs/web`'s `hydrate`: `web.js:1168-1169` `id => globalThis._$HY.r[id]`, `id => id in globalThis._$HY.r`) | **host** reads of `globalThis._$HY.r`; reads only |
| `peekNextChildId(getOwner())` | `solid.js:559`, `:310` | cond | archive (`owner.js:154-156` → `childId(e, false)`) | id arithmetic |
| `hydrateSignalFromAsyncIterable(coreFn, fn, options)` | `solid.js:571` → `:308-346` | cond | local | if the serialized value is an async iterable, wraps it (`normalizeIterator` `:190`, object literals) and returns `coreFn(prev => …)` (R5) |
| `readSerializedOrCompute` / `readHydratedValue` | `solid.js:568`, `:577` → `:159-163`, `:145-158` | cond | local | reads the serialized value; `initP.then(undefined, () => {})` — **caller-shaped thenable** from the server payload; `initP.then.bind(initP)` builtin |
| `subFetch(fn, prev)` | `solid.js:162`, `:343`, `:577` → `:121-137` | cond | local → **host** | **temporarily** assigns `window.fetch = () => new MockPromise()` and `Promise = MockPromise` (`:125-126`), runs the **caller's** `fn`, and restores both in `finally` (`:134-135`). A host-global mutation that does not survive the call; registers no resource. |
| `armLiveTakeover()` | `solid.js:578` → `:169-179` | cond: serialized value is a live source | local | `createSignal$1(false)` (R5) into module `liveGate`; `onHydrationEnd(cb)` (`:66-73`) pushes the export's arrow into a module array or `queueMicrotask(callback)` (`:68`, **host**, microtask only) |
| `fn(prev)` | `solid.js:556`, `:562`, `:567`, `:575`, `:127`, `:342` | cond | **caller** | the caller's compute |

**12 rows.** Every reach outside the invocation is a read of `globalThis._$HY`,
a microtask, or a `window.fetch`/`Promise` swap that is undone before return.
The browser conditions perform **no `create`**.

### 7.4 R6 — the `node`/`worker`/`deno` condition, and why the row is withheld

`solid-js/dist/server.js:285-305` (`server.cjs:286-306` identical):

```js
function createSignal(first, second) {
  if (typeof first === "function") {
    …
    const memo = createMemo(prev => first(prev), opts);
    return [memo, () => { warnServerWrite("signal"); return undefined; }];
  }
  return [() => first, v => { warnServerWrite("signal"); return first = typeof v === "function" ? v(first) : v; }];
}
```

| Callee | Site | Reach | Disposition | What it does |
| --- | --- | --- | --- | --- |
| (plain overload) | `:301-304` | cond: first arg not a function | — | returns two closures; nothing runs; **no create** |
| `warnServerWrite` | `:297`, `:302` | later: caller invokes the setter | local `:279-284` → `console.warn` (**host**, console only) | dedup set + warning |
| `createMemo(prev => first(prev), opts)` | `:295` → `:306-395` | cond: derived overload | local | `options?.sync` → `createSyncMemo` (`:396-439`: owner + `pull()` running the **caller's** compute; no context reach except `ctx?.commitEpoch?.()` reads); else: `const ctx = sharedConfig.context` (`:310`), `createOwner` (`:311`, § 3.4), builds `comp`, and unless `ssrSource === "client"` or `lazy`, calls `update()` (`:378-379`) |
| `update()` → `run()` | `:341-367`, `:337-340` | cond | local | `resetOwnerForRerun` (`:170-174`), `runWithOwner` + `runWithObserver` (`:207-215`) around **`comp.compute(comp.value)`** — the **caller's** compute; `armDispose()` registers a disposal flag on the creating owner (`:327-336`, reactive-graph owner, a `cleanups`-shaped act); on `NotReadyError`, `subscribePendingRetry` (`:248-252`) attaches `then` to the **caller's** pending source |
| `processResult(comp, result, owner, ctx, …)` | `:349` → `:500-803` | cond: compute returned normally | local | **the finding.** For a thenable result: `recordSlot` writes into `ctx[SLOTS]` (`:541-553`, called `:557`) — the per-request render context; **`if (serializes) ctx.serialize(id, deferred.promise, deferStream);`** (`:558`) where `serializes = !!(ctx?.async && ctx.serialize && id && !noHydrate)` (`:555`); `settleServerAsync` (`:634-655` → `:254-277`, `Promise.resolve(current).then(...)`) and `ctx?.commit?.()` (`:575`, `:630`, `:646`, `:654`). For an async-iterable result: `ctx.serialize(id, deferred.promise, deferStream)` (`:699`) or `ctx.serialize(id, tapped, deferStream)` (`:760`), `ctx.hold?.()` (`:599`, `:762`), `ctx.commit()` (`:617`, `:780`). For a synchronous result with a served loading value: `ctx.serialize(id, Promise.resolve(result), deferStream)` (`:797`). For a plain synchronous result: field writes only (`:800-802`) — **no reach** |
| `recordSlot(0, undefined, deferred)` | `:557` → `:541-553` | cond: thenable/async-iterable result, `id` and `ctx` both set | local | **not** a `create` by this audit's reading, stated explicitly because it is the other candidate in the same three lines. It writes `ctx[SLOTS][id] = {s, v, d}` into the per-request render context — so it *is* a registration into a runtime outside the invocation that outlives the call — but the thing registered is an ad-hoc `{s, v, d}` slot record that **names no version-1 resource kind**, and its only consumer is a *later read of the same `id` by this same code* (`:528-539`: `const slot = id && ctx ? ctx[SLOTS]?.[id] : undefined`, then the `slot.s === 1`/`=== 2` arms), never the renderer. § creates' second load-bearing qualification — the registry must be "a runtime that acts on it" — is therefore not met: `ctx[SLOTS]` is a scratch map the primitive keeps for its own rerun, structurally the same case as § creates' "parks an ad-hoc object in a module-level binding". Only `ctx.serialize` at `:558` hands something to a consumer that is not this code. |

What `ctx.serialize` is: `sharedConfig.context` is installed by `@solidjs/web`'s
server renderer. `renderToString` (`@solidjs/web/dist/server.js:1034-1064`)
installs a context **without** `async`, so `serializes` is `false` there and
its `serialize` (`:1042-1048`) throws on any async value anyway. `renderToStream`
(`:1325-…`) installs `async: true` (`:1326`) and a `serialize(id, p, deferStream)`
(`:1383-1397`) that adds the promise to `blockingPromises`, chains
`p.then(d => serializer.write(id, d))`, or batches it into `stubBatch`, and
otherwise `serializer.write(id, p)` — i.e. it **hands the pending computation
to the response stream's hydration serializer, which later writes it into the
HTML the server sends.** That is a registration into the server runtime that
remains live after `createSignal` returns and is reached by that runtime, not
through the tuple the call handed back.

Whether that is a `create` in the settled meaning is the question this audit
cannot answer:

- **For:** it matches the definition's letter — a registration, into a server
  runtime, of something that outlives the call and is reached by the runtime.
  The nearest audited precedent is `createServerReference`'s
  `register-reference` (a `server-reference` registered with the server
  runtime, `solidjs-web--server-functions-node-server.json`).
- **Against:** the registered thing must be a *version-1 resource kind*. The
  candidates are `async-computation` (the memo's pending work) or `stream`
  (the response); neither audited `create` names either kind, and no audited
  document has ever examined a `solid-js` server-condition primitive —
  `solid-js.json` captures `browser/development` only.

  **This bullet is weaker than it looks, and the audit says so.** § creates
  requires the `create`'s `resources` to be "drawn from the version-1 resource
  vocabulary" — *not* from the set of kinds some existing `create` already
  names. That vocabulary is `ResourceKind` in
  `rust/crates/solid-reactive-ir/src/contract_semantics.rs:1167-1178`:
  `owner`, `reactive-source`, `async-computation`, `transition`, `cleanup`,
  `request`, `response`, `stream`, `server-function-reference`. Both candidate
  kinds are already in it, and `async-computation` is already declared by
  audited summaries (`createMemo`'s, and `@solidjs/web`'s `module-load`,
  `solidjs-web.json`). So "no audited `create` names either kind" is a fact
  about the corpus's size, not a bar the definition sets.

  The bullet's other half is weaker still. The one *browser* precedent does not
  register an exotic kind: `render`'s `register-delegation` registers the
  resource `browser-root`, and that resource is declared
  `{"id": "browser-root", "kind": "owner", "states": ["active", "disposed"]}`
  (`pkg/contracts/bundled/solid-v2/solidjs-web.json`, the `render` summary's
  `call.resources`) — kind **`owner`**, the same reactive-graph kind
  `createTrackedEffect`, `onSettled`, and `createEffect` declare while closing
  `creates: []`. What separates `render` from them is therefore *not* the kind;
  it is the registry (`document`) and the outliving. Which is exactly the test
  `ctx.serialize` meets.
- **Not decidable by the census rules either way:** the model gives the census
  "does any resolved target publish a `create`", and no publication exists to
  consult for `solid-js/dist/server.js`.

So R6 is **withheld**, with the reason stated as: *the `solid-js` archive's
`createSignal` differs by condition — the browser bodies perform no `create`,
and the server body's derived overload reaches `ctx.serialize(...)`
(`server.js:558`, `:699`, `:760`, `:797`), whose `create` standing needs a
**[Decision]** in `semantic-model.md` § creates before any `solid-js`
server-condition primitive can carry a row.* The owner has five options, each
of which is a change to a frozen document or to the table's shape and therefore
not taken here: (a) decide the serializer registration **is** a `create`
(naming the resource kind) — then R6 stays withheld permanently as `render` is,
and **C1** below becomes a defect in an existing row; (b) decide it is **not** a
`create` — then R6 can be granted on this audit; (c) make the table
condition-aware, which the backlog already records as the "per-condition audit"
gap; (d) make the table **guard-aware**, a strictly stronger version of (c)
(see below); (e) decide (a) but not yet which resource kind is registered,
leaving the sub-choice between `async-computation` and `stream`/`response`
open (also below).

> **Resolved after this section was drafted.** The `[Decision 2026-09-04]` this
> paragraph asks for now exists, in `semantic-model.md` § creates: "Handing a
> value to a per-request render context that writes it into the response is a
> `create`", reached on the same four terms § 7.4 walks below, with a following
> "A guarded reach still counts, and a flat row must therefore withhold" and
> "Signed off by delegation, 2026-09-04". That is **option (a) plus option
> (d)'s diagnosis**: R6 stays withheld, and the decision records that it
> withdrew `(solid-js, createEffect, Creates)` for the same guard. The rest of
> this section is left as the reasoning that was put to the owner — it is the
> review material for that decision, not a competing one.

#### What this audit recommends: option (a)

The audit's own reading of the settled wording is that **`ctx.serialize` is a
`create`**, and it recommends the owner decide (a). The four qualifications
§ creates makes load-bearing were walked one at a time, and each is met:

1. **It is the export's own act, not a caller's.** `createSignal`'s derived
   overload calls `createMemo` (`server.js:295`), whose `update()` → `run()`
   calls `processResult` at `:349`, which calls `ctx.serialize` at `:558`. Every
   frame on that path is `solid-js/dist/server.js`'s own code. The caller
   supplies only the compute and the options; § creates' "excludes anything a
   caller-supplied callable registers" does not apply.
2. **The registry is a runtime that acts on it.** `ctx` is
   `sharedConfig.context` (`server.js:310`), and on the path that matters it is
   the per-request object `renderToStream` installs at
   `@solidjs/web/dist/server.js:1325` (`async: true` at `:1326`). Its
   `serialize(id, p, deferStream)` (`:1383-1397`) does one of three things with
   a thenable: adds it to `blockingPromises` and chains
   `p.then(d => serializer.write(id, d)).catch(e => serializer.write(id, e))`
   (`:1386-1390`); batches it into `stubBatch` (`:1391-1394`); or writes it
   straight through with `serializer.write(id, p)` (`:1396`). In all three the
   value is handed to the hydration serializer that composes the HTTP response —
   a runtime that acts on it, not a scratch map. (Contrast the `recordSlot` row
   above, which fails exactly this qualification.)
3. **The registered thing is a version-1 resource kind.** What is registered is
   the memo's *pending result* — `deferred.promise` (`:558`, `:699`) or the
   tapped async-iterable (`:760`) — which is an `async-computation` in the
   version-1 vocabulary, and it lands in the response stream, which is `stream`
   / `response` in that same vocabulary. Whichever of the two the owner names,
   it is a listed kind (see the **Against** bullet's correction above).
4. **It stays live after the call and is reached by that runtime.** The promise
   sits in `blockingPromises` / `stubBatch` / the serializer after
   `createSignal` returns, and the renderer — not the tuple `createSignal`
   handed back — is what later resolves and writes it.

Nothing in the definition is left unmet, and the one difference from `render`'s
audited precedent (a server context instead of `document`) is a difference
§ creates names as equally sufficient: "a browser document **or a server
runtime**".

**The consequence the owner must accept with (a).** It is not confined to R6,
which is only *withheld*. It falsifies an **already-shipped** row: as § 0's
**C2** and the C1 paragraph below record, `(solid-js, createEffect, Creates)`
closes `creates: []` for a declaration whose `node`/`worker`/`deno` body
performs that same `ctx.serialize` registration on a reachable, type-correct
guard. Under (a) that row is unsound today and must be **withheld** until the
table can express the guard: the JSON-derived half of the pinned count moves
`24 → 23`, and R1–R5 — which are unaffected either way — add 5 on top, for 28.
Ideally the withdrawal and the grants land as two commits so the two decisions
are not entangled.

**Option (d), guard-aware publication.** (c) would key a row by `exports`
condition, which is enough for R6 and C1 as stated but not for the general
case: the `createEffect` reach is gated on *five* independent conditions
(§ C1 below), only the first of which is the resolution condition. A row shaped
`(package, export, domain, condition)` would still have to answer "no `create`"
or "withheld" for the whole `node` condition, and answering "withheld" there
discards a true negative on every non-streaming server render. A row carrying a
guard predicate — resolution condition ∧ context shape ∧ argument-supplied
option ∧ callback return shape — could publish the negative where it holds. The
cost is that the guard is only as good as the certifier's ability to *refute*
it: a tier that cannot see whether the compute returns a thenable must fail
closed, so guard-awareness buys precision only alongside a fact the IR can
actually establish. This audit records (d) as the shape to aim at, not as work
it scoped.

**Option (e), the sub-choice of kind.** Qualification 3 above admits two
answers, and they are not equivalent for downstream reasoning.
`async-computation` describes what the export registered (a pending
computation, with the `pending`/`settled`/`errored`/`cancelled` states the
vocabulary already gives that kind); `stream` (or `response`) describes what it
registered *into*, and would correlate with `@solidjs/web`'s own server
summaries rather than with `createMemo`'s declared `async-computation`. The
audit does not choose: it observes only that picking `async-computation` makes
the `create` a statement about this export, while picking `stream`/`response`
makes it a statement about the renderer the export reached, and the latter would
want `@solidjs/web`'s server document audited in the same pass.

**C1, for the owner:** the existing row `(solid-js, createEffect, Creates)`
(`solid_2.rs:456-466`, read from the `browser/development` case) has the same
exposure: `solid-js/dist/server.js:868-870` `createEffect` → `serverEffect`
(`:810-867`), which calls `processResult(comp, result, owner, ctx, …)` at
`:835` **when the caller passes a truthy `options.ssrSource`**. That one reach
is the whole of it. In particular the `ctx.block(...)` calls at `:852` and
`:859` are **not** reachable from `createEffect`: both sit inside
`if (effectFn && ctx?.async)` (`:840`), and `createEffect` is
`serverEffect(compute, undefined, options)` (`:868-870`) — `effectFn` is
`undefined` on every `createEffect` call, so the guard is false. (It is
`createRenderEffect` (`:871-873`) that forwards a real `effectFn` and can reach
`ctx.block`; that export has no row.) The `processResult` reach at `:835` stands
on its own and is enough: under option (a) the row is unsound for the `node`
condition, under (b) it is fine.

Under option (a) — the option this audit recommends below — the row is not
merely "flagged": it asserts `creates: [] closed` for
`(solid-js, createEffect)` while a reachable, type-correct call sequence makes
the export perform a `create`, so it must be **withheld**. The full guard the
row would have to be aware of is:

- the consumer resolves `solid-js` under the `node`, `worker`, or `deno`
  condition (§ 1.2), so `dist/server.js` is the running body; **and**
- `sharedConfig.context` is a `renderToStream` context, i.e. `ctx.async` is
  `true` (`@solidjs/web/dist/server.js:1325-1326`) — `renderToString`'s context
  sets no `async` (`:1034`), so `serializes` is false there; **and**
- the caller passes `options.ssrSource` as `"server"` or `"hybrid"` —
  `"client"` returns at `serverEffect`'s `:812-814` before any context is read;
  **and**
- the caller's `compute` returns a thenable or async-iterable, which is what
  selects `processResult`'s serializing arms (`:505`, `:558`; `:667`, `:699`,
  `:760`) rather than the plain-value arm (`:800-802`, no reach); **and**
- the creating owner has an `id` (`:502`) and no `NoHydrateContext` is in scope
  (`:504`) — together `serializes` at `:555`.

None of those is an unsupported call: `options.ssrSource` is *type-correct* on
`createEffect` because `solid-js` augments the core `EffectOptions` with
`HydrationSsrFields` (`solid-js/types/client/hydration.d.ts:37-46`, the
`declare module "@solidjs/signals"` block; `ssrSource?: "server" | "hybrid" |
"client"` at `:35`). And the row is genuinely keyed to the `solid-js` archive:
`solid-js/types/index.d.ts:8` exports `createEffect` from
`./client/hydration.js`, where `:568` is
`export declare const createEffect: typeof coreEffect;` — a **re-declaration**
inside `solid-js`, so § 1.2's cross-archive re-keying does not move it to
`@solidjs/signals`. The row therefore denies a `create` for exactly the
declaration whose `node`-condition body performs one.

This audit does not move the row — it is not this document's to move — but it
records the consequence as **C2** in § 0.

### 7.5 Verdict

- **R5 `(@solidjs/signals, createSignal)`: `creates` closed** in all three
  bundles (§ 7.2).
- **R6 `(solid-js, createSignal)`: withheld** — browser conditions closed
  (§ 7.3); `node`/`worker`/`deno` condition undecidable pending a semantic
  decision (§ 7.4).

---

## 8. `render` and `hydrate` — is the table's treatment consistent with the bytes?

The table carries **no** row for `render` (its summary publishes
`register-delegation`) and **withholds** `hydrate` although
`solidjs-web.json` closes `creates: []` for it (`solid_2.rs:168-183`, `:1743-1745`;
`docs/precision-backlog.md:173-188`). Both were re-read in all four browser
bundles and the server bundle.

### 8.1 `render` registers before it does anything else

| Bundle | `render` | `registerDelegatedRoot(element)` | Before `createRoot`? |
| --- | --- | --- | --- |
| `@solidjs/web/dist/web.js` | `:347-378` | `:349` | yes — `createRoot` is `:351` |
| `dist/dev.js` | `:348-385` | `:355` (after the `!element` throw `:349-351`, `resetErrorHalt()` `:352`, `enforceLoadingBoundary(true)` `:353`) | yes — `createRoot` is `:357` |
| `dist/web.cjs` | `:348-379` | `:350` | yes |
| `dist/dev.cjs` | `:349-386` | `:356` | yes |

`registerDelegatedRoot` (`web.js:398-401`, `dev.js:407-410`) →
`registerDelegatedContainer(root, root)` (`web.js:407-417`, `dev.js:416-426`):
`delegatedContainers.set(container, state)` (module `Map`, `web.js:338`),
`state.owners.set(...)`, and **`delegatedEvents.forEach(name =>
attachDelegatedEvent(name, container, state))`** (`web.js:415`) →
`container.addEventListener(name, handler)` (`web.js:431`, `dev.js:440`). So
on the same stack the element (or `document`) receives one listener per
already-delegated event; events delegated later by compiled templates
(`delegateEvents`, `web.js:389-397`) attach to every registered container at
that later time (`:394`). The registration is onto the browser document and
outlives the call until the returned disposer runs `unregisterDelegatedRoot`
(`web.js:375`). That is exactly the `register-delegation` `create` the audit
publishes, and the table's silence on `render` is correct.

### 8.2 `hydrate` reaches `render` on every path

`hydrate` has four return paths in each browser bundle; every one is a call to
`render(code, element, [...element.childNodes], options)`:

| Path | `web.js` | `dev.js` | `web.cjs` | `dev.cjs` | Timing |
| --- | --- | --- | --- | --- | --- |
| `globalThis._$HY.done` fast return | `:1162` | `:1174` | `:1163` | `:1175` | same stack |
| module-preload fulfilled continuation | `:1211` | `:1233` | `:1212` | `:1234` | later (`p.then`) |
| module-preload rejected continuation | `:1219` | `:1241` | `:1220` | `:1242` | later (`p.then` rejection arm) |
| ordinary `try { … return render(…) } finally { sharedConfig.hydrating = false }` | `:1226` | `:1248` | `:1227` | `:1249` | same stack |

The preload branch is taken only when `loadModuleAssets(rootMapping)`
(`web.js:1140-1158`) returns a pending `Promise.all` (`:1157`); otherwise
control falls through to the `try` at `:1224`. There is no path on which
`hydrate` returns without having called or scheduled `render`. Per § creates
("The claim includes such a registration performed at a later `at` event
because of this call"), the deferred arms count. **Confirmed:** the audited
`creates: []` on `hydrate` is contradicted by the bytes, and the withholding
(`WITHHELD` at `solid_2.rs:1744`) is the right state until the audit is
corrected to publish the registration as `render`'s does. The backlog's line
numbers (`dev.js:1171`, `:1174`, `:1233`, `:1241`, `:1248`, `:355`) match the
bytes read here.

Two additional observations for the eventual re-audit, neither of which changes
the verdict: `hydrate` also writes `globalThis._$HY.modules = {}` and
`globalThis._$HY.loading = {}` when absent (`web.js:1164-1165`) and populates
`sharedConfig.registry` from `element.querySelectorAll('*[_hk]')`
(`gatherHydratable`, `web.js:1602-1615`) — host-global and module state that
survive the call but name no version-1 resource kind; and `loadModuleAssets`
performs dynamic `import(entryUrl)` (`web.js:1148`) — a module load, not a
resource registration.

### 8.3 The server condition

`@solidjs/web/dist/server.js:3331` exports `notSup as render` and `notSup as
hydrate`; `notSup` (`:3183-3185`) throws. `server.cjs:3450`, `:3467` bind both
to `notSup`. No `create` on the server; a `throws`-domain fact only.

---

## 9. Proposed rows, and the citation format they need

### 9.1 What the existing row format can and cannot say

`NegativeClaimRow` (`rust/crates/solid-dialect/src/lib.rs:511-521`) carries
`citations: &'static [AuditedCitation]`, and `AuditedCitation`
(`lib.rs:493-502`) is `{ document, summary, start_byte, end_byte }` — a byte
range **inside an audited JSON document** that
`every_negative_row_citation_resolves_to_the_bytes_it_claims`
(`solid_2.rs:1754-1824`) parses as a summary object, checks for
`closed: [..."creates"...]` + `creates: []`, and binds to the export through
`entrypoints[*].cases[*].exports[export] == summary`. None of R1–R5 has a
summary: `solidjs-signals.json` audits twelve exports and these are not among
them. The format **cannot express** a citation to runtime bytes, and the
companion test `the_negative_table_is_derived_from_the_audited_documents`
(`solid_2.rs:1835-1915`) would refuse the rows outright — `shipped` must equal
`derivable − withheld` and end at `24`. (This paragraph describes the format as
it stood *before* this audit's review; the line numbers are the pre-change
file's. See the note under the title for what the tree now carries.) Inventing
a summary id, or adding a summary to an audited document, is forbidden by the
task and would be a fabricated authority in any case.

### 9.2 Minimal additive change

Turn `AuditedCitation` into an enum whose first variant is the existing struct
unchanged, and add one variant for a runtime-byte citation:

```rust
/// Exactly which audited bytes one negative row was read from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditedCitation {
    /// A summary object inside an audited contract document (the existing
    /// form; every field as before).
    Summary {
        document: &'static str,
        summary: &'static str,
        start_byte: usize,
        end_byte: usize,
    },
    /// A hand implementation census over the archive's own runtime bytes,
    /// recorded in a repository audit document. Used only where no audited
    /// summary exists for the export.
    Implementation {
        /// Repository-relative path of the audit document.
        audit: &'static str,
        /// The section heading of the audit that decides this row (as the
        /// literal heading text, so a reader can find it and a test can
        /// assert it exists).
        section: &'static str,
        /// The runtime file inside the archive, package-relative, exactly as
        /// spelled in the pinned per-file manifest.
        archive_path: &'static str,
        /// `sha256` of that whole file as recorded in
        /// `benchmarks/package-contract-v2/phase0/rc3/<archive>/files.json`.
        file_sha256: &'static str,
        /// The cited definition: `[start_byte, end_byte)` of that file.
        start_byte: usize,
        end_byte: usize,
        /// `sha256` of exactly those bytes, so the range can be re-verified
        /// against an on-disk archive without trusting the offsets.
        slice_sha256: &'static str,
    },
}
```

`NegativeClaimRow` is unchanged. The byte-reading test gains a second arm:

- **Always (no archive needed):** the `audit` file exists and contains the
  `section` heading verbatim; the pinned manifest for the row's `package`
  (`solid_2.rs` maps `@solidjs/signals` → `phase0/rc3/solidjs-signals`, etc.)
  has an entry with `path == archive_path` and `sha256 == file_sha256`;
  `start_byte < end_byte <= bytes` of that manifest entry. This binds the
  citation to the same pinned manifest the registry audit already trusts,
  without checking archive bytes into the repository.
- **When the archive is on disk:** if `SOLID_CHECKER_RC3_ARCHIVE_ROOT` names a
  `node_modules` root containing the package, read `archive_path`, assert its
  sha256 is `file_sha256`, and assert `sha256(bytes[start..end]) ==
  slice_sha256` and that the slice begins with `function <export>` or
  `const <export> = ` after an optional `*/ ` comment tail. Absent the root, the
  arm **skips** — and, mirroring `SOLID_CHECKER_EXPECT_PROBE_PINS=1`'s contract
  for the probe pins, fails loudly under that variable so a verification run
  cannot be green on a skipped slice check.

#### What the `Implementation` variant does *not* prove, said plainly

For a `Summary` citation, the test genuinely re-derives the claim: it parses the
cited bytes as a summary object and checks `closed` contains `"creates"` and
`creates` is `[]`. The row is then a *mechanical consequence* of the bytes, and
ADR 0007's sentence about it is true —

> Every row cites the document and the byte range of the summary object it was
> read from, and a test re-reads that range and re-derives the closure, so a row
> cannot drift from the bytes.
> — `docs/adr/0007-census-dialect-axiom-tier.md`, the "Decision" section **as
> it read when this review began**. This exact sentence is no longer in the
> file; see the paragraph after next.

**For an `Implementation` citation nothing re-derives anything.** There is no
machine-checkable proposition in `function createRoot(e, t) { … }` that yields
"performs no `create`"; that conclusion came from a human following a
transitive closure across seven files and an archive-wide host census (§ 1.3,
§ 1.5), and the closure is *not* in the citation. What `slice_sha256` pins is
strictly the **subject** the human read: it proves the reviewer's eyes were on
these exact bytes of this exact published file, and it will fail if a future
rc bumps the archive or the offsets slide. It does not, and cannot, prove the
verdict follows from them.

So ADR 0007's sentence as quoted above, applied to these rows, would be **false
as written**, and could not ship alongside them. **It has since been amended**
(same working tree): the ADR's "Decision" now opens with the weaker
"Every row cites exactly which audited bytes it was read from, and a test
re-establishes that citation", followed by a "Two citation kinds, and they are
not equally mechanical" section that says of `Implementation` that "the cited
range is *the definition a human read*, and the closure was that reading's
conclusion — **not** the range's content. Nothing re-derives 'this body
performs no `create`' from a JavaScript function body, and the test does not
pretend to". The quotation in the block above is therefore historical — it is
the wording this review found and objected to — and F2 in § 0 is **discharged**,
not outstanding.

Because the closure is not mechanical, the archive arm carries more weight for
these rows than it would for a summary row. Four strengthenings follow from
that. **All four are now implemented in this working tree**; each bullet below
states the requirement first and then where it landed, so the requirement
survives even if the implementation is re-cut.

- **(a) Check the 15 cited slices into the repository as small files** and
  verify `sha256(slice_file) == slice_sha256` **unconditionally**, with no
  environment variable involved. The cost is trivial: the 15 slices sum to
  **3417 bytes** (R1 382 B, R2 138 B, R3 952 B, R4 1036 B, R5 909 B, from the
  byte ranges in § 9.4), all three archives are MIT-licensed
  (`license: "MIT"` in each pinned `package.json`), and each excerpt is a few
  lines of a published tarball. The gain is that the *primary* fact — "these
  are the bytes the audit read" — becomes verifiable on a bare checkout with no
  `node_modules` anywhere, which is where every CI job that has no
  `rust/target` sits today.

  This is the one part of the plan the worktree already implements:
  `rust/crates/solid-dialect/audited-slices/solid-v2/<phase0-archive-dir>/<package-relative
  path>.<start>-<end>.slice`, with its own README stating the layout and the
  unconditional-verification rule. All 15 files are present, their lengths sum
  to exactly 3417 bytes, and each one's sha256 matches the 16-hex prefix § 9.4
  records for that citation. The naming derives the path from the citation's own
  fields, so a row and its slice cannot be named inconsistently.
- **(b) Keep the archive arm anyway, and make `make verify` actually run it.**
  (a) proves the slice files match the pinned digests; only the archive arm
  proves the *archive* still contains those bytes at those offsets, which is
  the drift the pin is for. Keep it loud under
  `SOLID_CHECKER_EXPECT_PROBE_PINS=1` as described, and have
  `scripts/verify.sh` **export `SOLID_CHECKER_RC3_ARCHIVE_ROOT` pointing at the
  tsc-oracle install** (`rust/target/tsc-oracle/v2/node_modules`, the tree this
  audit read and the one the oracle gate already materializes) alongside the
  pins it computes after `build-typefacts`. Otherwise the arm is skipped on
  every handoff run and the loud-failure switch never fires, which is the exact
  green-on-nothing shape AGENTS.md's "Known traps" warns about for the probe
  assertions.

  **Done:** `scripts/verify.sh:212-213` sets and exports
  `SOLID_CHECKER_RC3_ARCHIVE_ROOT="$PWD/rust/target/tsc-oracle/v2/node_modules"`,
  and the `Makefile` carries the same as `RC3_ARCHIVE_ENV` (`:58`) for
  `make test-rust`. The archive arm therefore runs on a handoff verification
  rather than skipping, which is what makes the loud-failure switch meaningful
  instead of decorative.
- **(c) Keep the unconditional `files.json` check** — entry exists with
  `path == archive_path`, `sha256 == file_sha256`, and
  `start_byte < end_byte <= bytes`. It is cheap, needs no archive, and it is
  what binds an `Implementation` citation to the same pinned manifest the
  registry-integrity gate already trusts. **Done**, and ADR 0007's amendment
  now lists it as one of the three unconditional checks alongside the section
  heading and the checked-in slice.
- **(d) The "slice begins with `function <export>`" assertion must tolerate a
  leading `*/ `.** This is not a hypothetical: the `prod` and `node.cjs` bundles
  keep the JSDoc block's closing delimiter on the definition's own line, so the
  `createRoot` slice literally begins `" */ function createRoot"`
  (`dist/prod/core/owner.js:298`, `dist/node.cjs:1597`), and § 3.2, § 5.2 and
  § 6.2 quote it that way. It affects **8 of the 15** slices — every `prod` and
  `node.cjs` one except `createSignal`'s two, whose definition happens to start
  a fresh line (`dist/prod/signals.js:65`, `dist/node.cjs:4424`); the three
  `dev.js` slices are unaffected because that bundle is not minified and closes
  its comment on its own line. A plain `starts_with("function createRoot")`
  would therefore fail on more than half the citations. Strip an optional
  leading `*/` and surrounding whitespace before matching. **Done:**
  `solid_2.rs:2275-2281` does `text.strip_prefix(" */ ").unwrap_or(&text)`
  before asserting `function <export>(` or `const <export> = `.

The derivation test gains a second source beside the JSON documents:

```rust
/// Rows whose authority is a hand implementation audit rather than a summary.
/// Each entry is (package, export, domain, audit document, verdict).
const IMPLEMENTATION_AUDITED: &[(&str, &str, CallClaimDomain, &str, ImplementationVerdict)] = &[…];
```

with `expected = (derivable_from_json ∪ implementation_audited_closed) −
withheld`, `withheld ⊆ derivable_from_json ∪ implementation_audited_withheld`,
and the pinned count moving in the same commit as the rows: `24 → 29` if the
owner takes option (b), or `24 → 28` under the recommended (a), which also puts
`(solid-js, createEffect)` in `WITHHELD` (§ 0 **C2**). The existing `WITHHELD`
list stays the home of `hydrate`; R6 is recorded as an implementation-audited
**withheld** entry with its reason, so the "neither shipped nor withheld"
omission check keeps working.

`census_dialect_axiom_for_callee` needs no change: it consults
`primitive_performs_no_operation(archive, export, domain)`, which reads only
`(package, export, domain)`. The citation is review material and test input,
never a runtime input — the same as today.

### 9.3 The digests the citations rely on

Every file cited in this document, its pinned digest (from
`benchmarks/package-contract-v2/phase0/rc3/*/files.json`), and confirmation
that the on-disk file under `rust/target/tsc-oracle/v2/node_modules/` hashes to
the same value. **Runtime bundles (27):**

| Archive · file | sha256 (pinned == on disk) |
| --- | --- |
| `@solidjs/signals` · `dist/prod/index.js` | `5c0a6384d330cfdf979197f0c6037bbb1db9712e3fe5d4cafb2de886dd509907` |
| `@solidjs/signals` · `dist/prod/core/owner.js` | `d86a4959cd0eca34617b14d29514d653193d5fcd87d2857c783bf04ff4aaefe7` |
| `@solidjs/signals` · `dist/prod/core/core.js` | `1726b40ebf79cf15b8d09ce2078a78a6a8ca71bf48e4b3ba12880736ae281a46` |
| `@solidjs/signals` · `dist/prod/signals.js` | `b80dc49d83f80b37a572d0fa7245866c978428e450a5f673e6e83972f9db9e8a` |
| `@solidjs/signals` · `dist/prod/core/graph.js` | `a5a646fa1e29b75fea4b44b54e6e937298bede7c09d03ae052909c4c55c7f71b` |
| `@solidjs/signals` · `dist/prod/core/heap.js` | `309a548f0e780b29ef14ba4ab88136f0ea8ef8731c3372086675ba964d781487` |
| `@solidjs/signals` · `dist/prod/core/scheduler.js` | `ab28069cf2f3e24815bdc6fe66ccfd48e685289fe3ad4287e8785b6b180d2256` |
| `@solidjs/signals` · `dist/prod/core/async.js` | `5afd8f848abe4b4b746922e92b098e3d74c3797df103a958dc9db9e0a604eeb7` |
| `@solidjs/signals` · `dist/prod/core/external.js` | `e7079dfbd068946e20808182f88fd0a64ec4dc93fe575b0d8ab50a6499994639` |
| `@solidjs/signals` · `dist/prod/core/verdict.js` | `e39196d7c1a815ae41c36cc2bb5fee7a93428674fb7ddc5ac8279d4904892076` |
| `@solidjs/signals` · `dist/prod/core/optimistic.js` | `4194a455bc4c8b8548c22ec9d0fb7fc3cad2a6543126847b6b88207f26ce1472` |
| `@solidjs/signals` · `dist/prod/core/error.js` | `635596a52d887368153be0bd2df5d277a4cc49d91c36374f4be2540280cd871a` |
| `@solidjs/signals` · `dist/prod/core/action.js` | `b8e7e270b261ce9d8c1410dfbea7dfb103557f2c48db5b139ef4e7ab04d1a270` |
| `@solidjs/signals` · `dist/dev.js` | `cc68ed0f0c5de86411555af407ac7acf4d1c10206f24bab4e1793c22553f1a79` |
| `@solidjs/signals` · `dist/node.cjs` | `bc0e35d32add395dc1c4dc3d6cd0fb4ea4a19bd582d68de3b44c708bb4b75c1c` |
| `solid-js` · `dist/solid.js` | `14af2d696eb0669c64973874601f691737aa1df359fced6dec55a523f34cfa1b` |
| `solid-js` · `dist/dev.js` | `dfc362391cbc0b069cef8b8d0d72c99d34310231a76fd66ef615533424d3ac18` |
| `solid-js` · `dist/solid.cjs` | `d155966bc29d2bf46e3cb32c8839d885933a50b60c28c6d8abd70a7ac1147333` |
| `solid-js` · `dist/dev.cjs` | `4ca1b958df30ef4b0fa9cfd17206293073846d33147ad164208805dafde22e51` |
| `solid-js` · `dist/server.js` | `63269da73b61b71fd775ef811f8ab88417c6ea6dda2de1e6f3c10d86b66fc8a8` |
| `solid-js` · `dist/server.cjs` | `2e2ed5833323d48a43454b89f03de9f31346a3549a9934d8e67b3b9fe4f231a7` |
| `@solidjs/web` · `dist/web.js` | `3eccc22880306613c83a658d5889f9b307fad4a114c8842e12b9db5ffe46bf27` |
| `@solidjs/web` · `dist/dev.js` | `d848d00341ac8195e191404ace7dd8b4c650f47befb0cfecac78ddcf01587851` |
| `@solidjs/web` · `dist/web.cjs` | `ad9d9b8b0a23e8dfc2251e44b8b3075513f3cd637d2824bb8623e6f69619dd5b` |
| `@solidjs/web` · `dist/dev.cjs` | `825e44a72c66176a44b8c33d36fd924486d03c1450151984aafbd5161c1f93f0` |
| `@solidjs/web` · `dist/server.js` | `80abb46a98a9d6695b7d2c42725ccfb538f8e941d6aa3a8ec5343d6d002d54b1` |
| `@solidjs/web` · `dist/server.cjs` | `f63841589bc8ead3784a6d97ce52de557b057b6519380e49807b4e688dc8ea68` |

**Declaration files (11)** — these are what § 1.2's declaration column and
§ 3.1 / § 4.1 / § 5.1 / § 6.1 / § 7.1 cite, and the census terminator keys a row
by the archive the *declaration* lives in, so their identity is as load-bearing
as the runtime bundles':

| Archive · file | sha256 (pinned == on disk) |
| --- | --- |
| `@solidjs/signals` · `dist/types/index.d.ts` | `e4157c4caba48476db4e7649b5a50827687c2d90c0a09fa091f0e65d0a63cfb4` |
| `@solidjs/signals` · `dist/types/core/owner.d.ts` | `40102fb1b3a833e6a6b21dd52937797f1f74ae83bae133968ea295506a3e9233` |
| `@solidjs/signals` · `dist/types/core/core.d.ts` | `a93e5f0dabb178543f77595ca4f7d3d06017adf857c9402af7107522d0065c15` |
| `@solidjs/signals` · `dist/types/signals.d.ts` | `c3dd3b2a247183379baaddf140fcdc67e3ac563a97b9af3151646a40471de068` |
| `@solidjs/signals` · `dist/types-cjs/index.d.cts` | `6097999d2de7943fbd1ae8ebe3a78d66eebcdd0eb41873dbaf09d6e6ec2e60ce` |
| `solid-js` · `types/index.d.ts` | `76b94bfb3a95099405a8cae461fff7b83c5a3cd61667cf72c23e7f850cf52740` |
| `solid-js` · `types/client/hydration.d.ts` | `b6e3310a19c5328fa1f239197f4e3eadcd325ffe8727f8e69ab7aa38b7cd144d` |
| `solid-js` · `types-cjs/index.d.cts` | `b7f712fd917b30e57d949f38d696f301effd3d10439d1ba85b8228c2826ca8fb` |
| `@solidjs/web` · `types/index.d.ts` | `5870c51be7674969670ccb084077d3df29ed732db8e8ad03527d384285c99635` |
| `@solidjs/web` · `types/client.d.ts` | `49d281ca558aa359a44bed4ce1fb9cc9b54d65cd353b1d3191ab62d350e18c13` |
| `@solidjs/web` · `types-cjs/index.d.cts` | `8957ea44a0232bd4d69536f56d498d1387b159dab8330cbdd9b2b2dd4ba292f0` |

`solid-js/types/client/hydration.d.ts` is the one declaration file a *row*
would cite rather than merely rest on: it is where both R6's `createSignal`
(`:246-253`) and the existing `createEffect` row's declaration (`:568`) live,
and where the `EffectOptions` augmentation that makes C1's `ssrSource`
type-correct is written (`:37-46`).

### 9.4 The rows, in the file's exact form under the proposed variant

Byte offsets are of the definition's line range in the pinned file
(start of first line to start of the line after the last); `slice_sha256` is
of exactly those bytes. Rows are listed in the `(package, export, domain)`
order `negative_rows_are_sorted_unique_and_canonical` requires; the insertion
points into `NEGATIVE_ROWS` are after `createProjection` (R1, R5), after
`flush` (R2), after `flush`/`getOwner` (R3), and last among `@solidjs/signals`
(R4).

```rust
    // R1 — docs/package-contract-v2/audits/2026-09-04-solid-2-rc3-core-primitives-creates.md § 3
    NegativeClaimRow {
        package: "@solidjs/signals",
        export: "createRoot",
        domain: CallClaimDomain::Creates,
        citations: &[
            AuditedCitation::Implementation {
                audit: "docs/package-contract-v2/audits/2026-09-04-solid-2-rc3-core-primitives-creates.md",
                section: "## 3. `createRoot` — archive `@solidjs/signals@2.0.0-rc.3`",
                archive_path: "dist/prod/core/owner.js",
                file_sha256: "d86a4959cd0eca34617b14d29514d653193d5fcd87d2857c783bf04ff4aaefe7",
                start_byte: 10535,
                end_byte: 10655,
                slice_sha256: "eefc749ba75a1917…", // full digest at wiring time
            },
            AuditedCitation::Implementation {
                audit: "docs/package-contract-v2/audits/2026-09-04-solid-2-rc3-core-primitives-creates.md",
                section: "## 3. `createRoot` — archive `@solidjs/signals@2.0.0-rc.3`",
                archive_path: "dist/dev.js",
                file_sha256: "cc68ed0f0c5de86411555af407ac7acf4d1c10206f24bab4e1793c22553f1a79",
                start_byte: 90641,
                end_byte: 90783,
                slice_sha256: "9a66667de7c1a48f…",
            },
            AuditedCitation::Implementation {
                audit: "docs/package-contract-v2/audits/2026-09-04-solid-2-rc3-core-primitives-creates.md",
                section: "## 3. `createRoot` — archive `@solidjs/signals@2.0.0-rc.3`",
                archive_path: "dist/node.cjs",
                file_sha256: "bc0e35d32add395dc1c4dc3d6cd0fb4ea4a19bd582d68de3b44c708bb4b75c1c",
                start_byte: 58592,
                end_byte: 58712,
                slice_sha256: "eefc749ba75a1917…",
            },
        ],
    },
    // R5 — § 7.2
    NegativeClaimRow {
        package: "@solidjs/signals",
        export: "createSignal",
        domain: CallClaimDomain::Creates,
        citations: &[
            AuditedCitation::Implementation { audit: "…", section: "## 7. `createSignal` — two archives, two rows",
                archive_path: "dist/prod/signals.js",
                file_sha256: "b80dc49d83f80b37a572d0fa7245866c978428e450a5f673e6e83972f9db9e8a",
                start_byte: 2517, end_byte: 2797, slice_sha256: "ce6ade13e9e77463…" },
            AuditedCitation::Implementation { audit: "…", section: "## 7. `createSignal` — two archives, two rows",
                archive_path: "dist/dev.js",
                file_sha256: "cc68ed0f0c5de86411555af407ac7acf4d1c10206f24bab4e1793c22553f1a79",
                start_byte: 232899, end_byte: 233248, slice_sha256: "d1292c1a9d7a916d…" },
            AuditedCitation::Implementation { audit: "…", section: "## 7. `createSignal` — two archives, two rows",
                archive_path: "dist/node.cjs",
                file_sha256: "bc0e35d32add395dc1c4dc3d6cd0fb4ea4a19bd582d68de3b44c708bb4b75c1c",
                start_byte: 175815, end_byte: 176095, slice_sha256: "d003a64c857bac06…" },
        ],
    },
    // R2 — § 4
    NegativeClaimRow {
        package: "@solidjs/signals",
        export: "getOwner",
        domain: CallClaimDomain::Creates,
        citations: &[
            AuditedCitation::Implementation { audit: "…", section: "## 4. `getOwner` — archive `@solidjs/signals@2.0.0-rc.3`",
                archive_path: "dist/prod/core/owner.js",
                file_sha256: "d86a4959cd0eca34617b14d29514d653193d5fcd87d2857c783bf04ff4aaefe7",
                start_byte: 7691, end_byte: 7739, slice_sha256: "67fcbebd02b9e57f…" },
            AuditedCitation::Implementation { audit: "…", section: "## 4. `getOwner` — archive `@solidjs/signals@2.0.0-rc.3`",
                archive_path: "dist/dev.js",
                file_sha256: "cc68ed0f0c5de86411555af407ac7acf4d1c10206f24bab4e1793c22553f1a79",
                start_byte: 87147, end_byte: 87189, slice_sha256: "e8cb95b765807fa1…" },
            AuditedCitation::Implementation { audit: "…", section: "## 4. `getOwner` — archive `@solidjs/signals@2.0.0-rc.3`",
                archive_path: "dist/node.cjs",
                file_sha256: "bc0e35d32add395dc1c4dc3d6cd0fb4ea4a19bd582d68de3b44c708bb4b75c1c",
                start_byte: 55747, end_byte: 55795, slice_sha256: "67fcbebd02b9e57f…" },
        ],
    },
    // R3 — § 5
    NegativeClaimRow {
        package: "@solidjs/signals",
        export: "onCleanup",
        domain: CallClaimDomain::Creates,
        citations: &[
            AuditedCitation::Implementation { audit: "…", section: "## 5. `onCleanup` — archive `@solidjs/signals@2.0.0-rc.3`",
                archive_path: "dist/prod/signals.js",
                file_sha256: "b80dc49d83f80b37a572d0fa7245866c978428e450a5f673e6e83972f9db9e8a",
                start_byte: 2368, end_byte: 2421, slice_sha256: "89ddda3041ae8017…" },
            AuditedCitation::Implementation { audit: "…", section: "## 5. `onCleanup` — archive `@solidjs/signals@2.0.0-rc.3`",
                archive_path: "dist/dev.js",
                file_sha256: "cc68ed0f0c5de86411555af407ac7acf4d1c10206f24bab4e1793c22553f1a79",
                start_byte: 231953, end_byte: 232799, slice_sha256: "f20080340dc75986…" },
            AuditedCitation::Implementation { audit: "…", section: "## 5. `onCleanup` — archive `@solidjs/signals@2.0.0-rc.3`",
                archive_path: "dist/node.cjs",
                file_sha256: "bc0e35d32add395dc1c4dc3d6cd0fb4ea4a19bd582d68de3b44c708bb4b75c1c",
                start_byte: 175666, end_byte: 175719, slice_sha256: "89ddda3041ae8017…" },
        ],
    },
    // R4 — § 6
    NegativeClaimRow {
        package: "@solidjs/signals",
        export: "untrack",
        domain: CallClaimDomain::Creates,
        citations: &[
            AuditedCitation::Implementation { audit: "…", section: "## 6. `untrack` — archive `@solidjs/signals@2.0.0-rc.3`",
                archive_path: "dist/prod/core/core.js",
                file_sha256: "1726b40ebf79cf15b8d09ce2078a78a6a8ca71bf48e4b3ba12880736ae281a46",
                start_byte: 24547, end_byte: 24827, slice_sha256: "520000b7e8781162…" },
            AuditedCitation::Implementation { audit: "…", section: "## 6. `untrack` — archive `@solidjs/signals@2.0.0-rc.3`",
                archive_path: "dist/dev.js",
                file_sha256: "cc68ed0f0c5de86411555af407ac7acf4d1c10206f24bab4e1793c22553f1a79",
                start_byte: 155777, end_byte: 156253, slice_sha256: "2a929c2683ae93a3…" },
            AuditedCitation::Implementation { audit: "…", section: "## 6. `untrack` — archive `@solidjs/signals@2.0.0-rc.3`",
                archive_path: "dist/node.cjs",
                file_sha256: "bc0e35d32add395dc1c4dc3d6cd0fb4ea4a19bd582d68de3b44c708bb4b75c1c",
                start_byte: 112365, end_byte: 112645, slice_sha256: "0ac3b93214231630…" },
        ],
    },
```

The `slice_sha256` values above are the first 16 hexadecimal digits of the
digests computed while drafting; the wiring commit must carry the full 64 and
re-derive every one from the archive under `SOLID_CHECKER_RC3_ARCHIVE_ROOT`.
The `start_byte`/`end_byte` pairs are likewise the draft's, and the same
re-derivation must confirm them — these 15 ranges sum to 3417 bytes, which is
the figure § 9.2 (a) uses when it proposes checking the slices themselves into
the repository.
The `solid-js/dist/server.js` bodies read for § 1.4 (`:91-93`, `:97-102`,
`:175-178`, `:1392-1394`; file digest `63269da7…`) are **not** citations of
these rows — the rows are about `@solidjs/signals` — but the wiring commit
should record them in the `IMPLEMENTATION_AUDITED` entries' reasons, because
the rows' soundness for a `solid-js` import under `node` depends on them.

**R6** is recorded as:

```rust
    // docs/package-contract-v2/audits/2026-09-04-solid-2-rc3-core-primitives-creates.md § 7.4
    ("solid-js", "createSignal", CallClaimDomain::Creates,
     "docs/package-contract-v2/audits/2026-09-04-solid-2-rc3-core-primitives-creates.md",
     ImplementationVerdict::Withheld(
        "browser conditions perform no create; node/worker/deno condition reaches \
         ctx.serialize(id, deferred.promise, deferStream) (dist/server.js:558, :699, :760, :797) \
         whose `create` standing needs a § creates decision",
     )),
```

---

## 10. What this audit did not decide, exactly

- **R6 `(solid-js, createSignal)`** — withheld; needs the § 7.4 decision.
- **`(solid-js, createRoot|getOwner|onCleanup|untrack)`** as rows keyed to
  `solid-js`: not proposed, because no `solid-js` import resolves its
  declaration into the `solid-js` archive for these names (§ 1.2). If the tier
  is ever changed to key on the written entrypoint's own binding (the
  alternative the backlog names under "Four rows dead via cross-archive
  re-export"), the § 1.4 readings of `solid-js/dist/server.js` and the
  browser re-exports are what those rows would cite.
- **Any domain other than `creates`** as a row. § 4.4, § 5.5, § 6.4 record
  which sibling closures were decidable for `getOwner`, `onCleanup`, and
  `untrack`; none was decidable for `createRoot` or `createSignal` at this
  rigor (the disposal path's companion snap is a reactive write the audit did
  not chase to a `writes` verdict; `callbacks` and `reads` are positive on the
  derived overload).
- **`@solidjs/web`'s `browser/production` condition for the existing rows** —
  unchanged; the per-condition gap the backlog records stays open. This audit
  did read `web.js` (production) for § 8, so `render`/`hydrate` are now
  confirmed on that condition too.
- **The protocol-method reach** ADR 0008 lists as an open producer-side gap
  (`toJSON`, `Symbol.iterator`, element `toString`, `then` on a thenable) was
  handled here by reading: the derived `createSignal` overload's
  `handleAsync` and `solid-js/dist/server.js`'s `processResult` do call `then`
  on the caller's returned value, which this audit dispositions as **caller**
  (§ 3.2); a machine census would refuse there, and rightly so until the
  producer classifies the form.
- **Hooks.** Dev builds invoke `DEV.hooks.onOwner`, `DEV.hooks.onGraph`, and
  diagnostic listeners; prod and dev invoke `GlobalQueue._externalUntrack` /
  `_wireExternalSource` when `enableExternalSource` installed one. Their bodies
  are not these exports' behavior, so `creates` is unaffected; a future
  `callbacks` audit must account for every one of them (§ 5.5 shows one that
  already blocks a cross-condition closure).
- **`solid-js/dist/server.js`'s other derived primitives** (`createMemo`,
  `createStore(fn, …)`, `createProjection`, `createOptimistic(fn)`,
  `createEffect` and `createRenderEffect` with `ssrSource`) share the § 7.4
  `ctx.serialize` reach. Only `createEffect` has an existing row (C1/C2); the
  rest have none and this audit proposes none. `createRenderEffect` is worth a
  separate note if a row is ever wanted for it: unlike `createEffect` it
  forwards a real `effectFn` to `serverEffect` (`:871-873`), so it reaches
  `ctx.block(...)` (`:852`, `:859`) as well as `processResult` — a second
  context reach this audit did not classify.
- **Whether the `createEffect` row is actually withdrawn.** § 0's **C2**
  records the consequence of the recommended option (a) and the count move
  `24 → 23`, but withdrawing a shipped row is the owner's act, not this
  audit's, and it needs the § 7.4 **[Decision]** first.
