# Simple cases that main leaves uncertifiable (R3, 2026-11-21)

Read-only research. No repo change, no commit, no push.

**Build.** origin/main **b600297eb**, release checker and producer built in a scratch worktree
(`r3/main`, `sc-heavy make build-checker-release`). Copies are in `r3/bin/`.
No fuzz-audit run was usable: every run since 2026-11-16 failed or was cancelled.

## 0. What was measured, and what was estimated

| | programs | how |
|---|---|---|
| mx-base, every code | 27,960 | `focus.mjs --codes all --grids mx-base,lg-base --split-codes --runtime-diff 0` |
| lg-base, every code | 12,360 | same run |
| mx-recall (lean), **simple slice** | 42,609 of 1,323,492 | `focus.mjs --grids mx-recall -- --ids mx-recall-simple.json` |
| lg-recall (lean), **simple slice** | 32,433 of 744,816 | same, `lg-recall-simple.json` |
| textbook set | 21 | `textbook.mjs check`: 21 certified, **0 U** |

Every grid program runs in real Chrome: the fast host is Chrome (`--fast-host chrome`) and results come from the per-program cache.
So every row below has a definite Chrome answer:
- misuse `recall-gap`: the checker is U at the site and Chrome reports the code;
- legal `uncertifiable`: the checker is U and Chrome is silent (a legal program where Chrome reports is `not-legal`, a different outcome).

**Measured:**
- the outcomes;
- the at-site `blockedBy.kind`, from `blocker-scan.mjs` on every U row: 45,870 programs;
- the deciding role arm at the site, from `SOLID_CHECKER_IR_ROLE_LOG` on one program per (side, code, blocker set, context, variant): 9,408 programs.

**Estimated:**
- proof size;
- proof idea;
- counterexample;
- "why U" where no arm names the cause (read from source, not run);
- the family grouping, which is my regex rules in `family.cjs`.

Not run:
- mx-src, mx-combo, lg-src, lg-combo;
- the recall grids outside the simple slice;
- the pair grids;
- the census.

## 1. Definition of SIMPLE (operational)

A U row is SIMPLE when all of the following hold. The scripts are `select.mjs` and `classify.mjs`.

1. **Shape.**
   - Binding is `direct`, `alias`, `alias-const` or `namespace`.
   - Layer is `none` or `closure` (one `const f = () => …; f()`).
   - Mount is none, or one of `child`, `show`, `for`, `show-signal`, `for-item`, `show-later`.
   - No file source, no package site, and callback form `const` or none.
   - The recall slice was preselected with layer `none` only, and with no context whose name matches patch/cast/recv/flw/impostor/eval/proxy/reflect/proto.
2. **Program text.**
   - Exactly one source file (`App.tsx`) and no generated package.
   - At most 30 non-blank lines.
   - **None** of these tokens, which mark the adversarial families:
     - prototype: `prototype`, `__proto__`, `setPrototypeOf`, `Object.create`;
     - `Reflect.` or `Proxy`;
     - `defineProperty`, `Object.assign(`;
     - `eval` or `new Function`;
     - protocol tricks: `toString`, `valueOf`, `Symbol.`, `toJSON`;
     - a getter or setter;
     - any `as X` cast other than `as const`;
     - `.call(`, `.apply(`, `.bind(`;
     - assignment to `window`, `document` or DOM-class members;
     - synthetic events and imperative DOM: `dispatchEvent`, `new *Event`, `addEventListener`, `.click()`, `.focus()`, `createElement`, `DOMParser`, `srcdoc`, `javascript:`, `attributes`;
     - `JSON.`;
     - `solid-js/internal` or `createOwner`;
     - `class X`, `function*` or `yield`;
     - `with(`;
     - a computed-key call.

`runWithOwner` and `getOwner` are allowed. Everything here is a plain Solid component of under 30 lines.

**Result:** 45,870 U rows in total. 18,128 are SIMPLE: **6,744 misuse** (Chrome reports) and **11,384 legal** (Chrome silent).
The rest fail on adversarial tokens (24,458; `dispatch` 12k, `dom` 8.7k, `cast` 6.8k, `proto` 5.9k), shape (1,252), length (1,367), files (344) or package (1,173).

