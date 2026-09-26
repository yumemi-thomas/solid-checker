# Audit: `@solidjs/signals@2.0.0-rc.9` — five `creates` rows

Date: 2026-09-26. Status: **for the repository owner's review**, as the rc.3
and rc.6 audits it follows were. It establishes, on the exact bytes of
`@solidjs/signals@2.0.0-rc.9`, the five `creates` rows that
[`2026-09-26-solid-2-rc9-vocabulary-review.md`](2026-09-26-solid-2-rc9-vocabulary-review.md)
§ 7 named as cheap: `getOwner`, `onCleanup`, `createRoot`, `untrack` and
`runWithOwner`. Nothing else of rc.9 was read, so no other rc.9 row exists.

**Why it exists.** Rows are archive-scoped
(`rust/crates/solid-dialect/src/solid_2.rs`, `NegativeClaimRow::version`), and
no rc.9 archive was listed, so no row answered for rc.9: certification of an
rc.9 tree (solid-primitives' `next`) lost closures rather than gaining wrong
ones (review § 7). This audit lists the rc.9 archive and grants the five rows
it read. It changes no row of rc.3 or rc.6.

**Why it is a new reading, not a carry-over.** rc.9's packaging changed
(review § 2.2): there is no CommonJS bundle and no `require` condition; the
development build is split into `dist/dev.js` and `dist/dev-shared.js`; and a
new `observe` condition selects a third build, `dist/observe/**`, that the rc.6
audit never had to cover. Every row below was read on rc.9's own bytes, in all
three builds rc.9's `exports` map can select, by the method of
[`2026-09-25-solid-2-rc6-signals-negative-rows.md`](2026-09-25-solid-2-rc6-signals-negative-rows.md)
group A, which follows
[`2026-09-04-solid-2-rc3-core-primitives-creates.md`](2026-09-04-solid-2-rc3-core-primitives-creates.md)
§ 1.3 and
[`2026-09-23-solid-2-rc3-owner-and-context-creates.md`](2026-09-23-solid-2-rc3-owner-and-context-creates.md)
§ 1. It decides only what `semantic-model.md` § creates lets it decide: whether
one invocation performs a published `create`, meaning it **registers a
version-1 resource into a runtime outside this invocation** (a browser document
or a server runtime). Reactive nodes, owners and computations coming into
existence are not `create` operations, and neither is anything a
caller-supplied callable does. A guarded reach still counts
(`[Decision 2026-09-04]`).

**Identity.** `@solidjs/signals@2.0.0-rc.9`,
`sha512-o3pqiTgpH5NR2DstiKrt9s/6+0YOFtv+MfvLONwLsS247I+EWMMyTu9BkRcgd35UR5Pa1DM16lI1/5uaIMY6Gw==`,
`package.json` sha256
`c612461c9264f2b3509ced91ea91019ed7b1ea0df0d64bba7f0f30c8d00bb1c6`. Unlike rc.6,
the integrity **was re-derived**:

- solid-primitives' `pnpm-lock.yaml` (line 4881) records that integrity;
- `npm view @solidjs/signals@2.0.0-rc.9 dist` reports the same integrity,
  shasum `2140db06574f917ec35ce590f3420bd37a05c9b5`, 119 files, gitHead
  `9a29b1a07aa3e06ee32afd1fc4c18414b4a558bb`;
- `npm pack @solidjs/signals@2.0.0-rc.9` downloaded the 723,305-byte tarball;
  its SHA-512 (`openssl dgst -sha512 -binary | base64`) is the integrity above,
  its SHA-1 is the registry shasum, and its SHA-256 is
  `661548ddec851f8d37ed82d571d97673d9ca7d2d053c1e3430baf8b0c0673e16`;
- `diff -r` of the extracted `package/` against the lockfile install the review
  read (`sweep/consumers/solid-primitives/node_modules/.pnpm/@solidjs+signals@2.0.0-rc.9`)
  reports no difference, so the install *is* the tarball.

The per-file digests of all 119 files are pinned at
`benchmarks/package-contract-v2/phase0/rc9/solidjs-signals/files.json`, beside
the verbatim `package.json`, its `exports.json`, and `tarball.json` (the record
of the check above). In the readings below `<rc9>/` is that tree.

**Performed** by one reader. Every citation in § 6 was computed from the tarball
bytes and re-read from the lockfile install, and the slicing script was first
run on the installed rc.3 tree, where it reproduced the rc.3 and rc.6 slice
digests the table already carries for these five exports.

## 0. For the owner's sign-off

| # | row | verdict | the one claim, or the reason it is withheld |
| --- | --- | --- | --- |
| 1 | `getOwner` `creates` | **GRANT** | `return context` in `dist/prod/core/owner.js:211-213`, `dist/dev-shared.js:2847-2849` and `dist/observe/core/owner.js:213-215`. No invoking form. All three slices are byte-identical to rc.6's. |
| 2 | `onCleanup` `creates` | **GRANT** | Prod and observe run `return cleanup(e)`, which stores the caller's function on the current owner's disposal field (`owner.js:219-223` / `:221-225`). Dev (`dist/dev.js:2174-2203`) adds only diagnostics: `reportDiagnostic(emitDiagnostic(...))` (console output, installed listeners, the caller-installed footer hook and its console microtask) or `emitDiagnostic` + `throw`. A `cleanups` item on a reactive-graph owner, not a `create`. |
| 3 | `createRoot` `creates` | **GRANT** | `createOwner` (owner literal and parent link) + `runWithOwner` around the caller's `init`, in all three builds. The later dispose path invokes only caller-registered callables and archive hooks, and its only host reach is `schedule()` → `queueMicrotask(flush)` or the halted `console.error`. Prod and dev slices equal rc.6's; observe differs only in mangled locals. |
| 4 | `untrack` `creates` | **GRANT** | Toggles the module `tracking` flag (dev: also `strictRead`) around the caller's `fn`, or calls the external-source hook `enableExternalSource` installed (`GlobalQueue.Yt` / `.In` / `._externalUntrack` → the caller's `config.untrack`). Prod and observe differ from rc.6 only in the hook slot's mangled name; dev is byte-identical. |
| 5 | `runWithOwner` `creates` | **GRANT** | Saves and sets `context` and `tracking` around the caller's `fn` and restores them in `finally`. Prod and observe are byte-identical to rc.6's slice. Dev adds one disposed-owner branch whose only effect is `reportDiagnostic(emitDiagnostic(...))`: console output and installed listeners. |

