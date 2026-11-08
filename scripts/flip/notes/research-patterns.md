# The 133 open false cleans and 3 false positives: root causes and smallest fixes

Code read at `879eca007`. Lines are from that tree. Build `rk2`. Causes C1-C7 are `census-research/report.md` and are not repeated. Patterns here are P1-P13, F1-F3.

## Answer

**The census and bounds drafts close 117 of the 133 (measured on a build that no longer exists). 16 stay certified, from 9 causes, all small or medium. But three holes matter more than those 16: a store or accessor that reaches a carrier through a call result, a prop or a context (P12), a source with no binding (P13), and the bounds draft's cleanup test, which only works because every `types/` program happens to call `getOwner` (P2).** I found 24 live false cleans outside the 133 and 3 false positives outside the list, all `tsc`-clean, all Chrome-confirmed. No fix was built, so every "after" is inferred.

Every row is in exactly one cause: 90 + 27 + 3 + 2 + 2 + 3 + 1 + 1 + 1 + 1 + 2 = 133.

**Measured**
- All 139 run through a private copy of the lane on `rk2`: 139 of 139 fail as listed (3 FP, 133 FC, 3 no run). Chrome confirms the claimed code on all 133. Install: `/tmp/sc-scratch.noindex/rules-audit/install`, rc.21, the only rc.21 copy outside the agent's worktree.
- `tsc` (tsrs) on all 139: 129 pass.
- About 130 new probes through `rk2`, `tsc`, Chrome. Table in `probes.all.json`. The 3 new false positives carry a log line that proves the site ran (`p10/`).
- Role log at each straggler's site.
- `origin/claude/flip-writes` still lists all 139 open.
- Census and bounds together: `flip-reads/openfc-dbg6.txt`, written 06:05, debug build, **before the last merges**. That binary is gone: `rdbg/` is from 02:27 and has no census. So "the census closes it" is measured for rows that file holds, on an older tree, and inferred for today's tree.

**Inferred**: every fix, every "after", and every statement about what the census does to a probe. The census was not run on any probe.

### The list is wrong in three ways

- **7 of the 133 are rejected by `tsc` as written**, so they do not count by the rule: `a11`, `a20`, `b05`, `b06`, `d01-dispose`, `d07` (a `createOwner` or boundary import from `solid-js`, whose main types lack it; `a20` calls `.dispose` on `Owner`), `d6-comma` (TS2695). I re-ran twins of 6, and they pass `tsc` and fire in Chrome, so the causes stay: `b05` and `b06` from `solid-js/internal`, `d07` and `d01-dispose` without the unused import, `a20` with a cast, `d6-comma` with a used left side. `a11`'s twin was not valid (`createErrorBoundary` is in the same import line), so it is unconfirmed. **Fix the programs, keep the causes.**
- **`h03` is no false positive, it is a bug in the program.** `after()` appends to `document.querySelector("div")`, which is `#root`, not `el`, so the observer never fires. The corrected twin (`F2-h03-twin/m1-mutation-twin`) reports `NO_OWNER_EFFECT` and the checker's SC4001 V stands. No code change.
- **`g12` is rejected by `tsc`** (`setS("a", 2)`, TS2554). Its cause is real: twin `s1` with a valid setter is the same FP.

The 3 no-run rows are not defects: `e807` (`1.length`) and `e911` (`on:click`) fail `tsc`, `f08` does not compile through the plugin (a decorator).

The other 18 of the 34 stragglers are off the list now, and all 18 report on `rk2` (16 SC2003 U, 1 SC2003 V, 1 SC2001 V).

## 1. Root causes

Size is the edit. Each still owes the plan's steps 3-6, including the fuzzer shape.