A count is a **program** count. The grids cross each question with codes, variants, mounts and two bindings, so the "shapes" column (distinct code × context × variant) is the better measure of distinct work.

Base grids alone:
- mx-base: 24 U, all `PENDING_ASYNC_FORBIDDEN_SCOPE tracked` at layer `iife`, so none is simple.
- lg-base: 1,392 U, of which 146 are simple. The largest simple group is `resolve()` or `until()` written directly in an `onClick` handler (F20).

## 2. Defects first: SIMPLE cases with V where Chrome is quiet, or clean where Chrome reports

- **False cleans:** 0 in every measured program (base, both simple recall slices).
- **False positives:** 0.
- **One unledgered V at a site Chrome never runs**: 12 programs. `focus` lists them as `unexecuted-violation` and exits 1.

```tsx
const [s1, setS1] = createStore({ v: 0 });
const K1 = () => { { const { v } = s1; void v; }; return <i />; };
return <p>{isServer ? <K1 /> : null}</p>;   // CSR build: K1 never renders
```

- `no-destructure` gives a **violation** at line 7. Chrome never runs the site (hit counter 0).
- The variants are `store-dstr-{object,empty-sub,empty-items,empty-sibling,then-first,quoted}`, each with `direct` and `alias`.
  Example id: `lg-strict-read-untracked-arm-tag-store-dstr-object-direct-none`.
