# SC9012, W's leftovers and the fuzzer's gaps: root causes and smallest fixes

Code read at `879eca007`. Corpus report `sweep-k2.json`. Build `rk2`, the newest one that matches the handoff (`rm7` and `rt0` also ran on the 10 probes; same answers).

## Answer

**SC9012 is not flipped, and most of what remains is not in the 8,653 rows.** Those rows all name a blocker and none of them has to shrink. What blocks the flip is sites with no row at all.

- **148 live false cleans** in new probes: `tsc` passes, Chrome fires, `rk2` certifies. Plus the 10 known probes.
- **The census closes most of them, but not 3 root causes it trusts** (N1, N2, N3 below). It marks those positions `Followed`, and no row stands behind them.
- `check.mjs SC9012` is **51 of 83**. 2 of the 32 failures are not census at all (`f02`, `p03`).
- `c ? useThing : useThing` **is closed** on `rk2` (ADR at `976e73028`), but no fuzzer shape or fixture holds it, so by AGENTS it is not closed yet.
- Item 5 needs no Rust: the place exists in the page's record and the judge drops it.

Measured: 786 new programs through `tsc`, `rk2` one-shot and real Chrome (3 rejected by `tsc`, not counted). 15 old probes on `rk2`. 136 of 136 `LOADING_ON_OUTSIDE_HOLD` rows in Chrome. 1,794 programs through a warm daemon. Of the 148, 5 are an uncaught error with no diagnostic code (`p03`'s family); the rest report a code.
Not measured: **no fix was built.** Every "after" is inferred from the code. Corpus cost of every fix is unmeasured; the only number given is a labelled upper bound.

## 1. The rest of SC9012

### What "flipped" requires, given uncertifiable is an acceptable end

SC9012 never warns, so only the clean direction counts. It is flipped when no dispatch the checker cannot resolve stays silent. **Shrinking any of the 8,653 is not required.**

**Required now:**

| # | Work | Why | Closes |
|---|---|---|---|
| R1 | N1: `handed_member_callee` one-step gate | Census marks these followed | 9 of the matrix's 21 `nested` programs, `n1`-`n9` |
| R2 | N2: parameter obligations truncated at tracked roles | Census marks these followed | `run(setS)` in a memo, JSX or compute; `a13` async |
| R3 | N3: `handed_call` tail | Census `returned()` is `Followed` because "its callers are the handed net's" | `c7 c9 c11 c12` |
| R4 | Census, ADR 0692 (`w0/hold/value_use.rs`), after R1-R3 | The rule's main body | 30 of 32 witnesses (inferred; R's debug build, before the merges), the 10 probes, 8 forms of the matrix |
| R5 | `f02` and `p03`: move to their own rules, section 3 | Not census | 2 witnesses |
| R6 | Bounds, ADR 0695 | Its silent default `allowed-unplaced-nested` is a false clean | R's list |
| R7 | The census report's C1, C6, C2 | The async half of the same gates | 684 rows |
| R8 | A test that an unknown form hits `value-use-unclassified`; `check.mjs` accepts SC9012 for the 9 ids; `blockers.mjs` stays 0 unnamed (measured: 21,616, 0 unnamed) | The default arm is the flip | |
| R9 | Fuzzer shapes, section 4 | AGENTS: a defect closes only when the fuzzer rediscovers it | |

**Can wait:** every row in the table below. So can V proofs for anything now U.

**Owner's review rule:** R1, R2 and R4 each add over 20 U, so each needs the Fable review before merge. Upper bound for R1: 546 lines match `props.x.y(` in the frozen corpus, before the standard-library filter and the role gate.

### The 8,653, by blocker

All are U. 829 are in test files; 7,679 distinct sites. "Hole" means a sibling shape that has no row today, so it is a false clean; "U only" means a later proof could shrink it and nothing is at risk.