| # | Root cause | Rows | Minimal program (`by-cause/` or pattern) | Path that answers clean | Smallest fix | Ends as | Size |
|---|---|---|---|---|---|---|---|
| **P1** | A proven source's reference leaves through a position no row follows: a carrier, a logical, a `let`, a class field, a member, a helper parameter | **90** | `api/r01`, `api/v02`, `values/d4-string-ctor`, `values/t5-helper-module`, `types/h08` | No row exists. A call of a value has no symbol: `local_access.rs:2777` returns. A member on an alias root has no source | Land `value_use.rs`, `census-block.rs` | U, SC9012, blockers below | Medium, drafted |
| **P2** | A read in a cleanup: nothing places it, and the last arm of the function is a silent default | **27** | `types/x01`, `types/a02` | `callee_roles.rs:698` `allowed-unplaced-nested`, `DeferredCallback`, `default=true silent=true` (role log on `a01`, `x03`, `b1`) | Bounds hunk, **plus a project-wide disposer test** (section 2) | U, SC1001, `function-run-unplaced` | Small, but the draft has a hole |
| **P3** | A lying cast: `(t as unknown as Promise<void>).then(fn)` resolves to `Promise.then` from the written type | **3**, `d01-cast d07 x30` | `P3-cast/e4-cast-any-then` | Arm `fresh-stack-callback` (role log), `callee_roles.rs:1731`, from `indexes/callees.rs:389` | In `subclass_veto`'s spot: a standard method on an `as` receiver whose inner value is not proven that container answers nothing. Both `callees.rs:400` and `base/memo/build.rs:1731` | U | Small |
| **P4** | A patched global or prototype. ADR 0683 names it a premise; the rule says it is unproven | **2**, `e107 e108` | `P4-patched/*` (`window.setTimeout =`, `const P = Promise.prototype; P.then =`, `defineProperty(globalThis, …)` are all live) | Same arm as P3 | Extend `hazards()` (`base/contributions.rs`) with assignments and `defineProperty` to a standard global or `.prototype` member, and `subclass_veto` with `Hazard::StandardOverride` for a bare name. `subclass_veto` returns early on a name with no dot, so that needs its own arm | U | Small to medium |
| **P5** | `export * as Solid from "solid-js"` names no primitive | **2**, `d7-export-star-as`, `-store` | `P5-star-as/s2-star-as-reexport` | `symbols.rs:32` walks `ast.imports` only. Proved by `s6`: the same program plus any `import * as X from "solid-js"` in the project gives V | Same arm over `ast.exports` with `namespace_binding` | **V** (`s6` measured SC1001) | Small |
| **P6** | `createErrorBoundary`, `createLoadingBoundary` return an accessor but are no reactive source | **2**, `b05 b06` | `P6-boundary/b05-internal` | `solid_2.rs:2422` `creates_reactive_source` lacks them. Same shape as ADR 0690 | Add both | **V** | Small, 2 lines |
| **P7** | A dependency's reactive read reaches the project with no project reference | **3**, `d8-pkg-getter-export`, `-side-effect-import`, `-types-lie-plain` | The patterns | Getter: `resolver_dependencies.rs:268` reads `object_get_shapes` only, so `Object.defineProperty(api, "v", { get })` is no getter. The dep read has a row (`Unknown`, `callee_roles.unplaced`) and no report. **The other two not traced** | Getter: read a descriptor literal as a shape (small, proved or U). Other two: below | U | Small + medium |
| **P8** | A computed-key delete on a store is silent by design | **1**, `d6-delete-computed` | `P8-delete/d2-delete-computed`, `d3` | `shared_reactivity.rs:4978`. Its doc says so. `t3-delete-reflect` and `st["a"] = 5` are V | Drop the computed test there and at `demand_plan.rs:130`, so the optionality question reaches it. A required key stays silent (`t4` is TS2790, invariant 1). No entity answers U, as that doc already says | V or U | Small. **Upstream port: run `upstream-parity`** |
| **P9** | A generator stepped by `for await` before any await | **1**, `f02` | The pattern. `q1`, `q2` are already U | `after_await.rs:286` skips an async generator as an entry, and `:1202`'s step arm is only walked after an await. The site has role `Unknown` (`window_after_awaits.unknown`) and no finding. **Why `Unknown` stays silent: not traced** | Walk a `for await` operand that calls a project generator through `report_prefix` + `GeneratorStepped` | U, SC1002 | Small |
| **P10** | An inline `onLoad` in a control-flow callback | **1**, `h18` | `P10-denied/cd-iframe-keyed-for` | `control_flow.rs:91` returns `DeferredCallback` first, so `execution_role.rs:361`'s denied-event arm never sees it. In a body it is U (`h3`) | At `:91`, if `in_denied_handler(file, span)`, `Unknown` | U | Small |
| **P11** | A census arm answers `Followed` too early | **1**, `t4-template-array-empty` | `P11-census-followed/y4 y6 y7` | `value_use.rs` `Holder::Coerced` opens only for `Root`, `RootParameter`; `Holder::Tag` always follows | Open `Coerced` for `Holder`, `RootHolder` too. `Tag`: open | U | Small, one match arm each |