### What the owner is asked to agree with beyond the table

1. **The three builds are the whole runtime surface.** rc.9's `.` entry has
   `test`, `development`, `observe` and `default` (§ 0.2). `default` covers a
   `require` or `node` consumer too, because there is no other arm. The
   `./attribution` subpath is a different entry and exports none of the five.
2. **rc.9's new `globalThis` touches are not `create` operations, and none is
   reached by the five.** `setAttributionHooks` writes the installed
   attribution engine to `globalThis[Symbol.for("@solidjs/signals/observe/attribution")]`,
   the records channel parks a listener registry on
   `globalThis[Symbol.for("@solidjs/signals/observe/records")]` at module
   initialization, and `haltReactivity` hands an uncaught cause to
   `globalThis.reportError` (§ 0.3). The first two register the archive's own
   objects in a realm-global slot keyed by the archive's own name, which
   nothing outside the archive's copies acts on; the third reports an error.
   None is a version-1 resource kind. None is reachable at the call event of
   the five, or on `createRoot`'s dispose path.
3. **The `solid-js` pairing.** `solid-js@2.0.0-rc.9` re-exports `getOwner`,
   `onCleanup`, `untrack` and `runWithOwner` from this archive
   (`types/index.d.ts:1`), so a `solid-js` import of those names resolves its
   declaration here while its `node`/`worker`/`deno` conditions run
   `solid-js`'s own server bodies. Those were read in all three server builds
   rc.9 ships (§ 1.3, § 2.3, § 4.3, § 5.3) and reach the same verdicts. So do
   `solid-js@2.0.0-rc.3`'s pinned server bodies, which the rc.3 audit read and
   whose `^2.0.0-rc.3` range admits rc.9. In rc.9, `createRoot` is `solid-js`'s
   own declaration (`types/client/hydration.d.ts:372`), so a `solid-js`
   `createRoot` import does not resolve into this archive and this row does
   not answer for it. A `solid-js` other than rc.3 or rc.9 is not covered.

### What this audit does not do

- It grants nothing beyond the five. In particular `createSignal` `creates` is
  **not** granted on rc.9: its closure (`computed`, `recompute`) was rewritten
  (review § 7). So the scoped `solid-js@2.0.0-rc.3` `createSignal` row, whose
  delegates are `@solidjs/signals` `createSignal` and `getOwner` `creates`,
  cannot bind beside rc.9: the census refuses it with "whose audit carries no
  every-condition row denying it". The dialect test now names that gap
  explicitly (`DELEGATE_GAPS` in `solid_2.rs`) rather than requiring every
  audited archive to answer every delegate.
- It does not read `reads` or any other domain of the five.
- It adds rc.9 to no install pin. `SOLID_SIGNALS_RELEASES`
  (`scripts/ecosystem-benchmark/lib/runtime-pins.mjs`) still refuses a probe
  that names rc.9 (§ 7).

---

## rc.9 audit — `getOwner`, `onCleanup`, `createRoot`, `untrack`, `runWithOwner` (`creates`)

Result: **all five rows GRANT.**

---

### 0. Inputs, identity, and the archive-wide bound

#### 0.1 Files the citations and walks rest on

sha256 of each whole file, from `files.json` (every one equal in the tarball and
the install):

| File | sha256 | bytes |
| --- | --- | --- |
| `package.json` | `c612461c9264f2b3509ced91ea91019ed7b1ea0df0d64bba7f0f30c8d00bb1c6` | 2,356 |
| `dist/prod/index.js` | `aec0c86a89ff4f7fb8748e38db842ab00bcc4409329b1a8977aab4ea7fbce0ed` | 2,444 |
| `dist/prod/core/owner.js` | `a6b4d87b97f2d8021224d343a28bccf77ef2a9be8ba6872d91cfaa8b29dfc36e` | 12,066 |
| `dist/prod/core/core.js` | `4baa2f64e47621423c0246529d3ce1b56ef82a8274aa54dfc1d345141158d53d` | 91,465 |
| `dist/prod/signals.js` | `d1a61ff0872b42987400100413bdb69e83e1e8e67dad175cf1178c7fb34646b6` | 32,262 |
| `dist/prod/core/scheduler.js` | `ac77e8c1c6b44a943310bb3976d41b2e66cdd58a8805dad70d7c5e7c91039cde` | 69,197 |
| `dist/prod/core/graph.js` | `a7eefada760b83e0fd3ac2b0893f301c8ac11048bd72a534959c9e5fcb2ebafa` | 6,209 |
| `dist/prod/core/heap.js` | `1f64643053ca8e50defbeb9bb483c3ffbe5e35faba069b8fcf2ce4cbde33b024` | 5,390 |
| `dist/prod/core/external.js` | `5a316b10a1f37b7de1c0fd173b2e7ec1e0b14465099640197fb7c52295889ce2` | 3,421 |
| `dist/prod/core/verdict.js` | `518c2e0be1f71c9571e5e619299ee8f2f28423489751243300a4aac9a5c5f88c` | 30,341 |
| `dist/prod/store/next/store.js` | `ebbd50893d1c21fc259f61be7dca1659d9479485d4839fb2210fcf51e655479f` | 110,510 |
| `dist/dev.js` | `f08c227c5c64baad8c7bf67acfadc1ed07d0027de562c7343370105ad18f2120` | 418,037 |
| `dist/dev-shared.js` | `70b88ba97dcb1107878ccc161cd00651b3cff09d7e17aee5443f1cbf9689463e` | 295,301 |
| `dist/observe/index.js` | `52ee1b4037b0d10c15b90aa551d564f03f26bbc7dad1149d8c4ae23d586a8df7` | 2,499 |
| `dist/observe/core/owner.js` | `c384c5ab163cb53e1a611ce76bf9e2b27c8a7e7884d7a9c4ec7dfebf0f5e4126` | 12,112 |
| `dist/observe/core/core.js` | `5b2dba3ad755fce3a52b6d1a788dd3db9b720bd03193cc16bcd909958208adbe` | 93,186 |
| `dist/observe/signals.js` | `6a338c1513530b9c423c215723b171fda5b23f47a010037f845fd3c467ad41b5` | 32,402 |
| `dist/observe/core/scheduler.js` | `3abec8dc70d0ed2834a5b7040ea0e28b0af867c4201ecb4c90b46581020327ca` | 71,016 |
| `dist/observe/core/graph.js` | `ef10edb3a18550952d081f46cec9c33e690254c86487bc44c0ebecb5abbd5686` | 6,209 |
| `dist/observe/core/heap.js` | `30a03fc08438d31de4bc0455194c3c7e1c2d87207b5657e3a85371928cb86c58` | 5,390 |
| `dist/observe/core/external.js` | `f6b42050154b6d18dcb6003d4579a560ead8d2344d1d9615450b0c79df3063cb` | 3,421 |
| `dist/observe/core/verdict.js` | `0ceb6a716af7d6892a3b832f1966ec14e5876b4ca3c70ee7133b1e2ab8977992` | 30,341 |
| `dist/observe/core/dev.js` | `5dd639780e431e11b09a4839dc5c08192a5d3dfcceebe1bdcd582141e688fd57` | 11,292 |
| `dist/observe/core/attribution-hooks.js` | `3d7e642cdd340d325e6774a34f604143f81d236bed6455f84d5e92b4ed697bc4` | 3,221 |
| `dist/types/index.d.ts` | `b7a04c91f3aaaa529c56e5262aa6a5dbf177eda969cb653d73d8317891567356` | 2,151 |