| Group | Rows | Blockers | State | Hole, measured | Later proof |
|---|---|---|---|---|---|
| Handed value | 4,424 (802 in tests) | `handed-member-dispatch` 3,354, `handed-member-read` 1,070 | **Hole** | N1, N3, C1, C7 | C1b sync/async/open; bind the hook's argument at the call (F0's follow-up); a receiver with a proved non-Solid type |
| Parameter | 2,563 | `closed-parameter-member-dispatch` 822, `parameter-use-unmodeled` 670, `parameter-member-target-unresolved` 626, `exported-parameter-member-dispatch` 227, `parameter-member-path-unresolved` 112, `parameter-default-unresolved` 41, `parameter-member-targets-diverge` 41, `parameter-call-argument-unproven` 24 | **Hole** | N2 | Writes are legal in an apply: `a5` is U and quiet |
| Untrack | 188 | `value-escaped-from-untrack` 152, `value-read-under-untrack` 32, `derived-read-under-untrack` 4 | **Hole** | `untrack(() => c)()`, `createRoot(() => c)()` certified: census default `value-from-primitive-callback` | |
| Prop getter | 130 | `async-prop-getter` | **Hole** | Member tag, section 3 | V at a body read: `m0` is U, and Chrome throws |
| Literal member | 108 | `withheld-object-member` | **Hole** | C1 literal member | Closed literal over an async source |
| Leaf callback | 561 | `leaf-callback-opaque` | U only | 0 in 240 behaviour programs | Resolve the synchronous body; 396 are `app-game` |
| Listener | 492 | `listener-reach` | U only | 0 in 240 | The dispatch veto proof (ADR 0435) |
| Primitive value | 80 | `primitive-value` | U only, **wrong text on 77** | | See below |
| Small | 107 | `returned-callable-outside-source-universe` 44, `installed-member` 23, `computed-call-target-unresolved` 17, `dependency-source-getter` 11, `type-only-source` 6, `package-callee` 3, `primitive-result-method` 2, `composite-callee-unresolved` 1 | U only | | |

Total 8,653.

**240-program matrix** (`mx/`): a reactive *behaviour* in a visible body, behind 5 dispatch forms, 8 positions, 6 behaviours. 0 certified where Chrome fires: `function-run-unplaced` holds them. **So the holes are hidden values, not hidden bodies.**

**378-program matrix** (`vx/`): a *value* (accessor, setter, async memo) through 18 forms at 7 positions. **78 certified where Chrome fires:**

| Form | False cleans | Closed by |
|---|---|---|
| `nested` `<K o={{ go: v }} />`, `props.o.go()` | 9 | **N1, not the census** |
| `run-param` `run({ go: v })` | 4 | **N2, not the census** |
| `hook-slot`, `gen` | 1 each | `jsx/async`: C1 |
| `array-from` | 10 | Census `value-in-carrier` |
| `default`, `spread-call`, `tag`, `untrack-ret` | 9 each | Census |
| `root-ret` | 7 | Census |
| `bind` | 6 | Census `value-bound-held` |
| `object-values` | 4 | Census |

"Census" here is by reading the draft; only R's earlier debug probes ran it. No build in scratch holds it.

### The three things the census does not close

| # | Root cause | Minimal program (Chrome, `rk2`) | Path that answers clean | Smallest fix | Ends |
|---|---|---|---|---|---|
| N1 | `handed_member_callee` stops at one member step, so `props.label.trim()` is not an obligation. The call path (`:2803`) already filters standard-library methods first, so there the gate adds nothing and only drops `props.o.go()` | `n1`: `<K o={{ go: setSig }} />`, `props.o.go(1)` in a memo: REACTIVE_WRITE, certified. `n9`: `props.o.go()` in a body: STRICT_READ, certified. Compare `n3`: `const o = props.o; o.go(1)` is U | `local_access.rs:3562-3571` | Lift the one-step limit at `:2803` only, through a flag or a sibling function. **Do not delete the early return:** `:4408` (`member_get_may_read`) asks it about plain member Gets, so `props.a.b` would open there | U `handed-member-dispatch`. `n7` (plain literal, quiet) goes U too |
| N2 | `excluded_role_holds` says true for `TrackedJsx` with no named callback, and its reason is about reads only. A value handed to a helper's parameter may write or be pending there. Writes through a parameter have no row, unlike reads (`add_invoked_parameter_reads`) | `a1`: `run(setS)` in a memo: REACTIVE_WRITE, certified. `a9`: ACTION_CALLED. `a13`: `<p>{peek(a)}</p>`: ASYNC_OUTSIDE. Compare `a2`, same call in a body: U | `interproc/result_reads.rs:303`, truncate at `:328`; `execution_role/body_roles.rs:706` | Keep the obligations when an argument passes `handed_value`, or names a setter, action or async source. `is_source` in `handed_value` is gated on `depth < HANDED_DEPTH`, so call it one level down. `r9` stays certified | U. Later: an `add_invoked_parameter_writes` row gives V for an exact `run(setS)` |
| N3 | `handed_call` ends in `false` for any callee no arm resolves | `c7` `let hook` rebound; `c9` `[useThing][0]()`; `c11` `useThing.call(null)`; `c12` `useThing.bind(null)()`: all throw in Chrome, all certified | `local_access.rs:517-541` | The rebound binding answers true. `dispatch_target` (`.call`, `.apply`, `.bind`) answers as its target. A computed member callee answers true | U `handed-member-dispatch` |