Live false cleans with **no row on the list**:

| # | Root cause | Probes | Path | Smallest fix | Ends as | Size |
|---|---|---|---|---|---|---|
| **P12** | A store or accessor reaches a carrier through a call result, a prop or a context. The census trusts "the handed net", and the net only covers calls, not roots | **6**: `JSON.stringify(useS())`, `String(useS())`, `JSON.stringify(id(st))`, `Object.values({ f: useC() })[0]()`, `JSON.stringify(useContext(SCtx))`, `JSON.stringify(h.s)` | `value_use.rs` `returned()` and `jsx_value()` answer `Followed` | `returned()`: walk each call of the function with `position(call.span, mode)`; unknown callers open. `jsx_value()`: walk `props.<name>`. Provider: walk `useContext` calls. Keep one hop | U | Medium |
| **P13** | A source with no binding has no symbol, so the census never starts | **4**: `{ a: createMemo(…) }` then `Object.values`, `[createSignal(1)[0]]`, `JSON.stringify(createStore(…)[0])`, hook `{ a: createMemo() }` through `Object.values` | `census-block.rs` starts from `ctx.accessors` keys. This is the handoff's `hookm` lead, wider | In the block, also start `position(…)` from every source-creating call, or its `[0]` slot, whose holder is not a `Binding`. The hook probe also needs P12 | U | Small to medium |

False positives:

| # | Root cause | Rows | Path | Smallest fix | Ends as | Size |
|---|---|---|---|---|---|---|
| **F1** | A component mounted inside a cleanup. rc.21 skips the write guard while `disposalDepth` is non-zero (`setSignal`, `dev-shared.js:6585`) | `e703`, `g12` (+ `s1`, `s2`) | `write_roles.rs:799`: `ComponentStatus::Proven => NotIn` | Walk the component's mount sites (`mount_sites_within`, `async_context.rs:190`): all in a disposal is `Only`, some is `Unplaced`, none is `NotIn` | Proved clean, or U | Small to medium |
| **F2** | Program bug, `h03` | 1 | None | Fix the selector in the pattern | V stands | Trivial |
| **F3** | **New.** The lying cast the other way: `(t as unknown as number[]).forEach(cb)` with a `t` that defers. SC1001 V, Chrome silent | `P3-cast/s1-then-cast-array` | Inline-callback proof reads the cast | The P3 veto in `resolved_callee_call` covers it, if it is not narrowed to schedulers | U | With P3 |