#### 0.2 Which build each condition selects

`package.json` `exports["."]` is, in order:

| Condition | Runtime file |
| --- | --- |
| `test` | `dist/dev.js` |
| `development` | `dist/dev.js` |
| `observe` | `dist/observe/index.js` |
| `default` | `dist/prod/index.js` |

`main`, `module`, `unpkg` and `jsdelivr` are all `./dist/prod/index.js`. There
is no `require`, `node`, `browser` or `import` arm, so every other consumer
lands on `default`. The census test
`census_dialect_axiom_binds_rc9_signals_on_its_own_rows` replays that selection
over the pinned manifest.

The bindings, build by build:

- **`default`.** `dist/prod/index.js:3` re-exports `runWithOwner` and `untrack`
  from `./core/core.js`, `:7` re-exports `createRoot` and `getOwner` from
  `./core/owner.js`, and `:27` re-exports `onCleanup` from `./signals.js`.
- **`observe`.** `dist/observe/index.js:3`, `:7` and `:29`, the same shape over
  `dist/observe/**`. It also imports `./core/dev.js` for the `OBSERVE` object
  (module initialization, § 0.3).
- **`test`/`development`.** `dist/dev.js` imports `getOwner` (`:9`),
  `cleanup` (`:5`), `runWithOwner` (`:108`), `createRoot` (`:114`), `untrack`
  (`:120`), `reportDiagnostic` (`:89`) and `emitDiagnostic` (`:90`) from
  `./dev-shared.js`, defines `onCleanup` itself (`:2174-2203`), and lists all
  five un-aliased in its final `export { … }` (`:9504`, `:9514`, `:9527`,
  `:9534`, `:9546`). `dist/dev-shared.js` defines each of the other four once.

Each name has exactly one `function` definition per build.

**Declarations.** `dist/types/index.d.ts:1` exports `createRoot`,
`runWithOwner`, `getOwner` and `untrack` from `./core/index.js`, and `:12`
exports `onCleanup` from `./signals.js`. The definitions are
`core/owner.d.ts:61` `getOwner(): Owner | null`, `core/owner.d.ts:121`
`createRoot<T>(init…, options?)`, `core/core.d.ts:105`
`untrack<T>(fn, strictReadLabel?)`, `core/core.d.ts:364`
`runWithOwner<T>(owner, fn)` and `signals.d.ts:41` `onCleanup(fn: Disposable)`.

#### 0.3 Archive-wide host-boundary census of `@solidjs/signals@2.0.0-rc.9`

This is the rc.6 audit's § 0.3, redone on rc.9 with a tokenizer rather than a
line-prefix scan (the rc.6 audit found that a line-prefix scan misses code
written after `*/` on the same line). Comments were stripped by a lexer that
respects string and template literals; every remaining identifier-boundary
occurrence was listed and mapped to its enclosing top-level definition.

**Scope.** Every `.js` file under `dist/` — `dist/prod/**`, `dist/observe/**`,
`dist/dev.js`, `dist/dev-shared.js`, and the `./attribution` entry files
(`dist/dev.attribution.js`, `dist/prod/attribution.js`,
`dist/observe/attribution.js`, `dist/observe/core/attribution*.js`) — for
`document`, `window`, `navigator`, `globalThis`, `addEventListener`,
`removeEventListener`, `queueMicrotask`, `setTimeout`, `setInterval`,
`clearTimeout`, `clearInterval`, `requestAnimationFrame`,
`requestIdleCallback`, `MessageChannel`, `process`, `performance`,
`localStorage`, `sessionStorage`, `fetch`, `Promise`, `Date`, `self`,
`console`, `WeakRef`, `FinalizationRegistry`, `XMLHttpRequest`, `WebSocket`,
`Worker`, `BroadcastChannel`, `structuredClone`, `postMessage`,
`setImmediate`, `sharedConfig`, `_$HY`, `eval`, `Function`, `Symbol.for`,
`reportError`, `dispatchEvent`, `CustomEvent`, `EventTarget` and
`import.meta`, and for every non-relative import.

**What the archive does not contain:**

- No import leaves the archive. The two non-relative hits
  (`dev.attribution.js:1280`, `observe/core/attribution.js:1250`) are the text
  `from "@solidjs/signals/attribution"` inside a diagnostic message string.