### `c ? useThing : useThing`

**Closed.** `local_access.rs:517-531` answers true for a chosen hook. R's 5 probes (`v1/a b c`, `v2/a-nosib c-nosib`) and my `c1`-`c5`, `c13`, `c14`, `c17`, `c18`: U on `rk2`, and Chrome fires on all of mine. `rm5` certifies `c1` and `c13`, so an old build fails. Negative `c16` (handler) stays certified.

**Still owed:** a fuzzer shape and a fixture. `rg` finds neither. Text variant: `const useA = () => ({ read: ${o} }); const hook = Math.random() >= 0 ? useA : useA; void hook().read();`.
**Can wait:** `c15`, two plain hooks, is U and quiet. Answering each operand through `handed_call` instead of `true` would prove it.

### `primitive-value`: 77 rows name a primitive nobody wrote

`hoxxes-briefing` 71, `inferay` 4, `spookysoftware-www` 2 say "Solid primitive createOwner is used as a value" at `css.raw(...)`. `css` comes from `styled-system/css`, a generated module not in the frozen tree. This is the misattribution of `rediscovery.json` #192, whose "not reproduced outside those two projects" is false. Harmless (U, internal). Fix: `static_rules.rs:228` asks W's `primitive_identity_proven` (`cleanup.rs`, ADR 0711), and otherwise a blocker that does not name a primitive.

## 2. W's leftovers

`docs/open-work.md:223` is the list. All three are SC2003 writes to a store. One cause: **`ParameterFlow::value` is silent by default.** Everything no row recognises falls to `shared_reactivity.rs:2249`, then `(None, false) => return` at `:1821`.

| Leftover | Minimal programs (`w/`, all warn STORE_WRITE, all certified) | Path | Smallest fix | Ends |
|---|---|---|---|---|
| Destructured loop binding | `dl1` `for (const { p } of st.ls) p.n = 2`, `dl2` `[e]`, `dl3` rest, `dl8` `st.ls.forEach(({ p }) => …)`, `dl11` `const { p } = st.ls[0]`, `t34` | `:2219` `for_of_operand` and `:2232` both require `Identifier`; `plain_parameter_at` `:1843` too | Generalise `for_of_operand` to any pattern; a pattern binding's initializer or operand goes to `value` with `linked = true`. A parameter pattern calls `parameter` with the same | U `unproven-container-root`. Links are never proved without a wrap fact |
| Nested identity | `ni2` `outer(st)` with an inner `id`, `ni3` `via = (x) => id(x)`, `ni5` IIFE | `:2494` `returned_parameter` needs the return to be the parameter itself | Take the one return expression and run `value` on it with a frame for this call. Two frames do not nest (`FlowFrame` holds one): past one, pass `in_place = false, linked = true`, or `:2196` walks every caller of the outer function and a plain caller's sink becomes a **false V** | V for an unlinked root, as ADR 0720; else U |
| `Object.entries` | `oe1` index, `oe2` for-of, `oe3` `forEach`, `oe4` `map`, `oe7` `fromEntries` | `:2351` `_ => return false` | **The default arm, not one row:** with `linked`, probe receiver and arguments; a store there is open. Same closes `fromEntries`, `Object.assign`, `flat`, `sort`, `Array.of`, `Map.values` | U `unproven-container-root` |

