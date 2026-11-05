# SC9012 and the value-use census: design note

Read at `origin/claude/flip-reads` `ef46d0ac6`. Nothing here was built. "Measured" means `tsrs`, real Chrome and the `r5007` checker on one program. Probes:
`/tmp/sc-scratch.noindex/flip-reads/research/probes/p5`, `p6`, `p8`, Chrome in `pN-chrome.json`.

## Read this first

1. **The plan's `check.mjs` cannot pass the 8 W witnesses.** A `clean` witness of rule `SC2003` needs a finding with id `SC2003` (`check.mjs`: `own.length > 0`), and the census emits SC9012. "They pass after the merge" is false until
   `check.mjs` accepts it. The same goes for `o12` under SC1003. One line, section 8.
2. **"Primitive argument" is not a followed position in general.** `createStore({ f: c })` then `untrack(() => st.f)()` (`h02`) and `createSignal({ s: set })` then `untrack(h).s(2)` in a memo (`h04`): certified, Chrome warns or throws.
3. **`.bind` is followed only when called on the spot.** `const b = c.bind(null); b()` (`g07`): certified, warns.
4. **"JSX attribute" is followed only as far as `props_reactivity` is honest**, and it is not: H-D5 (`x1 x2 x3 x5 w6`) is certified. No row of the plan owns H-D5.
5. **The census is the same shape as F0's walk.** Build one position classifier and use it twice.

## 1. Functions to change

| Place | Change |
|---|---|
| new `value_use.rs`, in `solid-reactive-ir/src` | `fn use_position(file, reference, tables) -> Position`, then the census loop. Rule logic: no dialect names in it; ask `Dialect` |
| `reactive_analysis.rs`, after the loop at `:327` | call it. `:327` is right; the transport tables are in scope |
| `blockers.rs` | one per default position, below |
| `local_access.rs:200-342` `handed_value` | F-B: walk operands in the `false` tails at `:258`, `:291`, `:314`, `:321`, and the `is_some_and` at the end |
| `indexes/callees.rs`, `resolve_call_target` in `result_reads.rs:~606` | F-C. The report's `callees.rs:908` is `indexes/callees.rs`; find it by `Object.values(o)[0]()`: a computed callee never takes the inner call's fact. Invariant 3 |
| `source_discovery/props_reactivity.rs:722-733` | H-D5, new in this note: `function.span.contains(span)` exempts any reference in the component's own function, and `element.span.contains(span)` any same-named one in its element |
| `shared_reactivity.rs` | **Do not touch:** ported, invariant 5 |
| `scripts/flip/check.mjs` | accept SC9012 for the 9 ids in section 8 |

Plan stale or vague:

- The audit's `vcensus.cjs` is name-based and per file. The real census walks `lookup.symbol_references(symbol)`, which crosses files.
- `splitProps`/`mergeProps` views are excluded (1,376 uses). rc.21 calls them `merge` and `omit` too: exclude by Dialect primitive, not by name.
- The table lists no row for several positions. These must be **explicit**:

| Position | Answer | Why |
|---|---|---|
| `export const`, `export { c }`, `export default c` | followed | Symbols cross files; a closed program has no other importer. Mirror `props_reactivity`'s `closed` test |
| Shorthand `{ c }` | as a holder | `g13` |
| Template `${c}` | followed: it has a coercion rule, SC1005 | |
| Tagged template argument | **default** | `g10` certified, warns |
| Spread call `h(...[c])` | **default** | `g09` certified, warns |
| `.bind` held | **default** | `g07` |
| TS `as`, `!`, `satisfies` | peel | `peel_ts_sugar_span` |
| `void c`, `delete` | followed, no value leaves | |
| `Reflect.apply(a[0], …)` | **default** | `g08` |

## 2. The classifier

No parent map exists, and none is needed. Test each followed position with the existing queries, in order, and fall through:

```rust
fn use_position(..) -> Position {
    let at = peel(reference);
    if callee_or_dispatch(at)    { return Followed; }   // calls_with_peeled_callee, dispatch_target, immediate .call/.apply/.bind
    if member_object(at)         { return member_get_or_set(..); }
    if jsx_value(at)             { return jsx_position(..); }
    if test_position(at)         { return Followed; }   // typeof, ===, !, if
    if const_alias(at)           { return recurse(alias); }
    if holder(at)                { return holder_uses(..); }
    if returned(at)              { return returned_uses(..); }
    if argument(at)              { return argument_position(..); }
    Open(blocker_for(at))
}
```

`classify_parameter_use` (`interproc/parameter_use.rs:266`) is the model for each query, and its tail is the warning: its `None` default is what makes it not a flip.

**Followed, corrected:**