- There is no `document` object. Every `window` hit is a local variable
  (`dev.attribution.js:461-539`, the hot-scope window of `checkHotRuns`) or
  message text; every `self` hit is a parameter name (`disposeChildren`'s
  `self`, `setupComputedNode`'s `self`, …).
- There is no network API (the `fetch` hits are message text), no
  `sharedConfig`, no `_$HY`, and no dynamic code.

**Every code reference that does leave the archive** (prod · observe · dev):

| Site | Enclosing definition | What it touches | Reached by the five? |
| --- | --- | --- | --- |
| `prod/core/scheduler.js:286` · `observe/core/scheduler.js:289` · `dev-shared.js:1304` `queueMicrotask(flush)` | `schedule()` | a one-shot microtask draining the archive's own queues | yes, `createRoot`'s dispose path (§ 3); registers no version-1 resource |
| `scheduler.js:352` · `:355` · `dev-shared.js:1378` `console.error` | `notifyHalted()`, the halted branch of `schedule()` | console only | same as the row above |
| `scheduler.js:343-344` · `:346-347` · `dev-shared.js:1370-1371` `globalThis.reportError`, `console.error` (**`reportError` new in rc.9**) | `haltReactivity()` | the host's uncaught-error channel and the console | no: its callers are `runEffect` and the error paths in `core/effect.js` and `boundaries.js`, which run at flush for effects other invocations registered |
| `dev-shared.js:554-556` `queueMicrotask(() => console.warn(footer))` | `emitDiagnostic()` (dev), only when a `consoleFooter` hook is installed and the entry is `severity: "error"` | a microtask that writes to the console | yes, dev `onCleanup` and dev `createOwner` on their throwing branches; console only |
| `dev-shared.js:608` `console.error`/`console.warn` | `reportDiagnostic()` (dev); the observe build's `reportDiagnostic` is an empty function (`observe/core/dev.js:218-220`) | console only; may pass an element a rendering runtime stamped on the subject (`_devElement`) as a second console argument | yes, dev `onCleanup` (no owner) and dev `runWithOwner` (disposed owner) |
| `dev-shared.js:296` · `observe/core/attribution-hooks.js:14` `globalThis[Symbol.for("@solidjs/signals/observe/attribution")] = hooks` (**new in rc.9**) | `setAttributionHooks()` | the installed attribution engine, mirrored into a realm-global slot keyed by the archive's own name | no: reached only through `OBSERVE.attribution.install` and the `./attribution` entry's `enable`/`disable` |
| `dev-shared.js:416-442` · `observe/core/dev.js:59-91` `globalThis[Symbol.for("@solidjs/signals/observe/records")]`, `console.error` (**new in rc.9**) | `recordsChannel()`, called once while building the `OBSERVE` object at module initialization | a listener registry shared by copies of the archive; a throwing listener is reported to the console | no: module initialization, not a call event |
| `dev-shared.js:801` · `observe/core/invariants.js:30` · `prod/core/invariants.js:28` `globalThis.process?.env?.COMPANION_CENSUS` | `devCheckFlushStart` / `devCheckActiveOverrides` | an environment read at flush | no |
| `dev.js:1741`, `observe/core/effect.js:103`, `prod/core/effect.js:99` `console.error` | `runEffect` | console at flush | no |
| `dev.js:2131`, `*/core/error-hooks.js:67` `console.error` | `reportClientError` | console, from error hooks at flush | no |
| `dev.js:1985`, `*/core/action.js:95,99` `new Promise` | `action` | a promise handed to `action`'s caller | no |
| `dev.js:2459`, `*/signals.js:294,297` `queueMicrotask` | `MicrotaskQueue` / `createReaction` | a microtask | no |
| `dev.js:2489,2609-2610,2732-2778`, `*/signals.js:319-585` `Promise`, `queueMicrotask`, `setTimeout`, `clearTimeout` | `resolve`, `refresh`, `until` | promises, microtasks, and `until`'s timer | no |
| `dev.attribution.js:58,447,661-670` · `observe/core/attribution.js:71,482,676-685` `performance.now`, `Date.now`, `console.*` | the attribution engine (`./attribution` entry) | clock reads and console | no; if installed it is a hook, and still console and clock only |
| `dev-shared.js:1346` · `*/core/scheduler.js:328,331` `Symbol.for("solid-js/root-error-hook")` | the `ROOT_ERROR_HOOK` constant | a registered symbol key | no invocation |

**Hook slots reached by the five.** All are archive functions installed at
module load or by another export:

- `GlobalQueue.En` (prod) / `.Qn` (observe) / `._snapCompanions` (dev) =
  `snapCompanionsToState` (`prod/core/verdict.js:312-346`, installed `:653`;
  `observe/core/verdict.js:653`; `dev.js:1237-1276`, installed `:1589`). It
  computes pending state, and may `dispose` a disposed firewall's shadow or
  `insertIntoHeap`/`insertSubs` + `schedule()`. Archive-internal.
- `GlobalQueue.Yt` (prod) / `.In` (observe) / `._externalUntrack` (dev) =
  `externalUntrack` (`prod/core/external.js:59-61`, installed by
  `syncExternalHooks` `:66-69`; `dev.js:217-219`, installed `:225`), set only
  after a caller runs `enableExternalSource`. It returns
  `externalSourceConfig.untrack(e)`, the callable that caller supplied.
- `slotUnobservedHook` (**new in rc.9**; `prod/store/next/store.js:162`,
  `observe/store/next/store.js:166`, `dev.js:4572`), called from `unlinkSubs`
  for store leaf nodes. It deletes the leaf from its target's node map, calls
  `unlinkFirewallChild`, or defers the release with `deferSlotRelease`
  (`prod/core/scheduler.js:87-89`, a set insert). Archive-internal.
- Dev only: `DEV.hooks.onOwner?.(owner)` in `createOwner`
  (`dev-shared.js:2937`), a devtools hook.

**Builds against each other.** `dist/observe/core/owner.js` differs from
`dist/prod/core/owner.js` only in mangled property and local names, the
`import "./dev.js"` side-effect import, and the `_name: undefined` slot
`createOwner` adds to the owner literal (`diff` of the two files). The observe
`runWithOwner` slice is byte-identical to prod's, and observe `untrack`
differs only in the hook slot's mangled name. Comparing every top-level
function of `dist/prod/**` with its `dist/observe/**` twin after normalizing
short (mangled) identifiers, the functions of `core.js` and `scheduler.js`
flagged as differing are `recompute`, `computed`, `signal`, `slotSignal`,
`setSignal`, `staleValues`, `mergeTransitionState`, `insertSubs`, `flush`,
`transitionComplete` and `notifyHalted` (the last, like `owner.js`'s
`disposeRootSelf`, is textually identical on inspection: the comparison also
takes in the top-level text that follows a function). The observe
attribution-hook call sites are inside those functions or in `markRefresh` and
`Queue`. `insertSubs` is the only one the five can
reach (through `snapCompanionsToState`), and its observe addition is the
fan-out note `noteFanOut` → `reportDiagnostic(emitDiagnostic(...))`, whose
observe `reportDiagnostic` is empty. `dev-shared.js`'s `createOwner`,
`disposeChildren`, `runDisposal` and `unlinkSubs` are the rc.6 dev bodies plus
`_name: undefined`, `clearSignals`, and the `slotUnobservedHook` dispatch
above.

**Consequence.** No call that stays inside `@solidjs/signals@2.0.0-rc.9` can
register a version-1 resource into a browser document or a server runtime,
because the archive has no handle to either. rc.9's new host reaches —
`reportError` in `haltReactivity`, the two `globalThis` symbol slots, and the
records channel — register no version-1 resource kind, and none is reachable at
the call event of any of the five or on `createRoot`'s dispose path.

Terms (`local`, `builtin`, `caller`, `hook`, `host`) and the reach notation
(`always`, `cond:`, `later:`) are the rc.3 audit's § 2.

---

### 1. `getOwner` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`

#### 1.1 Implementation, all three builds

| Build | File and lines | Body |
| --- | --- | --- |
| `default` | `dist/prod/core/owner.js:211-213` | ` */ function getOwner() { return context; }` |
| `test`/`development` | `dist/dev-shared.js:2847-2849` | `function getOwner() { return context; }` |
| `observe` | `dist/observe/core/owner.js:213-215` | ` */ function getOwner() { return context; }` |

Transitive call table: **0 rows.** The body contains no call, no `new`, and no
non-call invoking form. `context` is a module-level `let` of `core.js`
(`dev-shared.js` in the development build).

#### 1.2 What changed from rc.6

The three slices are byte-identical to rc.6's (`67fcbebd…` for prod and
observe, `e8cb95b7…` for dev). Only the file and offsets moved.