**The same silence hides 17 more** (`w2/`, `tsc` and Chrome pass, all certified): `t1` `(c ? st.a : x).n`, `t4` `[st.a][0].n`, `t5` `({ s: st.a }).s.n`, `t7` `let r; r = st.a`, `t13`, `t16`, `t17`, `t18`, `t20` rest parameter, `t21` and `t22` destructured parameter, `t24` `delete` through a call, `t30`, `t31` `await`, `t32` `.then`, `t33` generator, `t38`.
**One helper closes most:** `expression_holds_store(file, ctx, span)`, any identifier in the span (not in a nested function) that is a proven store. Use it at the tail `:2249` on the initializer, and at `:1699-1704`, where a root that is neither bare nor a call returns silently.

**Guards, quiet and certified today:** `dl6`, `dl7`, `ni7`, `oe5`, `oe6`, `t39`, `t40` must stay certified. `dl5` (a write to a rest copy) stays certified only if a rest slot with `linked == false` is treated as fresh. `dl10` (rebound `let`) goes U.
**W's rule of thumb holds:** U only, so no V at risk, except nested identity's frame above.

## 3. Item 3: `async-value-member`, the 32, `g2d`, the 10 probes

**The 71 are not false cleans by the owner's definition.** All 71 are U at the site on `rk2`. The judge says false-clean because SC1001 does not decline PENDING_ASYNC_UNTRACKED_READ. Chrome: `m1` `m2` `m3` `m7` throw the code.

- Cause: `<ui.C a={a()} />` has no entry in `uses`, so `async_getter` is never set. `post_rows.rs:529` calls `trace::render_target`, which reads the symbol at the tag only. The plain tag `m0` gets SC9012 `async-prop-getter`; `m1` gets only `read-backing-unproven`.
- Fix: in pass 10, for a member tag, take the value of the holder's slot and accept it when `compound_value` (ADR 0342) has accounted it. Same-file holders first; an imported holder stays as it is. Keep `escapes` as it is: `async_getter` reads `Escaping` too.
- Ends: U, as `m0` (SC1001 `source-pending-unproven`, SC9012 `async-prop-getter`, SC5003). The judge reads that as recall-gap: withholding. Inferred.
- Stays true: `m5` (a written slot) fails the holder test and keeps today's answer. `m4`, `m8`: quiet, unchanged.

**The 32: still unexplained, and now smaller.** All 32 are moot on `rk2` only if the next grid agrees, so they need one rerun.

| Tried | Result |
|---|---|
| Warm daemon, one slot, strides 2, 3, 5, 7, all `store-*` ref-row programs | 1,584 programs, 24 targets, 0 certified |
| Warm daemon, strides 1, 2, 3, 5, 7, `cbp-jsx`, every answer against one-shot | 210 programs, 8 targets, 0 certified, 0 differ |
| Folding or pruning of rows | None exists: `fold-worker.mjs` folds role logs only |
| Findings filter in `parseFindings` | App.tsx only, which matches |
| Builds | `rm5` (04:18) and `rm6` (08:14) bracket the grid (06:07), and both answer U on `wrapper-none`. `direct` and `alias` moved between them, `wrapper` did not. The grid's own binary was `release build`, a live path, and I cannot name it |

`rerun-ids.mjs` or a grid was out of bounds here (no grids, and no install in this worktree). **Next step, one line:** on the next `mx-recall`, add `--warm-verify` for `STRICT_READ_UNTRACKED ref-row` and `ASYNC_OUTSIDE_LOADING_BOUNDARY cbp-jsx`. A difference there is a daemon bug and a real one; no difference means the 32 are closed by ADR 0691.

**`g2d` is the census report's C2.** All measured on `rk2`, Chrome ASYNC_OUTSIDE: `g2d-map-body`, `g2d-foreach`, `g2d-apply` (`hf.call(null)` in JSX), `g2d-module-hf`. Inline `g2d-inline` is V and `g2d-plain-call` is U, so only the named hand-off is missing. Path: `handed_role`, `local_access.rs:9384`, with `reports_untracked_read` as the predicate (`:8776`), which is false for tracked JSX. Negatives quiet: `g2d-neg-empty`, `-handler`, `-under-loading`. The fix and the `timing_at` trap are in the census report; `g2d-apply` and `g2d-module-hf` are two more positive twins for it. The fuzzer gap is `rediscovery.json` #199, section 4.