| Position | Condition |
|---|---|
| Callee, comma, `.call`, `.apply`, `.bind` | called on the spot only |
| Member Get or Set on a store or props | existing rows; see links below |
| JSX attribute, child | exact tag, not `<Dynamic>`, and the prop passes `props_reactivity` after H-D5 |
| JSX spread | as a holder |
| Primitive **callback** position or a read-only primitive position | Ask `Dialect`. A position that **retains** the value (`createSignal`, `createStore`, `createContext`'s default, a provider `value`, `createOptimistic`) is a holder: followed only if the literal holds no source (`h02`, `h04`) |
| `const` alias | recurse over its references |
| Return, holder | `ClosedUses` intact, no `this` in the literal |
| `typeof`, `===`, `!`, a test | |
| Reviewed fresh-stack | `runtime_semantics::argument_behavior == FreshStackCallback` only. Not `.then`, not `addEventListener` by name: `RetainedValue` and invoker-stack hosts have their own rows |
| Void or primitive std parameter | `ValueOnly` and primitive return. `String(c)` is SC1005's, `JSON.stringify(store)` is V-D4's |
| Project parameter | **recursion**: run this classifier over the parameter's references, memoised, bounded, past the bound open. Not F0's predicate |

**A reference inside a nested function.** The position is decided as above, then the nested function must be placed: for a callee, `unplaced_run` is SC1001's and followed here. For a parameter captured by a closure that leaves its function, F0 owns it at the call, so skip it to avoid a double count, as the report says.

**Default blockers:** `value-held-in-binding`, `value-held-in-member`, `value-through-logical`, `value-in-class-field`, `value-through-generator`, `value-through-await`, `value-through-new`, `value-as-parameter-default`, `value-in-carrier`, `value-to-unresolved-callee`, `value-from-primitive-callback`, and one catch-all `value-use-unclassified`. The catch-all is the flip: add a test that it fires on an unknown form.

Stores and props walk the same way. A store root at a project parameter is followed only if the parameter's Gets and Sets are: `x.a = 2` on the parameter is **open** (`c04`, `f24`). SC2003 does not see it, and calling it followed would keep the false clean.

Links are not this census: `const t = s.a; t.x = 1`, `<For each={st.items}>` rows. A root census would flag every `For`. V-D2's 22 entries stay W's forward flow (`SC2003`, queue item 10).

## 3. Order of edits

Inside SC9012, each commit green:

1. F0 (`f0.md`).
2. F-C, small.
3. F-D: `children`, `mapArray`, `repeat` results are accessors; a primitive's result called directly is a read. Note: `h13` (a hook returning `mapArray`) is quiet and certified; keep it.
4. **H-D5**: `props_reactivity.rs`, narrow the two exemptions. Do this before the census, because the JSX row rests on it.
5. `check.mjs` accepts SC9012 for the 9 ids.
6. Census, **callables only**: accessors, setters, actions. Closes most.
7. Census, **stores and props roots**. A separate commit: this is where cost and H-D5's trust sit.
8. F-B, `handed_value` total. Last, since it reads what the census decided and is role-gated.

## 4. What each edit closes

| Step | Witnesses (`SC9012/...`) | Open entries beyond those |
|---|---|---|
| F0 | `cases2-v42` | `f0.md` |
| F-C | `cases3-r10` | `C-inner-call-identity` |
| F-D | `cases3-w07 w08`, `cases3b-f06`, `cases4-k10 k11` | T-D2: `b01 b02 b03 b04 f14 h01 i02 i03 x10 x11 x12 x13` |
| H-D5 | none | `w6 x1 x2 x3 x5` |
| Census callables | `cases1-h22 h26`, `cases2-a01 a02 a03 a05 a08 a12 a18 a19 a20 a21 a22 a24 a25 a30 a31 a32 w01 w02 w03`, `cases3-e01 e03-e07 e10 e20 e23-e29 e31 f10 k12 k13 k14 r01 r03 r05 r06 r08 r09`, `cases2-v40 v41 v43 v49` | H-D2, H-capture, V-D3 (`d3-*`, `t3-*`), `T-other`, each read: `b13 b15 f18 f19 f26 f30 h04 h06 h08 h10 h12 h14` (`f05` is SC1001 step 2, `h02` is W's), V-D7 part. New: `g07 g08 g09 g10 g12 g15 g18 g22` |
| Census stores, props | `cases2-v01-v06 v08-v11 v20-v24`, W's 6 | V-D5 root half (`d5-helper-props d5-json d5-object-assign d5-object-values`), `f24`, `h02`, `h04`, `g25` |
| F-B | `cases2-v31-v38` | `f23` |
| Stays failing here | `cases3-f02` is a role (F-E). `cases3-p03` is **no value flow**: `go` reads a pending memo from a timer and the throw is uncaught. It belongs to SC5003+SC5001, check it there | V-D2, V-D4, V-D6, V-D8; merge and omit destructures (ADR 0630) |

`a06 a29 e08 e09 e12 e32`, `d6-comma`, `h11` are `rejected` by `tsc`: reword or remove, do not flip for them.

Must not change: `t01`-`t06` stay certified; `g17` is quiet; `FocusScope.tsx`-style `let` identity keys go U, which the plan accepts; every `keep`; `h13` certified.

## 5. The warning direction

The census only adds SC9012 U. It takes no violation away and asserts none, so **no corpus finding is at risk**. No project changes status: all 34 are already violation or uncertifiable.
SC9012 is internal: no user sees it. The hazard is a false clean in a **followed** row, not a false positive. H-D5 is different: narrowing it moves that rule to U, or to a proved row.

## 6. How it could go wrong

| Failure | Program | Guard |
|---|---|---|
| `.bind` held treated as a callee | `g07` | `dispatch_target`'s shape only |
| Retaining primitive treated as read-only | `h02 h04` | Dialect question, tested per primitive |
| JSX row trusts D5 | `x1`-`x5` | Step 4 first |
| `.then` treated as fresh-stack, sync thenable | `g23` is V today; `then` on a custom thenable must not be followed | `argument_behavior`, not a name list |
| A parameter census that loops | recursive helpers | memo with in-progress = open |
| Default arm reached only by accident: a new form silently matches `Followed` | | Order the tests strictly; a table test per row, plus one test that an unknown form hits the catch-all |
| Cross-file: the source is imported | | `symbol_references`; add a two-file fixture |
| Premise: `eval`, `new Function`, string-keyed globals | `d3-new-function`, `a19` | Record it (plan). V-D7 wanted narrow rules: decide with the owner, the plan and `queue.md` differ |
| `make coverage` moves many SC9012 | | Commits 6 and 7 apart; read the moved findings |

**F0's cost overlap:** `v42` fires at `useT(c)` and `x.read()`. Skip a parameter F0 owns, or the count doubles.

## 7. What owns what, and the order

| Piece | Owns | Does not own |
|---|---|---|
| SC1001 steps 2, 3 | a **call of a proven accessor** placed nowhere. This makes "callee" a followed position | values |
| F0 | **handed values** at a call of a resolved target | proven sources |
| Census | **every reference of a proven source**, including setters, actions, stores, props | handed values, links |
| F-B | handed values held in operands, `let`, `new` | |
| F-C, F-D | spellings no value row reaches | |
| V-D2 flow (W) | links from a store, rows, iteration | roots |

**Built once:** the position queries. F0's walker asks "is this captured value reachable", the census's asks "is this reference in a followed position". Both call the same helpers (`peel`, `calls_with_peeled_callee`, `object_properties`, `bindings`). Put the helpers in `value_use.rs` and let `completeness.rs` import them.

**They do not share a predicate**, whatever the plan says: `keep(f) { window.__k = f; }` is complete for F0, because running it reaches nothing unknown, and open for the census, because its parameter leaves. `function h(x) { unknown(); return x.id; }` is the reverse. Two questions, one set of helpers.

## 8. Track W

**The 8 witnesses:**

| Id | Program | Closed by | Note |
|---|---|---|---|
| `SC2003/c04-param` | `w(s)`, `x.a = 2` on the parameter | census stores, parameter Set open | |
| `c15-map` | `new Map([["s", s]])` | census, default | |
| `c16-class` | `class H { t = s; }` | census, default | |
| `j05-copy-ref` | `w(s.o)` | **Not by a root census.** `s.o` is a member Get, followed | Plan is wrong. Needs a rule: a member Get that is a whole call argument is a hand-off. `SC1001/u@7` today does not cover the write code |
| `j07-weakmap` | `new WeakMap([[k, s]])` | census, default | |
| `j08-closure` | `const get = () => s; get().a = 2` | **Both tracks claim it.** W's SC2003 item (3) names `get().a = 2, j08`; the plan also lists it for the census | Whichever lands first; say so in W's list |
| `SC2009/a02-opt-class` | `new H(setO)` | census, `new` default | |
| `SC2009/a03-opt-map` | `new Map([["s", setO]])` | census, default | |

`o12` under SC1003 (`use(props)`, destructured in a helper): census props row, the plan's SC1003 line says so. Same shape as V-D5's `t5-helper-destructure-param`.

**`check.mjs`.** The `clean` arm reads `w.rule === "SC9012" ? out.status !== "certified" : own.length > 0`. Add a list of the 9 ids, or a `closedBy` field in `witnesses.json`, answered by `out.status !== "certified"`. It belongs in the census commit, on R; W sees those 9 as expected failures either way.

**Files the plan's table omits:** `reactive_analysis.rs` (both tracks), `props_reactivity.rs` (R, new), `result_reads.rs` (R), `value_use.rs` (R, new). `blockers.rs` is append-only: W also adds lines there; union at merge.

**Ask W:** does SC2003's forward flow reach `w(s.o)`? If not, `j05` has no owner.