#### 1.3 The `solid-js` server condition

`solid-js@2.0.0-rc.9`'s `dist/server.js:107-109`, `dist/server.dev.js:155-157`
and `dist/server.observe.js:142-144` are `function getOwner() { return
currentOwner; }`, byte-identical to each other and to `solid-js@2.0.0-rc.3`'s
`dist/server.js:91-93` (slice sha256 `f21df202…` in all four). **Same
verdict.**

#### 1.4 Verdict

**GRANT.**

Sign-off: `@solidjs/signals@2.0.0-rc.9`'s `getOwner` is `return context` in all
three builds and contains no invoking form, so the row denies that one
invocation of `getOwner` performs any `create`.

---

### 2. `onCleanup` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`

#### 2.1 Implementation, `default` — `dist/prod/signals.js:57-59`

```js
 */ function onCleanup(e) {
    return cleanup(e);
}
```

| Callee | Site | Reach | Disposition | What it does |
| --- | --- | --- | --- | --- |
| `cleanup(e)` | `signals.js:58` → `owner.js:219-223` | always | local | `if (!context) return e;`. Otherwise stores `e` in `context.ke` as a value, an array `push` (builtin), or a two-element array. Returns `e`. |

**1 row.** The caller's function is stored, not invoked. It runs later in
`runDisposal` (§ 3), a `callbacks` item of whatever disposes the owner.
Registering it on a reactive-graph owner is a `cleanups` item, not a `create`.

#### 2.2 The other two builds

- **`observe`, `dist/observe/signals.js:59-61`.** The same bytes as prod's
  slice (`89ddda30…`). `cleanup` is `observe/core/owner.js:221-225`, prod's
  body with the field spelled `Ue`. Same verdict.
- **`test`/`development`, `dist/dev.js:2174-2203`.** New in rc.9 as a slice
  (`5f8d40b1…`; rc.6's was `f2008034…`):
  - It calls `getOwner()` (§ 1).
  - With no owner it runs `reportDiagnostic(emitDiagnostic({code:
    "NO_OWNER_CLEANUP", severity: "warn", …}))` and continues. rc.6 called
    `console.warn` directly here.
  - When the owner forbids children it runs `emitDiagnostic({code:
    "CLEANUP_IN_FORBIDDEN_SCOPE", severity: "error", …})` and then
    `throw new Error(message)`, as in rc.6.
  - Otherwise it runs `return cleanup(fn)` (`:2202`); `cleanup` is
    `dev-shared.js:2855-2861`, prod's shape.
  - `emitDiagnostic` (`dev-shared.js:528-560`) builds an entry, walks
    `isExcluded`/`ownerPath` over the owner chain, records the subject in a
    module `WeakMap`, invokes the installed `diagnosticListeners` and pushes to
    `diagnosticCaptures` (hooks and module sets), and, for an `error` entry with
    an installed `consoleFooter` hook, queues
    `queueMicrotask(() => console.warn(footer))` (host, console only).
  - `reportDiagnostic` (`:600-609`) consults `takeFooter` (the installed footer
    hook) and writes one `console.warn`/`console.error` (host, console only).
  - Same verdict.

#### 2.3 The `solid-js` server condition

`solid-js@2.0.0-rc.9`'s `server.js:113-118`, `server.dev.js:161-166` and
`server.observe.js:148-153` read `currentOwner`; with none they `return fn`,
and otherwise store `fn` in `o._disposal` exactly as prod does. All three are
byte-identical to `solid-js@2.0.0-rc.3`'s `server.js:97-102` (slice sha256
`21cd3b7a…`). **Same verdict.**

#### 2.4 Verdict