- The other 13 `store-dstr-*` variants in the same `arm-tag` context are U. So is the signal twin (`arm-tag-signal`, fixed by ADRs 0677/0678, listed in rediscovery.json).
- The owner's 2026-11-17 ruling (ADR 1018) is that a provably dead site gives U, not V.
- No entry for these ids is in open-defects.json or unexecuted-known.json.
- The `store-dstr-*` arm variants came with ADR 1124 round 2 (#299). The smoke cover probably does not sample them.

Also seen, all accounted for:
- 18 `dny-legal-{click,keydown,pointer}` unexecuted rows: undriven handler, accepted by ADR 1132.
- 5 `tbl-nodelistof-foreach-zero` rows: held in unexecuted-known.json.
- 4 `harness` rows (`pos-class-field-store-map-*`, "no runtime record"): not judged.

## 3. Catalog, misuse side (Chrome reports, main says U): 6,744 programs

Coverage marks:
- `OW` = named in docs/open-work.md;
- `R` = held U by the owner's ADR 1018 dead-site ruling;
- `UC` = code in `upstream_compat/` (invariant 5: read eslint-plugin-solid 6d3bc311 first).

No open ADR branch on origin (2026-11-21 list) targets any of these families directly.

| # | family | programs | shapes | main codes | at-site blocker | size |
|---|---|---|---|---|---|---|
| M1 | effect cleanup returned from the apply, run on rerun | 765 | 191 | SRU 232, PIEC 114, NO_OWNER_BOUNDARY 102, SWOS 60 | function-run-unplaced, owner-callers-unenumerated, effect-owner-unproven | M |
| M2 | owner call after an await inside an async loop/finally/for-await | 755 | 73 | NO_OWNER_EFFECT 257, _BOUNDARY 242, _CLEANUP 121, SETTLED_CLEANUP_UNOWNED 76 | owner-varies-by-caller + effect-creation-unaccounted | S (for-await, finally), M (loop) |
| M3 | named callback handed to a primitive **and** called directly | 666 | 333 | SRU 666 | function-run-unplaced | S–M |
| M4 | owner/leaf operation in a parameter default (`pd-*`) | 572 | 195 | PIFS 234, PIEC 117, CLEANUP_IN_FS 78, FLUSH_IN_TQC 78 | leaf-operation-unaccounted, parameter-initializer-unplaced | M (OW: ADR 0961) |
| M5 | store destructure with an empty, nested, quoted or for-of pattern | 384 | 192 | SRU 384 | destructure-root-unproven, props-callers-unenumerated | S |
| M6 | callback prop invoked by the child (`cbp-*`) | 371 | 139 | 15 codes | function-run-unplaced, callee-callback-timing, leaf-operation-unaccounted | M |
| M7 | store array `sort/reverse/fill/copyWithin` | 324 | 162 | SWOS 324 | store-operation-may-not-write | S (OW ADR 0718, UC) |
| M8 | store value handed to a standard function (`Object.values`, `map`, `snapshot`…) | 296 | 150 | SRU 244 | value-in-carrier, value-to-unresolved-callee | M (OW value-use census) |
| M9 | read or write after an await in a loop, finally or for-await | 224 | 94 | SWOS 150, URAA 53 | handed-read-after-await, read-after-await-unaccounted | S–M (OW ADR 1038/1112) |
| M10 | `flush(() => read)` inside a closure called once | 196 | 74 | SRU 144 | function-run-unplaced (arm `inline-callback`) | S |
| M11 | await under a condition or a `.then` chain (`aw-if`…, `then-*`) | 169 | 43 | URAA | handed-read-after-await | S–M |
| M12 | function in an object slot or method, called (`comp-slot`, `root-stored`, `root-method`) | 144 | 27 | RWOS 94 | function-run-unplaced (arm `inheriting-call`) | M (OW ADR 0456) |
| M13 | store held in a local, then written (`nested-held`) | 130 | 65 | SWOS 130 | unproven-container-root | S–M (OW ADR 1064, UC) |
| M14 | named function as `ref=` / ref in a row | 106 | 29 | PAUR 48, SRU 44 | function-run-unplaced, invocation-context-unproven | S |
| M15 | `render-apply-guarded-rerun` (the guard opens on rerun) | 105 | 9 | PIEC 36, NO_OWNER_* | owner-later-run-unowned, effect-owner-unproven | M |
| M16 | value or ref handed in an apply (`apply-*-call-untrack`) | 92 | 33 | 5 | function-run-unplaced | M |
| M17 | member call on a handed hook value | 91 | 48 | SRU | handed-member-dispatch | M (OW ADR 0692 trade) |
| M18 | effect apply that flushes or reruns | 91 | 36 | 6 | function-run-unplaced, owner-later-run-unowned | M (OW "lost proved verdicts", ADR 1052) |
| M19 | closure kept in a Set/array/Map, called later | 80 | 31 | 11 | function-run-unplaced (arm `allowed-unplaced-nested`) | M (OW ADR 0832) |
| M20 | helper fires via timer/microtask/then (`fire-helper-*`) | 78 | 28 | 8 | function-run-unplaced | M |
| M21 | `<Loading on={props.on}>` through a prop or spread | 54 | 12 | LOADING_ON_ACCESSOR | none at the site (remote U) | M |
| M22 | `sync:true` / `static:true` with a conditional source (`cmp-cond`) | 33 | 11 | SNRA, DSP | compute-return-unproven | S |
| M23 | error arm of an effect or reaction | 30 | 14 | SWOS 20 | function-run-unplaced | S |
| — | other (long tail) | 944 | 747 | — | function-run-unplaced 516, effect-creation-unaccounted 193 | — |

## 4. Catalog, legal side (Chrome silent, main says U): 11,384 programs

| # | family | programs | shapes | main codes | at-site blocker | size |
|---|---|---|---|---|---|---|
| L1 | read or write after an await in a handler, `ref`, loop, finally or for-await | 1,514 | 494 | SRU 874, RWOS 292, ON_FLUSH 89 | read-after-await-unaccounted (arm `window_after_awaits.unknown` 835, `.unreported` 340) | S–M (OW ADR 1038/1112) |
| L2 | callback prop the child never calls or calls in a handler (`cbp-*`) | 1,034 | 384 | 18 codes | function-run-unplaced, effect-creation-unaccounted, leaf-operation-unaccounted | M (part R: never called) |
| L3 | dead code (`gate-throw`, `dead-bundle-*`, `dead-*`, guarded apply, isServer arm) | 801 | 258 | 23 codes | unreached-dead-code, callee-callback-timing | **R**: stays U |
| L4 | store passed to `snapshot`, `Object.hasOwn`, `Object.values`, `.map` in a memo/JSX/untrack | 782 | 440 | SRU 769 | value-in-carrier | M (OW value-use census) |
| L5 | effect cleanup run on disposal, Show toggle or For removal | 651 | 259 | SRU 392 | function-run-unplaced (arm `allowed-unplaced-nested` 465, `allowed-effect-cleanup-rerun` 120) | S–M |
| L6 | owner/leaf/resolve call after an await in an async loop/finally | 627 | 126 | SRU 160, RESOLVE 151, UNTIL 151 | effect-creation-unaccounted, flush-in-effect-unaccounted | M |
| L7 | store destructure with a default, renamed, `then` or nested pattern | 525 | 307 | SRU 525 | store-key-undecided | S–M (OW ADR 1124/1138) |
| L8 | error arm `createEffect(c, { error: () => read })` | 510 | 169 | SRU 286 | function-run-unplaced (arm `allowed-unplaced-nested`) | S |
| L9 | named callback handed to a primitive and also called or listed | 477 | 260 | SRU | function-run-unplaced (arm `named-callback-entries-open`) | S–M |
| L10 | `resolve()` / `until()` in a handler or async computation | 458 | 230 | RESOLVE 229, UNTIL 229 | effect-creation-unaccounted | **S** (handler) |
| L11 | `flush(() => read)` in a closure (`flush-read-each`, `handed-flush`) | 404 | 176 | SRU 378 | function-run-unplaced (arm `inline-callback`) | S |
| L12 | closure the compute returns and the apply never calls (`apply-*-value-*`) | 358 | 132 | 21 codes | function-run-unplaced | **R** (returned closure nobody calls) |
| L13 | effect apply with an owner call or flush, never rerun | 309 | 104 | 13 | owner-later-run-unowned, effect-owner-unproven | S |
| L14 | helper handed to a handler, timer from a handler (`helper-handed-handler`, `*-from-handler`) | 248 | 95 | SRU 206 | function-run-unplaced, effect-creation-unaccounted | M |
| L15 | zero-iteration carrier (`[].forEach`, `zero-*`) | 116 | 116 | RWOS 57 | function-unentered, callback-invocation-unproven | **R** (empty literal collection) |
| L16 | closure kept in a Set/array/Map | 109 | 39 | 16 | function-run-unplaced | M (OW ADR 0832) |
| L17 | named function as `ref=` (`ref-named`, `ref-factory`) | 84 | 8 | RWOS 58, ON_FLUSH 26 | function-run-unplaced | **S** |
| — | others (≤ 82 each: pd, fire, aw, comp-slot, sort, sync) | 405 | | | | |
| — | other (long tail) | 2,052 | 1,783 | RWOS 208, SRU 170, URAA 118, PIEC 99 | function-run-unplaced 1,285 | — |

Legal rows in the **R** families (L3, L12, L15, about 1,275) are U by the owner's ADR 1018 ruling. They are not proof targets unless the owner reopens that ruling.

## 5. Group details (top 15 by size across both sides)

Each entry gives an example (from the grid program, trimmed), the blocker, why it is U today, a proof idea and a counterexample that must stay U.

### G1. Read after an await in a handler, loop, finally, `ref` or `for await` (L1 1,514 + M9 224 + M11 169)

```tsx
const f1 = async () => { try { await Promise.resolve(); } finally { r1(); } };
<button onClick={() => { f1(); }}>go</button>          // legal: silent
const m1 = createMemo(async () => { for await (const _ of [1]) { r1(); } return 1; }); // misuse: URAA
```

- **Blocker:** `read-after-await-unaccounted` (SC1002), `handed-read-after-await`.
- **Why U:**
  - `execution_role/body_roles.rs:window_after_awaits` (l.1445) returns `Unknown` for `follows_any_suspension || may_follow_own_await` (arm `body_roles.window_after_awaits.unknown`).
  - The unconditional case gives `Unreported`, which ADR 1112 says hides UNTRACKED_READ_AFTER_AWAIT in an async computation.
  - `static_rules.rs:region_calls_handed` (l.1686) for M9/M11.
  - The finally path is not counted as following the try's await.
- **Proof idea:** split the after-await role by the function's runner:
  - run only from an event callback or `ref` (no computation flight) → silent for every read rule: legal;
  - run as an async computation's compute and the body provably runs (a `for await` over a non-empty literal; a `finally` after a `try` whose head awaits on every path with nothing before the await that can throw) → UNTRACKED_READ_AFTER_AWAIT violation.
- **Size:** S for `for await` and `finally`; M for `aw-if`/`aw-and` (an await on some path only: a read that follows only on the awaiting path is still unplaced; see the counterexample).
- **Counterexample that must stay U:** `if (c) await p; r1();` in a memo, where `c` is not proven. The read may run in the window on the non-awaiting path.
- **Coverage:** OW (ADR 1038 "async-loop … await only on some path"; ADR 1112).

### G2. Callback prop handed to a child component (L2 1,034 + M6 371)

```tsx
<Kid1 run={() => { r1(); }} />
function Kid1(props: { run: () => void }) {
  createEffect(() => 1, () => { props.run(); });   // misuse: Chrome reports SRU
  return <p />;                                     // legal twin: never calls props.run
}
```

- **Blocker:** `callee-callback-timing` (SC1001), `function-run-unplaced`, `leaf-operation-unaccounted`.
- **Why U:** `execution_role/callee_roles.rs:semantic_role_before_awaits`, arms `compiler-source` (l.1017) and `invocation-flow` (l.1013). The prop's callback role is the compiler's "passed to a component", with no read of the child's uses of `props.run`.
- **Proof idea:** the child is an exact local component, its `props` parameter is not spread or escaped, and every read of `props.run` is a direct call. Place the callback at those call sites: in an apply → apply role; in a handler → event role; no reads → never runs.
- **Size:** M.
- **Counterexample that must stay U:** the child spreads `props` into another element or component (`<div {...props}>`), or reads `props.run` without calling it. The child could also be mounted through `dynamic`.
- **Coverage:** the never-called legal half (`cbp-never*`, about 284 rows) is arguably under the R ruling; ask the owner.

### G3. Effect cleanup returned from the apply (M1 765 + L5 651)

```tsx
createEffect(() => t1(), (v) => () => { r1(); });   // misuse: setT1 in a handler reruns → SRU
const d = createRoot((dispose) => { createEffect(() => 1, (v) => () => { r1(); }); return dispose; }); // legal: only disposal
```

- **Blocker:** `function-run-unplaced`, `owner-callers-unenumerated`, `effect-owner-unproven`.
- **Why U:**
  - `execution_role/callee_roles.rs:nested_unplaced_role` (l.1066). The ADR 0539 arm `allowed-effect-cleanup-rerun` (l.1154) returns `Unknown` whenever `write_roles::windowed_returned_cleanup == MayRerun`.
  - The disposal-only case falls through to `allowed-unplaced-nested` (l.1304).
  - Owner codes: `owners::owner_emission` arm `returned-unplaced`.
- **Proof idea:**
  - legal: the compute tracks nothing that any write can change → the cleanup runs only at disposal → `allowed-nested-disposal` (DeferredCallback);
  - misuse: some reachable write (a handler on a mounted enabled control, the ADR 1132 machinery) targets a source the compute reads, and the effect's owner is not disposed first → the cleanup runs in the window → V.
- **Size:** M (needs a "rerun reachable" proof). The legal "compute tracks nothing" half is S.
- **Counterexample that must stay U:** the compute reads `t1` and the only writer of `t1` also disposes the root before writing.
- **Coverage:** OW (ADR 0539; `createRenderEffect` rerun note).

### G4. Owner operation after an await in an async loop, finally or for-await (M2 755 + L6 627)

```tsx
const m1 = createMemo(async () => {
  for (let i = 0; i < 2; i += 1) { if (i > 0) { createEffect(() => 1, () => {}); } await Promise.resolve(); }
  return 1;
});
```

- **Blocker:** `owner-varies-by-caller` (SC4001) + `effect-creation-unaccounted` (SC6002).
- **Why U:**
  - `blockers.rs:for_owner`: `requirement.owner_varies_by_caller`. The owner pass sees one run before and one after the await.
  - The SC6002 row is from `proof_universe.rs:chain_end` (l.391), the creation census: no row places the creation.
  - Role arm `callee_roles.tracked-callback`.
- **Proof idea:**
  - `for await` body and `finally` after an awaiting try: every run of the site follows a suspension → no owner → violation (S).
  - Counted loop with an `i > 0` guard: needs loop-carried facts (the guard holds only after one iteration's await) (M).
- **Size:** S (`for-await`, `finally`), M (loop).
- **Counterexample that must stay U:** `for (…) { createEffect(…); await p; }`. The first iteration runs owned.
- **Coverage:** OW (ADR 1038).

### G5. Named callback handed to a primitive and also called (M3 666 + L9 477)

```tsx
{ const f = () => s1.v; createMemo(f); [f].map((g) => g()); }   // body: the direct call warns SRU
```

- **Blocker:** `function-run-unplaced`.
- **Why U:** `execution_role/callee_roles.rs:semantic_role_before_awaits`, arms:
  - `named-callback-also-called` (l.555);
  - `named-callback-also-called-untracked` (l.479);
  - `named-callback-entries-open` (l.376).

  A function bound to a name that has another entry besides its by-name primitive position is unplaced (ADR 0517).
- **Proof idea:** a read rule asks whether *some* run is untracked. If one entry is a direct call (or an inline standard-library callback over a literal array) in the body window, the read reports: V. Legal when every entry is tracked or deferred.
- **Size:** S–M.
- **Counterexample that must stay U:** the second entry is a call through an unresolved helper.
- **Coverage:** OW (ADR 0517, named callbacks).

### G6. Store value handed to a standard function (L4 782 + M8 296)

```tsx
const m1 = createMemo(() => { Object.hasOwn(s1, "v"); return 1; });   // legal
untrack(() => { snapshot(s1); });                                     // legal
```

- **Blocker:** `value-in-carrier` (SC9012).
- **Why U:** `value_use.rs:through` (l.1471). The value-use census stops when a store reaches a standard callee or carrier it has no reviewed behaviour for.
- **Proof idea:** reviewed read behaviour for these standard callees (`Object.hasOwn`, `Object.keys`/`values`/`entries`, `Array.prototype.map`, `snapshot`):
  - in a tracked position they are tracked reads (legal);
  - in the body window they are untracked reads (V).
- **Size:** M (a reviewed table plus the patch premise, ADR 0840).
- **Counterexample that must stay U:** a program that patches `Object.hasOwn`. That one is not simple, and `standard-member-impostor` already covers it.
- **Coverage:** OW (value-use census accepted trade, ADRs 0692, 0831, 0860).

### G7. Store destructure patterns (M5 384 + L7 525)

```tsx
createEffect(() => 1, (v) => { const { a: {} } = s1; });    // misuse SRU (empty nested pattern)
const { then = 1 } = s1;                                     // legal-side: store-key-undecided
```

- **Blocker:** `destructure-root-unproven`, `props-callers-unenumerated` (the SC1003 fallback in `blockers.rs:for_rule`), `store-key-undecided`.
- **Why U:** `static_rules.rs:open_destructures` (l.1078). `local_access.rs:discover_member` (l.7822) gives `store-key-undecided` for a default, renamed or `then` key.
- **Proof idea:** every pattern form reads its keys in order. An empty object pattern still runs `RequireObjectCoercible` and reads nothing. A nested pattern reads the outer key. A default reads the key. So the reads are exact keys of a literal store: placed like `s1.a`.
- **Size:** S.
- **Counterexample that must stay U:** a computed key (`{ [k]: x } = s1`) with `k` unproven, and a rest element.
- **Coverage:** OW (ADR 1124/1138).

### G8. Error arm of an effect or reaction (L8 510 + M23 30)

```tsx
createEffect(() => { if (t1() === 2) throw new Error("x"); return 1; },
  { effect: () => {}, error: () => { r1(); } });          // legal: silent
```

- **Blocker:** `function-run-unplaced`.
- **Why U:** `callee_roles.rs:nested_unplaced_role` falls through to `allowed-unplaced-nested` (l.1304). The `error` member of the effect's options object is no dialect callback position.
- **Proof idea:** a `Dialect` question: callback semantics of the `error` member of `createEffect` and `createReaction` options (where rc.21 runs it, and whether it tracks or warns). Placed like the apply.
- **Size:** S (read rc.21 first).
- **Counterexample that must stay U:** an options object that is not a literal, or that is spread (`{ ...opts, error }`).

### G9. Parameter default with an owner or leaf call (M4 572)

```tsx
const pdf1 = (p0: unknown = (() => { onCleanup(() => {}); return 1; })()) => p0;
createTrackedEffect(() => { pdf1(); });                     // CLEANUP_IN_FORBIDDEN_SCOPE
```

- **Blocker:** `parameter-initializer-unplaced`, `leaf-operation-unaccounted`.
- **Why U:** `static_api.rs:runs_unplaced_once` (l.3258). ADR 0961: no walk reads which calls omit the argument.
- **Proof idea:** for an exact local function, every call site's argument count. A call that passes fewer arguments than the parameter's index runs the default on that call's stack.
- **Size:** M.
- **Counterexample that must stay U:** a call that spreads arguments (`f(...xs)`), or a reference handed on.
- **Coverage:** OW (ADR 0961).

### G10. `resolve()` / `until()` in a handler or async computation (L10 458)

```tsx
<button onClick={() => { resolve(() => 1); }}>go</button>   // legal: silent
```

- **Blocker:** `effect-creation-unaccounted` (SC6002 primitive-in-effect-callback).
- **Why U:** `proof_universe.rs:chain_end` (l.391). The creation census has no row for `resolve`/`until` outside an apply, so SC6002 keeps an obligation.
- **Proof idea:** SC6002 is about an apply's stack. A site whose role is `EventCallback` (an inline JSX handler), or one inside a memo compute, is not an apply: close the row with that role.
- **Size:** **S**. This is lg-base, 240 base programs at direct/none.
- **Counterexample that must stay U:** `resolve` in a helper that an apply also calls.

### G11. `flush(() => read)` inside a closure called once (M10 196 + L11 404)

```tsx
const f1 = () => { flush(() => { r1(); }); return 1; };
f1();                                                     // body: SRU
```

- **Blocker:** `function-run-unplaced`.
- **Why U:** `callee_roles.rs:semantic_role_before_awaits` arm `inline-callback` (l.576). The flush callback's role is taken from the inline callback's own standing, which is Unknown when its function is a closure layer. The climb that places `f1()` at the body is not applied through the primitive callback.
- **Proof idea:** `flush(fn)` runs `fn` synchronously on the caller's stack. Compose with `climb_in_place_layers`: if the closure is called once in place, the callback takes the call site's role.
- **Size:** S.
- **Counterexample that must stay U:** the closure is also handed to a timer.

### G12. Effect apply that flushes or makes an owner call, but never reruns (L13 309 + M18 91)

```tsx
createEffect(() => 1, (v) => { onCleanup(() => {}); });
flush();                                                  // legal: apply runs once, owned
```

- **Blocker:** `owner-later-run-unowned`, `effect-owner-unproven`.
- **Why U:** `blockers.rs:for_owner` (`later_run_unowned`). The apply is owned only on its first run, and nothing proves there is no second run.
- **Proof idea:** a compute that reads no reactive source (a constant) never reruns, so the apply runs once, owned: legal. A compute with a reachable writer reruns, and the misuse is a V.
- **Size:** S for the legal half.
- **Counterexample that must stay U:** a compute that reads a prop.
- **Coverage:** OW ("lost proved verdicts", ADR 1052 parts 5 and 9).

### G13. Store array mutators (M7 324 + 30 legal)

```tsx
const [s1] = createStore({ items: [1, 2] });
s1.items.sort((a, b) => b - a);                           // SWOS
```

- **Blocker:** `store-operation-may-not-write`.
- **Why U:** `upstream_compat/shared_reactivity.rs:mutator_always_writes` (l.5513, ADR 0718). There is no length proof, so the method may write nothing.
- **Proof idea:** the store's initial literal array of length ≥ 2 (sort, reverse) or ≥ 1 (fill, copyWithin with a non-empty range), with no earlier write that shortens it before the site → the method writes. ADR 0491's literal proof, extended to these four methods.
- **Size:** S. It is UC code, so read upstream first.
- **Counterexample that must stay U:** `items: []` or `[1]` with `sort`/`reverse`, and any write to `items` that can run earlier.
- **Coverage:** OW (ADR 0718).

### G14. Named function as `ref=` (L17 84 + M14 106)

```tsx
const rf1 = () => { setW1(7); };
<p ref={rf1} />                                           // legal: ref runs untracked, no owner throw
```

- **Blocker:** `function-run-unplaced`.
- **Why U:** `callee_roles.rs` arm `inside-component`. The inline `ref={() => …}` is placed by the compiler; a name bound to a function and used only as `ref` is not followed. M14's `ref-row` uses `execution_role::ref_applied_read_role` (arm `.unknown`).
- **Proof idea:** a `const` whose only reference is a `ref` attribute value takes the inline ref's role.
- **Size:** **S**.
- **Counterexample that must stay U:** the same `rf1` is also called elsewhere, or exported.

### G15. Store held in a local, then written (M13 130)

```tsx
setTimeout(() => { (() => { const held = s1.a; held.v = 7; })(); }, 0);   // SWOS
```

- **Blocker:** `unproven-container-root`.
- **Why U:** `upstream_compat/shared_reactivity.rs:handed_container_write` (l.1920).
- **Proof idea:** a `const` initialised from a literal store path and never reassigned is that path. The write is `s1.a.v = 7`.
- **Size:** S–M (UC).
- **Counterexample that must stay U:** `let held` reassigned on some path, or `held` passed to an unknown function before the write.
- **Coverage:** OW (ADR 1064, not built).

### Smaller groups worth a note

- **M22 `cmp-cond` (S).** `createMemo(() => (c ? Promise.resolve(1) : 1), { sync: true })`: one branch returns a Promise, so if that branch can run, Chrome throws. The `cmp-any`/`cmp-unknown` casts are excluded as adversarial.
- **M21 (M).** `<Loading on={props.on}>` where `props.on` comes from a signal accessor through a component prop: no U at the site, only a remote one.
- **M12 `comp-slot` (M).** `({ slot: (c) => c() }).slot(Child9)`: a method of a literal object called in place. ADR 0456 says a member-built function is not followed.

## 6. Fixtures that README-mark a known miss in plain code (listed, not run)

Plain code:
- `reactive-ir/local-alias-bindings`;
- `returned-by-reference` C1 (a memo's returned closure called in the body; hits R-like reasoning);
- `stored-throw-dynamic-mount` MountsBodyRead (SC4002 through `dynamic`, `component-identity-unproven`);
- `parameter-list-owner` NestedCalledOmitted / IifeDefault (G9);
- `pending-read-closed-bundle` (object bundles: `Destructured`, `QuotedKey`, `WrappedInArrow`);
- `callee-spelling` CleanupBind (a `.bind` spelling, not simple);
- `action-resume-flush` CodeBeforeYield (a generator, not simple).

docs/open-work.md section 1 names the matching ADRs (0517, 0539, 0718, 0961, 1038, 1064, 1124/1138, value-use census).

## 7. Files (all in /Users/thomas/dev/sc-scratch.noindex/r3/)

Runs:
- `base/`, `mxr/`, `lgr/`: focus runs (programs.jsonl per grid or code);
- `all.log`: run log.

Selection and classification:
- `select.mjs`: simple recall slice → `mx-recall-simple.json`, `lg-recall-simple.json`;
- `*-blockers.jsonl`: blocker-scan output;
- `u-flat.jsonl`: every U row with its at-site kinds;
- `classify.mjs` → `u-class.jsonl`: features and SIMPLE flag;
- `family.cjs` → `u-fam.jsonl`: family per simple row.

Arms and probes:
- `armscan.mjs` → `arms.jsonl`: role-log arm at the site, one program per group;
- `probe.mjs <grid> <id>…`: program, site findings, blockedBy (`ROLES=1` adds arms);
- `show.mjs`: program text.

Binaries: `bin/` (release checker and producer at b600297eb).