**The 10 probes are all census.** `g06 g08 g09 g10 g12 g18 g22 g25 h02 h04`: certified on `rk2`, `rm7` and `rt0`. Reading `value_use.rs`, each hits an open row:

| Probe | Row |
|---|---|
| `g06`, `g22` | `value-from-primitive-callback` |
| `g08`, `g09`, `g25`, `h02`, `h04` | `value-in-carrier` |
| `g10` | `value-to-unresolved-callee` |
| `g12` | `value-as-parameter-default` |
| `g18` | `value-through-generator` |

R's debug build agreed before the merges. Remeasure on landing.

**The two witnesses the census does not close:**

| Witness | Minimal | Path | Fix | Ends |
|---|---|---|---|---|
| `f02` | `f1`, `f5`: async generator read after an await or a yield, `for await` in an async memo: UNTRACKED_READ_AFTER_AWAIT, certified. `f2`, the same as a plain async function, is V | `static_rules/after_await.rs:1505` `if helper.generator … return` | When `generator_iterator_used` is this call, an async generator takes the async helper's path below that line, with `GeneratorStepped` as its obligation. The literal arm at `:1202-1213` is the model | U. `f3`, `f4`, `f7` stay quiet and certified |
| `p03` | `p1`, `p2`, `p3`, `p6`, `p7`: `a()` on an async memo in a timer, `.then` or microtask: an uncaught throw with an empty message, **no diagnostic code**, certified | `DeferredCallback` is in neither `projection.rs:872-878` nor `reports_untracked_read` | In `push_async_read` (`local_access.rs:6024`): async provenance, `DeferredCallback` outside JSX, not declared: answer `Unknown` with `invocation_context_unproven` (field at `:6173`). **Do not add the role to selection:** the projector panics on it (`rules/src/lib.rs:603-612`) | U `invocation-context-unproven`, as handler `p5` is. `p4`, a timer after it settles, goes U; `p8`, a sync memo, stays certified |

`p03` is a false clean by your definition only because a real error fires with no code. The judge cannot see it, so its fuzzer shape needs a thrown-error row.

## 4. `outside` entries in `rediscovery.json`

50 entries. **30 still true, 13 stale, 7 hiding a real miss.** Numbers are array indices. "Text" means a `FLIP_READ_VARIANTS` `text` variant can write it: `misuse.mjs:663-669` applies to any code, so the "a context cannot respell it" reasons are out of date.

**Hiding a real miss:**

| # | Claim | The miss |
|---|---|---|
| 199 | `[1].map(h)`: a module-level function, which a context cannot write | Sync twin only. `g2d` family is certified. Text variant in `jsx` |
| 198 | hand-offs need a second file or a module helper | Same family, async |
| 217 | `sort.call`: not tried | **Tried:** `s1 s2 s3 s4 s5 s7 s12` U, closed. Neighbours are live: `s6` `.bind(st.a)()`, `s8` `Reflect.apply(Array.prototype.sort, st.a, [])` warn, certified. Fix: `push_function_call_target_obligation` (`local_access.rs:2161`) takes both. Shape: `store-array`, `Array.prototype.sort.call(${o})`, `.bind(${o})()`, `Reflect.apply(…)` |
| 218 | a parameter named like the store needs `model.mjs` | `t20`, `t21`, `t22`, `dl8` are live. Text: `const w = ({ a }: any) => { a.v = 7; }; w(${o});` |
| 219 | no context can root the write at a call's result or a loop element | All of section 2. Text writes them |
| 214 | a context cannot respell a loop head | Closed for `for (s.a of …)`, but the same reason hides `t7`. Text |
| 215 | a context cannot respell an aliased call | Closed for `Object.assign`; `t13` (`Object.assign({}, st)`, then a write through it) is live. Text |

**Stale:**