**GRANT.**

Sign-off: `onCleanup` appends the caller's function to the current owner's
disposal field; in the development build it may also emit a diagnostic, write
to the console, or throw. The registration is onto a reactive-graph owner and
is a `cleanups` item. The row denies that one invocation of `onCleanup`
registers a version-1 resource into any runtime outside the invocation.

---

### 3. `createRoot` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`

#### 3.1 Implementation, `default` — `dist/prod/core/owner.js:314-317`

```js
 */ function createRoot(e, t) {
    const n = createOwner(t);
    return runWithOwner(n, () => e(() => n.dispose()));
}
```

| Callee | Site | Reach | Disposition | What it does |
| --- | --- | --- | --- | --- |
| `createOwner(t)` | `owner.js:315` → `:255-289` | always | local | builds an owner literal (its parent in `_parent`) and, when a parent `context` exists, links it as the parent's first child; field writes only |
| `inheritId(e, n, t)` | → `:157-159` | always | local | `options.id`, the parent's id, or the parent's next child id |
| `getNextChildId` → `childId` → `formatId` | `:149-151`, `:137-142`, `:170-173` | cond: the parent has an `id` | local | counter arithmetic (`toString(36)`, `String.fromCharCode` are builtins), or `throw new Error("")` |
| `runWithOwner(n, …)` | `:316` → `core.js:1688-1699` | always | local | § 5 |
| `e(() => n.dispose())` | `:316` | always | **caller** | the caller's `init` |
| `n.dispose()` = `disposeRootSelf` | `:244-246` | later: the caller invokes `dispose` | local | `disposeChildren(this, e)` |
| `disposeChildren` | `:44-119` | later | local, and **caller** at the final effect cleanup (`e.yt`) | sets `REACTIVE_DISPOSED`; snaps companions; wakes a parked transaction (`wokenTransitions.push` + `schedule()`); recursively tears down children (`deleteFromHeap`/`queueFor`, `clearDeps`, `disposeChildren`); splices itself out of the parent chain; `runDisposal`; then invokes the effect-returned cleanup, which is **caller** |
| `GlobalQueue.En(t)` | `:56`, `:62` | later, cond: companions exist | hook (`snapCompanionsToState`, § 0.3) | archive-internal companion snap; may `schedule()` |
| `schedule()` | `:67-68` and via the hook | later, cond | local → **host** `queueMicrotask(flush)` or, halted, `console.error` (§ 0.3) | the only host reach; registers no version-1 resource |
| `clearDeps` → `unlinkSubs` → `unobserved` | `graph.js:52-60`, `:12-35`, `:62-66` | later | local; **caller** at `n.o?.Pt?.()` (the caller's `unobserved` option); **hook** `slotUnobservedHook` for store leaves (§ 0.3) | unlinks graph edges; may dispose an auto-dispose computed that lost its last subscriber |
| `deleteFromHeap`, `queueFor` | `heap.js:69-83`, `:7-9` | later | local | heap bookkeeping |
| `runDisposal` | `owner.js:121-135` | later | local → **caller** | `t.call(t)` on the functions `onCleanup` registered |

**12 rows.** No path performs a `create`. The owner and its child link are
reactive-graph facts, and every outward reach is the scheduler's microtask or
the halted console write.

#### 3.2 The other two builds

- **`test`/`development`, `dist/dev-shared.js:2964-2967`.** The slice is
  byte-identical to rc.6's (`9a66667d…`).
  - `createOwner` (`:2892-2939`) is rc.6's dev body plus a `_name: undefined`
    slot and the `DEV$1` → `DEV` rename. On a children-forbidden parent it runs
    `emitDiagnostic` + `throw`, and at `:2937` it calls
    `DEV.hooks.onOwner?.(owner)` (**hook**).
  - `runWithOwner` is § 5. Its disposed-owner branch cannot fire here, because
    the owner is fresh.
  - `disposeChildren` (`:2669-2753`) and `runDisposal` (`:2754-2768`) have
    prod's shape, plus `clearSignals` (`:659-661`, a field write) and
    `_inFlight = null`. `unlinkSubs` (`:2970-3000`) dispatches store leaves to
    `slotUnobservedHook` as prod does.
  - Same verdict.
- **`observe`, `dist/observe/core/owner.js:317-320`.** The body is prod's with
  the locals renamed (`(e, n)`/`t` for `(e, t)`/`n`), slice `d266035c…`.
  `createOwner` (`:257-292`), `disposeChildren` (`:46-121`) and `runDisposal`
  (`:123-137`) are prod's bodies modulo mangling (§ 0.3), and its `schedule`
  (`observe/core/scheduler.js:282-290`) is prod's. Same verdict.

#### 3.3 The `solid-js` pairing

In `solid-js@2.0.0-rc.9`, `createRoot` is `solid-js`'s own declaration
(`types/client/hydration.d.ts:372`, `export declare const createRoot: typeof
coreRoot`), and its client builds wrap this archive's `createRoot` rather than
re-exporting it (review § 3.3). A `solid-js@2.0.0-rc.9` `createRoot` import
therefore resolves its declaration into `solid-js`, not into this archive, and
this row does not answer for it. Beside `solid-js@2.0.0-rc.3`, whose
`types/index.d.ts` re-exports this archive's `createRoot`, the `node` condition
runs rc.3's pinned `server.js:175-178`, which the rc.3 audit read (§ 3.4
there): no host reference. **Same verdict.**

#### 3.4 What changed from rc.6

- The prod and dev slices are byte-identical to rc.6's.
- In prod, `createOwner`'s parent link is no longer a mangled property (rc.6
  `ke`, rc.9 `_parent`), and in dev and observe the literal gains a
  `_name: undefined` slot; field writes either way.
- Compared with rc.6's prod bytes (an installed rc.6 tree whose `package.json`,
  `dist/dev.js`, `dist/prod/core/owner.js` and `dist/prod/core/core.js` match
  the rc.6 pins), `unlinkSubs` gained the shared `slotUnobservedHook` dispatch
  for store leaves and `disposeChildren` the parked-transaction wake
  (`wokenTransitions` + `schedule()`); both archive-internal.