Other 24 live cases by cause: P1 1 (`r07`, `Object.values(props)[0]()`: the census's props walk reaches it), P2 3, P3 1, P4 4, P8 1, P10 1, P11 3, P12 6, P13 4. `b2` (cleanup in another file) is a 25th shape: U only on SC4001, which does not cover the code.

### P1 by blocker (`openfc-dbg6.txt`)

| Blocker | Rows | Local proof available later |
|---|---|---|
| `value-in-carrier` | 43 | `Reflect.apply(c,…)`, `[1].map(c)`, `[1].forEach(setW)`, `String(st)`, `JSON.stringify(st)`: resolved standard calls, could be V |
| `value-root-in-helper` | 10 | One hop, parameter read in the body: could be V |
| `value-in-class-field` | 6 | |
| `value-to-unresolved-callee` | 7 (1 also P2-blocker) | |
| `value-held-in-member` | 5 | |
| `value-held-in-binding` | 5 | `let s2; s2 = st` with one write: could be V |
| `value-through-logical` | 4 | |
| `value-from-primitive-callback` | 3 | `createRoot(() => c)`, `untrack(() => c)`: could be V |
| `value-as-parameter-default` | 3 | Could be V |
| `value-bound-held` | 3 | |
| `value-through-new` | 1 | |

Sum 90. `api/v21`, `v22` also carry `value-use-unclassified`: that default fired on a real program, so read those two before landing.

## 2. What the census leaves, and what is at risk

**The 34 stragglers are `flip-reads/openfc-dbg6.txt`.** 16 are open: `f02`, `b05`, `b06`, `d01-cast`, `d07`, `e107`, `e108`, `h18`, `x30`, `d6-delete-computed`, `d7-export-star-as`, `d7-export-star-as-store`, `d8-pkg-getter-export`, `d8-pkg-side-effect-import`, `d8-pkg-types-lie-plain`, `t4-template-array-empty`. The other 18 closed by other means: `h02`, `d2-handler-link`, `d2-identity-return`, 9 `d2n-*`, `t2-for-row-root-array`, `t2-for-row-write`, 4 `t6-*-helper`.

| Draft | Closes | Leaves |
|---|---|---|
| Census | P1 (90) | P3-P11 (16 rows), P12, P13 |
| Bounds | P2 (27) | P2's hole |

**P2's hole.** `disposal_reachable_here` (`p-bounds.diff`) keeps a cleanup silent unless the file has a `getOwner` call or a `createRoot` callback around the cleanup that names `dispose`. **46 `types/` programs hold `getOwner()` in their prelude helper `own`, unused by the defect.** So at least `a09` and `a10` closed by accident. Live, no `getOwner` in the file, `rk2` certified, Chrome warns: `b1` (`render()`'s dispose), `b4` (cleanup in a helper called under `createRoot`). `b2` (cleanup in another file) has the same shape. Inferred after bounds: `b1`, `b2`, `b4` stay clean, `b3` closes because the handle is called. **Fix: the test is project-wide, not file-wide:** any `createRoot` with a named first parameter, any `render` or `hydrate` result that is not discarded, any `getOwner` or `createOwner`, memoized on the lookup. Better later: a handle called only in an event handler is quiet (`g6`), and one called in a body is V where the cleanup is lexically under that root (`x01`, `c37`).

**Cost.** A project with any disposer makes every cleanup read U. `g1`, `g2`, `g4`, `g5`, `g6` are quiet in Chrome. That is not a false positive, but corpus cost is unmeasured for both drafts.

**Census over-reach.** `Object.keys(st)` is quiet (`y3`), and the census would answer `value-in-carrier` (inferred). `"a" in st` and `for…in` stay followed. An arm for key-only calls keeps those certified.

**Census site.** The census's U sits at the reference, not the read. The pattern lane judges per program, so it passes. I did not check how the grid judge places it.

**d8, the two not traced.** `side-effect-import` needs a call through `globalThis as any` to a key a dependency wrote. Conservative answer: a selected dependency that assigns a global member a function reading a source raises U at its `import`. `types-lie-plain`: the import resolves to the `.d.ts`, so no store source is known (inferred). Both medium, 1 row each.

**Guards** (`by-cause/guards`, all `tsc`-clean except `t4`, all run through Chrome and `rk2`):

| Guard | Chrome | `rk2` | After |
|---|---|---|---|
| `g1`, `g2`, `g4`, `g5`, `g6` cleanups with no strict stack | quiet | certified or U on an unrelated `after` write | no V |
| `g3` effect cleanup rerun | warns | U | stays U |
| `s5`, `s6`, `s8` real promise, cast to its own type, timer | quiet | certified | **stay certified** |
| `s7` `any` cast to `Promise` | quiet | certified | may go U, never V |
| `s3-star`, `s4-import` | warn | V | stay V |
| `g-boundary-jsx` read in JSX | quiet | certified | **stay certified** |
| `t4` required computed delete | warns | silent | **stay silent, TS2790** |
| `q4` async generator, read before its first await | quiet | certified | no V |
| `h1`, `c7` handler, no sync delivery | quiet | U, certified | no V |
| `c1`, `c4` nested function in a callback | warns | V | **stay V** |
| `s3-write`, `s4-write` | quiet, warns | certified, V | `s4` **stays V**: the F1 trap |
| `y1`, `y2`, `y3`, `q01` | quiet | certified | see over-reach |

## 3. Build order

Rows per unit of work. Merge W's `138ff86e7`, `91311857e` first: they touch `effect_phase.rs`, `static_rules.rs`, `after_await.rs`, `static_api.rs`, `native_promise.rs`.