| # | Why |
|---|---|
| 174 | Known. Cells exist, and `FLIP_READ_LEGAL` has `show-false-later`, `ternary-later`, `and-later`, whose comment names ADR 0661. List the ids |
| 175, 176 | Known. `dyn-jsx-body`, `-mixed`, `-body-l`, `-later` exist. List the ids |
| 183 | "No sibling `Loading`": `loading-on-outside` is exactly that, and it is item 5's 558 programs |
| 192 | "Not reproduced outside those two projects": section 1 |
| 178 | "One file": programs have `helpers.ts`, `lib.ts`. `import()` is still not written, so the reason needs rewording, not the entry |
| 71 | Flush in a parameter default: text |
| 181 | `lazy` tag: a `form.mount` with a prelude writes it |
| 185 | Parameter called after an await: text, as `HANDED_READS` does |
| 202 | Tuple slot: text can declare its own store |
| 193, 194, 197 | "A context cannot write a module-level helper": `module-helper` and `s-module` exist for the helper; the hand-off forms still need a text variant inside the body. No miss found: patterns hold them |

**Still true:** 1, 60, 110, 145, 156, 157, 158, 159, 162, 163, 164, 167, 171, 172, 173, 177, 182, 187, 188, 190, 191, 195, 200, 201, 207, 209, 210, 211, 220, 221. Closable cheaply, not wrong: 157 with a `thrownCodes` pattern for `node._effectFn is not a function`; 173 with a second boundary context.

How deep this went: I read each claim against the generator and ran probes for the 7 misses. I did not fail any old build for the stale ones.

## 5. The 136 `LOADING_ON_OUTSIDE_HOLD` wrong-rule rows

**The line is not lost by Solid. The judge drops it, and it is not the read's line anyway.**

Measured on 136 of 136 in Chrome: ASYNC_OUTSIDE_LOADING_BOUNDARY carries `event.loc`, `App.tsx:L:C`, the `{` of the JSX hole of the first pending render effect (`locationOf(subject)`, `_loc` from the compiler). On all 136 the site line is inside that hole. Example: `direct-none` has `loc` 11:25, site 12.

| Step | File | What happens |
|---|---|---|
| Runner keeps it | `chrome-host/runner.tsx:173`, `fast/runner.tsx:174` | `loc: d.loc` is in the record. `positions` are stack frames only; the stack is the flush's, so usually 0 App.tsx frames |
| Host keeps it | `chrome-host.mjs:343` | Spreads `...d`, maps `positions` only |
| **Lost here** | `classify.mjs:50` `runtimeEvents` | Copies `positions`, drops `loc` |
| Even with it | `matrix.mjs:157` `corroborated` | Tests `region.includes(line)`. Region is the read's line, not the hole's |

Smallest change, 2 files, no Rust, no change to `positions`:

1. `classify.mjs:50`: carry `loc` on the event.
2. `matrix.mjs:157`: for an `isPlacelessCode(other)` event, also accept when `loc`'s line is the `line` of some ancestor of `site.path` in `built.lineTable` (`print.mjs:695`) whose `endLine` reaches the site. **Measured on all 136:** an ancestor matches on every row (the view IIFE's arrow, path depth 6). `judge` has `built`; `judgeSite` reads it from the same input.

Parse `loc` with `/App\.tsx:(\d+):/`: my harness gave `cases/<id>/App.tsx:12:25`, a grid slot's prefix differs.

Do not add `loc` to `positions`: `falseclean.mjs:231` and `hits.mjs:157` read them, and `atSite` would flip rows to placed. Do not touch `silenceMeasured`. Neither file is in `RECORD_SOURCES` (`cache.mjs:199`), so runtime records stay warm; check once that a cached record still holds `loc`.

**Ends:** 136 wrong-rule go to recall-gap, as the cell's other 422. Inferred. The SC5003 claim stands, corroborated by the runtime's own record.
**Scope:** ASYNC_OUTSIDE fires once per mount at the first pending effect. In a one-site program that is the site; with several it names only the first.

## 6. Build order and split

**First, on any branch:** merge `origin/claude/flip-writes` (`138ff86e7`, `91311857e`), per R's handoff.