- Nothing that matters to `creates` changed.

#### 3.5 Verdict

**GRANT.**

Sign-off: `createRoot` allocates an owner, links it into the in-memory owner
tree, and runs the caller's `init` under it. Its only host reach is the
scheduler's one-shot `flush` microtask, or the halted console write, on the
dispose path. The row denies that one invocation of `createRoot` registers a
version-1 resource into any runtime outside the invocation.

---

### 4. `untrack` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`

#### 4.1 Implementation, `default` — `dist/prod/core/core.js:985-995`

```js
 */ function untrack(e, t) {
    if (GlobalQueue.Yt === null && !tracking && true) return e();
    const n = tracking;
    tracking = false;
    try {
        if (GlobalQueue.Yt !== null) return GlobalQueue.Yt(e);
        return e();
    } finally {
        tracking = n;
    }
}
```

| Callee | Site | Reach | Disposition | What it does |
| --- | --- | --- | --- | --- |
| `e()` | `core.js:986`, `:991` | always, unless an external-source hook is installed | **caller** | the caller's thunk |
| `GlobalQueue.Yt(e)` | `core.js:990` | cond: `enableExternalSource` installed it (`external.js:68`) | **hook** | `externalUntrack` (`external.js:59-61`) returns `externalSourceConfig.untrack(e)`, a callable the caller of `enableExternalSource` supplied |

**2 rows.** `tracking` is a module `let`, and toggling it is not a reactive
write.

#### 4.2 The other two builds

- **`test`/`development`, `dist/dev-shared.js:5183-5197`.** Byte-identical to
  rc.6's dev slice (`2a929c26…`): `strictRead` is saved, set and restored, and
  the hook is `GlobalQueue._externalUntrack` (installed at `dist/dev.js:225`).
  Same verdict.
- **`observe`, `dist/observe/core/core.js:1015-1025`.** The prod body with the
  slot spelled `GlobalQueue.In` (installed at `observe/core/external.js:68`).
  Same verdict.

#### 4.3 The `solid-js` server condition

`solid-js@2.0.0-rc.9`'s `server.js:1560-1562`, `server.dev.js:1661-1663` and
`server.observe.js:1634-1636` are `function untrack(fn) { return fn(); }`,
byte-identical to rc.3's `server.js:1392-1394` (slice `4849f0c6…`). **Same
verdict.**

#### 4.4 What changed from rc.6

- The prod slice differs from rc.6's only in the hook slot's mangled name
  (`ht` → `Yt`); observe is a new file with the slot spelled `In`.
- Dev is byte-identical.
- The hook's installer and body are unchanged in shape.

#### 4.5 Verdict

**GRANT.**

Sign-off: `untrack` toggles the module-level `tracking` flag (and, in dev,
`strictRead`) around a call to the caller's `fn`, or to the installed
external-source hook. The row denies that one invocation of `untrack` itself
registers a version-1 resource into any runtime outside the invocation.

---

### 5. `runWithOwner` — `creates` — archive `@solidjs/signals@2.0.0-rc.9`

#### 5.1 Implementation, all three builds

| Build | File and lines | Body |
| --- | --- | --- |
| `default` | `dist/prod/core/core.js:1688-1699` | saves `context` and `tracking`, sets `context = e` and `tracking = false`, runs `return t()` in a `try`, and restores both in `finally` |
| `observe` | `dist/observe/core/core.js:1720-1731` | the same bytes as prod's slice (`5c40363e…`) |
| `test`/`development` | `dist/dev-shared.js:6083-6111` | the same body, preceded by one dev-only branch: when `owner && owner._flags & REACTIVE_DISPOSED`, it runs `reportDiagnostic(emitDiagnostic({code: "RUN_WITH_DISPOSED_OWNER", severity: "warn", …}, owner))` |

| Callee | Reach | Disposition | What it does |
| --- | --- | --- | --- |
| `t()` / `fn()` | always | caller (parameter-rooted) | the caller's callable; whatever it registers is its own |
| `emitDiagnostic(...)` | dev, disposed owner only | local | § 2.2: installed listeners and captures, an owner-chain walk, a module `WeakMap`; no footer microtask, because the entry is `warn` |
| `reportDiagnostic(...)` | dev, disposed owner only | local → host | § 2.2: one `console.warn` |

**3 rows**, none of them a registration. Swapping the ambient owner is a change
to reactive-graph state, not a `create`.

#### 5.2 What changed from rc.6