| Order | Work | Rows | Live extras | Effort |
|---|---|---|---|---|
| 0 | Fix the 9 programs: 7 false cleans and `g12` that `tsc` rejects, and `h03`. Drop or fix the 3 no-run | 1 FP gone | | Trivial |
| 1 | P6 | 2 V | | 2 lines |
| 2 | P5 | 2 V | | Small |
| 3 | P10 | 1 | 1 | Small |
| 4 | Census as drafted, with P11's arms | 91 | 4 | Medium, drafted, then read the sweep |
| 5 | P12, P13 in the census | 0 | 10 | Medium |
| 6 | Bounds with the project-wide disposer test | 27 | 3 | Medium, drafted |
| 7 | P3 + P4 + F3 | 5 | 5, and 1 FP | Small to medium |
| 8 | F1 | 1-2 FP | | Small to medium |
| 9 | P8 | 1 | 1 | Small, plus parity |
| 10 | P9 | 1 | | Small |
| 11 | P7 | 3 | | One small, two medium |

Expected, not run: 0 false cleans, 0 false positives, 4 or 5 proved V, about 128 U. Corpus cost unknown; census and bounds together will be well over the review threshold of 20, so a Fable review is owed.

## 4. Split

| Piece | Causes | Changes | Collides with |
|---|---|---|---|
| A | P1, P11, P12, P13 | `value_use.rs` (`through`, `returned`, `jsx_value`), `census-block.rs` in `reactive_analysis.rs` after `:318`, `read_only` near `:369`, `lib.rs` `mod` line. Also `scripts/fuzz/flip-read.mjs` | One agent, or P12, P13 after P1 lands: all one file. `lib.rs` mod line with nobody |
| B | P2 | All of `p-bounds.diff`: `callee_roles.rs`, `local_access.rs`, `effect_phase.rs`, `static_rules.rs`, `after_await.rs`, `async_context.rs`, `resolver_dependencies.rs`, `shared_reactivity.rs:1717`. New lookup memo in `indexes/` | D (`after_await.rs`), E (`resolver_dependencies.rs`), W's merge. Land before D and E |
| C | P3, P4, F3 | `base/contributions.rs` (`hazards`, `subclass_veto`, `plain_container_receiver`), `indexes/callees.rs:400`, `base/memo/build.rs:1731`. Maybe a `TransparentWrapper` kind in `solid-facts/src/ast` | None. A new `Hazard` arm changes base cache hashes: run `ir-edit-fuzz` |
| D | P9 | `after_await.rs` near `:286`, `:1202` | B, W's ADR 0750 |
| E | P7 | `resolver_dependencies.rs:268` and `:47`, `result_reads.rs` `dependency_getter_obligations` | B |
| F | F1 | `write_roles.rs:799` `runs_in_disposal_within` | B reads `runs_in_disposal`, and does not edit it. Agree `Only`'s meaning first |
| G | P10 | `control_flow.rs:91` | None |
| H | P5, P6 | `symbols.rs:32`, `solid_2.rs:2422` | None |
| I | P8 | `shared_reactivity.rs:4978`, `solid-facts-backend/src/demand_plan.rs:130` | B's `:1717` hunk is disjoint. Past W's `:3299+` line, W is finished |

**Parallel now:** A, C, F, G, H, I. **After B:** D, E. All pieces share `findings-snapshots`, `rediscovery.json`, `open-defects.json`, `cases.json`, `docs/decisions.md`: union the JSON lists, regenerate snapshots. Next ADR 0694, 0697-0699, then 0752 up.

## Not checked

Hydrate and SSR hosts. The `grids` half of `open-defects.json`. Corpus effect of any fix. Whether the census or bounds drafts still apply cleanly (cut before the merges). Whether `event_keeps_delivery_proof` denies more than `onLoad` in P10's path. Why an `Unknown` read in `f02` gives no finding. Where dependency-file findings are dropped. `tsc` on `b2`'s helper file through the lane's config (my helper's tsconfig). Anything on `a11`'s twin.

Working files: `/tmp/sc-scratch.noindex/flip-research/patterns`: `by-cause/`, `cases/`, `join.tsv` (`rk2` and `dbg6` per row), `chrome.rows`, `lane-out/patterns.jsonl`, `probes.all.json`, `role.mjs` (role log at a regex), `chk1.sh`, `run-probes.sh`, `chrome/run.mjs`, `lane/`.