| Order | Piece | Work | Files, functions | Depends on |
|---|---|---|---|---|
| 1 | **P1 judge** | Item 5 | `scripts/fuzz/classify.mjs` `runtimeEvents`, `matrix.mjs` `corroborated` | Nothing. No Rust. Not while a grid runs: grids read `scripts/fuzz/*.mjs` live |
| 2 | **P2 handed net** | N1, N2, N3, `s6`, `s8`, then the census report's C6, C1, C2 + `g2d`, C5, C7, C3 | `local_access.rs`: `handed_member_callee`, `handed_call`, `push_function_call_target_obligation`, `push_owned_dispatch_obligation_for`, `handed_role`, `handed_async_read_role`. `interproc/result_reads.rs` `:303`, `:548`. `reactive_analysis.rs` post-pass after the `open_members` block | P1 not needed |
| 3 | **P3 census** | R4, R8 | New `value_use.rs`, `lib.rs` `mod`, `reactive_analysis.rs` after `open_members`, `scripts/flip/check.mjs` | N1-N3 merged, or its `Followed` rows are false |
| 4 | **P4 bounds** | R6 | `w0` `p-bounds.diff`, against ADRs 0750, 0751 | After P3 |
| any | **P5 SC2003** | Section 2 | `upstream_compat/shared_reactivity.rs`: `value`, `call_value`, `for_of_operand`, `returned_parameter`, `plain_parameter_at`, `push_unproven_container_write`. Fixture, `cases.json` entry per invariant 5 | Nothing |
| any | **P6 small rules** | `f02`, `p03`, member tag, `primitive-value` | `static_rules/after_await.rs`, `projection.rs`, `source_discovery/post_rows.rs`, `trace.rs`, `indexes/compound.rs`, `static_rules.rs` | Nothing |
| last | **P7 fuzzer** | Text variants, `outside` rewrites | `flip-read.mjs`, `rediscovery.json` | Each entry lands with its fix; the rewrite of stale reasons can go alone |

**Collisions:**

| Where | Pieces | Handling |
|---|---|---|
| `local_access.rs` | P2 alone | Keep every edit in one stream: N1 and C1 are 120 lines apart, and C1b rewrites `handed_value` |
| `reactive_analysis.rs` | P2 C6, P3 | Two hunks after the same block: land C6 first |
| `result_reads.rs:548` | P2 N2, C1 | Same function |
| `blockers.rs`, `rediscovery.json`, `open-defects.json`, `cases.json` | All | Append-only: edit by text, union at merge. `json.dumps` reformats the file |
| `docs/open-work.md:223` | P5 | Rewrite the line |
| Snapshots | All Rust pieces | `make coverage` shows one row per fixture: compare row by row |

**Three independent agents fit:** one on P2 then P3 then P4 (the same stream, as P3 needs P2), one on P5, one on P6. P1 and P7 need no build, so they fit around the gates.
**The census report's order changes in one way:** it says C6, C1, C2 before the census, and that stands; N1-N3 go first, because they are smaller and they are what the census trusts.

**Before P2 and P3, write these as guards:** `n7`, `r8`, `r9`, `a10`, `a12`, `c15`, `c16`, `g2d-neg-*`, plus the section 2 list. Most stay certified; `n7` and `c15` go U, on purpose.

## Not checked

- No fix built, and no `after` measured. Corpus cost of N1, N2, N3, the SC2003 default arm and `p03` is unknown.
- The census draft: no saved build holds it, so the "census closes" column is by reading it, plus R's debug run from before the merges.
- The 32: not reproduced, not explained. Real parallel slot order was not replayed.
- Hydrate and SSR hosts. Every program ran CSR, development build.
- Old builds were run only for `c1`, `c13`. The rule's "the build that had it fails `make fuzz-focus`" is not done for anything here.
- `outside`: reasons read and misses probed. No generator change was tried.
- Line numbers are at `879eca007`; the census report's are at `8833e6bfa` and are stale (its `:3122` is now `:3441`).

Working files in `/tmp/sc-scratch.noindex/flip-research/sc9012`: `w w2 a3 h9 mx vx n r2 e` (cases), `*.rk2` (checker), `*-chrome.json` (Chrome), `sc9012rows.json`, `lo136*`, `warm2.mjs`, `join.cjs`, `h/run.mjs` (records `loc`).