- Prod and observe are byte-identical to rc.6's prod slice.
- Dev (`332a218c…`; rc.6's was `87f9a231…`) replaces rc.6's
  `emitDiagnostic(...)` + `console.warn(message)` with
  `reportDiagnostic(emitDiagnostic(..., owner))`: the same console write,
  now with the owner as the diagnostic's subject.

#### 5.3 The `solid-js` server condition

`solid-js@2.0.0-rc.9`'s `server.js:87-98`, `server.dev.js:135-146` and
`server.observe.js:122-133` (byte-identical, slice `17977b59…`) save
`currentOwner`, set it, run `return fn()`, and restore it in `finally`. New in
rc.9 is a `catch (error) { stampThrower(error, owner); throw error; }`:
`stampThrower` (`server.js:100-103`) records the owner in a module-level
`WeakMap` keyed by the thrown object, unless the owner is `null`, the error is
not an object (`isObject`, `:1336`), or it is a `NotReadyError`, and the error
is rethrown. A module-private binding that nothing outside the invocation
reads is not a `create` (`semantic-model.md` § creates). rc.3's
`server.js:82-90` is the body without the `catch`, read by the rc.3 audit.
**Same verdict.**

#### 5.4 Verdict

**GRANT.**

Sign-off: `runWithOwner` swaps the module-level owner and tracking state around
a call to the caller's `fn`; in the development build it may also report a
disposed-owner diagnostic to the console. The row denies that one invocation of
`runWithOwner` registers a version-1 resource into any runtime outside the
invocation.

---

### 6. Citations, and how they were computed

Byte offsets of the whole file, read in binary from the extracted tarball, and
compared with the same range of the lockfile install. Each slice's sha256 was
computed, and each slice was checked to begin with the export's own definition
(`function <export>(`, after an optional ` */ `). The two conventions are the
rc.6 audit's:

- § 1–4 use the 2026-09-04 convention: from the start of the definition's first
  line (including a leading ` */ `) to the start of the line after the closing
  `}`.
- § 5 uses the 2026-09-23 convention: from the `function` token to the closing
  `}`, inclusive.

The slicing script reproduced, on the installed rc.3 tree, the rc.3 slice
digests `solid_2.rs` carries for these five exports (for example `getOwner`
`7691..7739` → `67fcbebd…`, `runWithOwner` `37382..37594` → `5c40363e…`), so the
conventions are applied as before.

| Export | `archive_path` | lines | `start_byte` | `end_byte` | `slice_sha256` | = rc.6 slice? |
| --- | --- | --- | --- | --- | --- | --- |
| getOwner | dist/prod/core/owner.js | 211-213 | 8615 | 8663 | `67fcbebd02b9e57fe095ba4af938b3b4b27e52e9ecda46862ddce141f1e4c9e2` | yes |
| getOwner | dist/dev-shared.js | 2847-2849 | 131908 | 131950 | `e8cb95b765807fa14a97032551f4bbced263cc3d7837fc1258f156ffc71ba8bb` | yes (rc.6 dev) |
| getOwner | dist/observe/core/owner.js | 213-215 | 8635 | 8683 | `67fcbebd02b9e57fe095ba4af938b3b4b27e52e9ecda46862ddce141f1e4c9e2` | yes (rc.6 prod) |
| onCleanup | dist/prod/signals.js | 57-59 | 2598 | 2651 | `89ddda3041ae80177d8bea1730aeb504ddb62f57d37956e3cfea6232f0f8925b` | yes |
| onCleanup | dist/dev.js | 2174-2203 | 96698 | 97559 | `5f8d40b1c3165f17bc00846b5063eb55f8bdd0a6b49dceba4a60f5aeb6570e44` | no (`reportDiagnostic`) |
| onCleanup | dist/observe/signals.js | 59-61 | 2646 | 2699 | `89ddda3041ae80177d8bea1730aeb504ddb62f57d37956e3cfea6232f0f8925b` | yes (rc.6 prod) |
| createRoot | dist/prod/core/owner.js | 314-317 | 11782 | 11902 | `eefc749ba75a19179e86ce86935623ba829da692628162c809267f812583bbe3` | yes |
| createRoot | dist/dev-shared.js | 2964-2967 | 135728 | 135870 | `9a66667de7c1a48f5a90e9901d994ba532da603ac763dc460c16fb8e60496fa5` | yes (rc.6 dev) |
| createRoot | dist/observe/core/owner.js | 317-320 | 11828 | 11948 | `d266035c22be8514767dbee4fe3aa0c947336ad4da585e24b09ee1722d916259` | no (locals renamed) |
| untrack | dist/prod/core/core.js | 985-995 | 49018 | 49298 | `28c2d9f3861b28ad08edaeb66a6d8f2caa5530d6e7cff226afda0a9a7b5d912a` | no (`ht` → `Yt`) |
| untrack | dist/dev-shared.js | 5183-5197 | 242868 | 243344 | `2a929c2683ae93a3820bf2b4bf1a4455b41c6a928880eaa480a6820fe43a1852` | yes (rc.6 dev) |
| untrack | dist/observe/core/core.js | 1015-1025 | 50382 | 50662 | `01672a8cba1c1d7a8800b0effde85a96cffd51ac0bb7025b40b208f805e98773` | no (slot `In`) |
| runWithOwner | dist/prod/core/core.js | 1688-1699 | 87558 | 87770 | `5c40363ecc6eaf66378b57e0c387103fe67f51706d30dab3cb041fd10f8af3e5` | yes |
| runWithOwner | dist/dev-shared.js | 6083-6111 | 287156 | 287864 | `332a218ceec024a1d0c3464213b7d043d4a902c529b5f02a7d6ae0467070a11c` | no (`reportDiagnostic`) |
| runWithOwner | dist/observe/core/core.js | 1720-1731 | 89058 | 89270 | `5c40363ecc6eaf66378b57e0c387103fe67f51706d30dab3cb041fd10f8af3e5` | yes (rc.6 prod) |

The `file_sha256` values are in § 0.1. The slices are checked in under
`rust/crates/solid-dialect/audited-slices/solid-v2/rc9/solidjs-signals/`.

**Caution on the "= rc.6 slice?" column.** A slice digest that matches rc.6's
pins only the *subject*. It does not pin the callee closure. The verdicts rest
on the rc.9 walks above, not on the slice equality.

### 7. What this changes, and what it cannot move

- **Rows.** 5 new `@solidjs/signals@2.0.0-rc.9` `creates` rows; the table goes
  from 75 to 80 rows (`creates` 49 → 54). No rc.3 or rc.6 row changed.
- **The proposal side does not move.** `some_audit_denies_primitive` is
  version-blind, and each of the five `(export, domain)` pairs was already
  denied on rc.6, so no proposal answer changes.
- **The pinned census does not move.** A row binds only through the census's
  four-field identity gate on an authenticated snapshot (name, version,
  integrity, `package.json` digest), and no census row installs
  `@solidjs/signals@2.0.0-rc.9`: `corpusSignalsPin` pins every Solid 2 probe to
  rc.0 or rc.6 and refuses a probe that names rc.9
  (`scripts/ecosystem-benchmark/runtime-pins.test.mjs`); the manifest's
  `solidReleases` list ends at rc.3; none of the 938 cached install locks under
  `rust/target/install-locks/` names `@solidjs/signals@2.0.0-rc.9` or its
  integrity (127 name rc.0, 26 rc.3, 148 rc.6); and the shipped tier
  (`pkg/contracts/accepted/index.json`) records no rc.9 environment.
- **Consumer environments.** A consumer environment that pins
  `@solidjs/signals@2.0.0-rc.9` is now held to this archive's integrity rather
  than refused by version. `solid-js@2.0.0-rc.9` is still refused by the audited
  Solid 2 ceiling, so no checked-in environment changes.
