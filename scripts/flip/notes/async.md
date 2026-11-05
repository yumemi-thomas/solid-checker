# Async group design note: SC5003 + SC5001, SC5006, SC5008

Read at `076ad9a8b`. That commit is on the local `claude/flip-reads` only; `origin` is still `aaaf31cad`. Line numbers are from `076ad9a8b`. No Rust was built:
designs are argued from the code, and "measured" means `tsrs`, real Chrome and the `r2007` checker (the build of this commit) on one program.

Probes are in `/tmp/sc-scratch.noindex/flip-reads/research/probes/`: `m1` (76 programs, mount × read × boundary), `m2` (72, 66 pass `tsc`), `p10`, `p11`, `p12`. Chrome results
are in `<name>-chrome.json` beside `probes/`, checker results in `m1-chk.txt`, `m2-chk.txt`, `p12-chk.txt`. `node m1-join.cjs <name>` prints them side by side. `async-gen.mjs` and
`m2-gen.mjs` regenerate the matrices. Rows of the corpus: `async-rows.json`.

## Read this first

1. **The plan's SC5003 fix is wrong and would keep false positives.** It says V only "on the creating pass's stack". The runtime does not ask that. `ASYNC_OUTSIDE_LOADING_BOUNDARY` fires only inside a
   synchronous `render()` call: `enforceLoadingBoundary(true)` at `web.dev.js:536`, off at `:557`, read at `dev-shared.js:2691`. A component that creates its own memo and mounts later is on the creating
   pass's stack, the source is really pending, and Chrome is quiet (`t10`, `t14`, `t17`). The gate is "the read's first run is inside a `render()`".
2. **`e02` is not "settled".** A module-scope memo with no reader is dormant (`CONFIG_AUTO_DISPOSE`, `dev-shared.js:3355`, `:5892`), and a later read wakes it and recomputes it: `t1` logs `compute` again at the
   click, and a body read there **throws** `PENDING`. The checker's text "a settled read neither throws nor needs a boundary" is false for this case, as is `matrix.mjs:294`'s comment. A memo created in a
   body stays settled (`t3`: STRICT). So the U in ADRs 0518 and 0570 is safe, but the reason is wrong.
3. **Fixing `<Dynamic>` as "no-window, silent" adds a false clean.** Chrome still warns `ASYNC_OUTSIDE` with no boundary (`dyn-body`, `t20`, `s03`, `g03`). That body needs an SC5003 row. And a mixed mount goes **silent**,
   not U, unless the row carries `invocation_context_unproven` (`async_read_reported`, `projection.rs:~843`).
4. **`return_open` needs no new field for the logic.** ADR 0631's `opaque` already makes every async row U. Do not widen `computation_is_async`: `static_api.rs` reads it.
5. **Nothing users see is at risk in the corpus today:** these four rules have **0 violations** there, only 213 uncertifiable rows. The warnings at risk are fixture-level: SC5001 109, SC5003 14, SC5008 4.
   The SC5003 gate is the one edit here that can remove a true warning: step 2 restores them.
6. **`patterns.mjs:130` comes first now.** `sc1001.md` step 0 lands in this group, because `<Dynamic>`'s hint lands here.

## What the runtime does

| Fact | Where | Measured |
|---|---|---|
| Boundary warning fires only during `render()`, once per render | `web.dev.js:536-557`, `dev-shared.js:2261`, `:2691` | `t10 t12 t14 t16 t17` quiet; `t13 t15` warn; `t18`, a second `render()` in a handler, warns |
| Pending throw fires in a strict window at any time | `dev-shared.js:6145` | `t11`, a later mount of an own memo: throws |
| `<Dynamic>` body has no window: the memo clears `strictRead` and `strictPending` | `dev-shared.js:4805-4806` | `dyn-body-L`, `t19`, `t21`: quiet, retries, `v 1` |
| The window reopens inside that body | | `t23` child body, `t24` apply, `t25` labelled `untrack`, `t26` `Show` callback: all throw |
| `Show`/`Match` child runs under a window only with `length > 0` | `solid.dev.js:1631`, `:1679` | all corpus rows have a plain parameter (`sc1001.md`) |
| An unowned memo with no reader goes dormant and recomputes on the next read | `dev-shared.js:3355` | `t1`, `t4` |
| `clientOnly`: first mount goes through a gate memo, later mount calls `Comp(rest)` | `web.dev.js:2478-2480` | `clientLater-body` throws |

## 1. Functions to change

| Function | Where | Plan says | Reading |
|---|---|---|---|
| `async_read_reported` | `projection.rs:823` | "projects uncertain" | Right place for the U arm. Add the `UntrackedUnwindowed` row here too |
| rule choice | `rust/dialects/solid-v2/rules/src/lib.rs:~563` | not named | `TrackedJsx if !read.under_loading` picks SC5003. `UntrackedUnwindowed` needs that arm, or the row has no rule |
| `read_in_creation_pass` | `owners/creation_pass.rs:40` | use `written_in_creating_pass` | **Wrong helper.** `written_in_creating_pass` (`local_access.rs:3443`) is SC2009's same-function test. This file is the one to extend. Keep it for "pending", add the root gate beside it |
| `synchronous_tag_position` | `owners/async_context.rs:310` | not named | Takes a tag. It must also answer for a read's own position, or `e02` passes: the read is lexically in `App`, so the climb never asks about the `Show` around it |
| `shown_child` | `async_context.rs:452` | not named | Only `when={true}`. Extend it, step 2 |
| `dynamic_component_entry` | `execution_role.rs:592` | "joins the census" | Knows `dynamic(() => K)` only. Add the `component` attribute of an exact `Dynamic` |
| `dynamic_read_role` | `execution_role.rs:448` | | `Some(false)` gives `Unknown`, which SC5001 drops. Keep the row, see section 5 |
| `read_loading_cover` | `async_context.rs:22` | not named | A `Portal` between a `<Dynamic>` body and the `Loading` does not cover (`portalDyn-body-L`) |
| `computation_is_async` | `async_context.rs:1217` | "misses any/unknown" | **Leave it.** `static_api.rs:131`, `:1053` depend on it. Add `computation_provenance` |
| `computation_may_answer_sync` | `async_context.rs:1247` | | Reuse its route through `first_answer_options` (`async_server.rs:55`) |
| source discovery | `discover_file.rs:58`, `:176`, `:284`, `returned_sources.rs:71` | "sources get `provenance_open`" | The four sites. `createSignal(fn)` and `createStore(fn)` go through them (`o12`, `o13`) |
| `discover_dynamic_jsx` | `local_access.rs:6589` | not named | `lazy` tags go here: `b04 j14 j15 d32` have no row in the plan |
| `loading_hold.rs` | `:306`, `:317` | "`:207`, `:218`" | Stale numbers, right spots, not enough, see SC5006 |
| `discover_caught_reads` | `local_access.rs:3566` | "`:3609`" | Stale number, right spot, not enough, see SC5008 |
| `producer` | `source_facts.rs` | "producer sets `return_open`" | Done by ADR 0654, for every function in a file. `createMemo(comp)` is an identifier: resolve it to the function, or Open |
| `patterns.mjs:130` | | | `sc1001.md` step 0 |
| `matrix.mjs:275-280`, `:294`, `:329`, `mounts.mjs:15` | | not named | Why the fuzzer never saw any of this, see section 5 |

Blocker names: `SOURCE_PENDING_UNPROVEN` names the wrong fact. Rename it `root-mount-pass-unproven` while nothing uses it. Replace the message at `projection.rs:1291`, which claims what `t1` disproves.

## 2. Order of edits

Each is one commit, tree green after it. The plan's "one commit, one fact" no longer fits: there are three facts.

**Step 0, harness, no behaviour.**
- `patterns.mjs:130`, as `sc1001.md`.
- `matrix.mjs`: `bodyLabelled` skips `dynamic` for `PENDING_ASYNC_UNTRACKED_READ` on the grounds that it "re-runs the body without end". On rc.21 it retries and renders (`dyn-body-L`, `ran2+v`). Drop that code from `BODY_LABEL` for pending and make the cell legal.
- `mounts.mjs:15`: `<Dynamic>` is left out as deprecated. rc.21 types accept it. Add contexts in `flip-read.mjs`, not a mount: `dyn-jsx-body`, `dyn-jsx-later`, `dyn-jsx-mixed`, `portal-body`, `portal-jsx`, `portal-dyn`, `show-false-later`, `ternary-later`, `and-later`.
- Fix `matrix.mjs:294`'s comment: true for an owned memo only.

**Step 1, SC5003 gate.** New `root_pass_unproven` on `AsyncRead`, set beside `creation_pass_unproven` (`reactive_analysis.rs:503`). The read is proven only if the climb reaches a function `direct_render_sites` mounts, through unconditional positions.
`read_in_creation_pass` stays as the proof that the source is pending. **V needs both.** Project U for SC5003 only: SC5001 needs no render window (`t11`). Exempt `ssr_client_hole`, as at `:1285`. Before this, make `synchronous_tag_position` answer for a span.

Unconditional means: not in a `Show`, `Match` or `For` child unless proven, not in a `fallback`, not on either arm of a conditional or the right of `&&`, `||`, `??`. An intrinsic's attribute and `<Show when={…}>` itself are unconditional (`c04`).

**Step 2, keep the warnings.** Same commit if a fixture moves, else next. Extend what proves a first run:
- `shown_child`: `when` is the accessor of a `createSignal` with a literal truthy initial value, `const`, read in the first pass (`t15`, `t8`, the kept `show-signal` mount).
- `Match when={true}`, `<For each={[…]}>` for a read, not only a tag; `Errored`, `Reveal`, a fragment, `Portal`, a context provider as synchronous holders.
Measured to run in the first pass: `errored-body`, `sw-body`, `reveal-body`, `forRow-body`, `portal-body`, `frag-pend`. This also turns U into V for SC5001 on those. Read each.

**Step 3, `<Dynamic>`.** In `dynamic_component_entry`, a reference that is the whole `component` attribute of an exact `Dynamic` is `mounted`. A reference inside that attribute's expression (`on() ? K : J`) sets `dynamic` true and `exclusive` false. An alias (`const Z = K`) must reach the same answer: today it is V and quiet.
- Exclusive: `UntrackedUnwindowed`. SC1001 keeps V with the no-window hint. SC5001 is silent: proved.
- `async_read_reported` and the rule table: `UntrackedUnwindowed && !under_loading` is SC5003. U is enough: `dyn-jsx` is U today because no chain follows `<Dynamic>`.
- Mixed or unknown: `Unknown` with `invocation_context_unproven`, so SC5001 stays U (`dynMixed-body` throws).

**Step 4, `clientOnly`.** No census entry and no unwindowed role. Both claims U, blocker `mount-form-window`. `d05`, `d19`, `b03`, `j16` are V through an imported module; the same-file forms are U already (`client1-*`).

**Step 5, Portal under Dynamic.** `read_loading_cover` answers `Unresolved` for a body under an unwindowed mount with a `Portal` between it and the `Loading`. Cause not traced; `portalDyn-body-L` warns with a `Loading` above, and `portalDyn-jsx-L` does not.

**Step 6, open provenance.** New `computation_provenance(lookup, file, argument) -> Async | Sync | Open`:
- `Async`: `computation_is_async` and not `may_answer_sync`.
- `Sync`: an exact fact with `!return_open && !can_return_async`, the same test as `static_api.rs:1062-1070`. An identifier resolves to its function first.
- `Open`: anything else.
At the four discovery sites, `Open` pushes to `async_sources` and sets `opaque` plus a wording flag `provenance_open`. Existing arms then give SC5001, SC5003, SC5005 U, and SC1001 U as on a real async memo. Blocker `async-provenance-unproven`. Add the compute that may `throw new NotReadyError` here: C-D4 is the same case (`n01 n03 n06`).

**Step 7, `lazy`.** `discover_dynamic_jsx`, for a `lazy` binding: an async row at the tag, U. Not in the plan.

**Step 8, `j06`.** `read_in_named_local_function` misses an object method, so `push_call_attributed_async_reads` is never asked. Extend it with `unplaced_run`. `check.mjs` needs an `SC5003` finding here, not only a read row. `h01` (a getter) has no call, so it needs step 2 of SC5006's getter helper below.

**SC5006, own commit.** The plan's "one edit" closes `b03` and `h03` only.
1. Drop `call.arguments.is_empty()` (`:306`).
2. At `:317`, a resolved target counts only if it is complete. **Note the argument:** `unplaced_run` asks about a site, so pass the target's body, not the call: `function_called_at` gives the file and function.
3. Any reference to an async source under the boundary counts as a read, not only a row at it: `rd(a)` (`h05`) has no row, and F0's predicate calls `rd` complete, since an own parameter is followed.
4. A member Get that may run a getter is open (`h01`). Share the helper with SC5008.

**SC5008, own commit.** At `:3566`:
1. A callee `handed_value` answers true stays a site, source `None`: `l05`.
2. Enumerate member Gets in the block: a prop, a handed root, a literal with a getter. `l07`, plus my `l07-getter-in-try`.
Uncertain only, blocker `caught-read-unproven`.

## 3. What each edit closes

| Step | Closes | Must not change |
|---|---|---|
| 0 | Leaves the list: 12 no-window rows (`sc1001.md`) | none |
| 1 | Witness `e02`. New: `t10 t12 t14 t16 t17`, `c01 c02 c03 c05` V → U | `j01-ctl-5003`, `t13`, `c04 c06 c07`, all SC5005 |
| 2 | none; keeps `t15`, `t8` V | `c03`, `c05` stay U |
| 3 | `d17 e421 g04` silent; `s03 g03` SC5003; `p-dyn` witness. New: `t19 t21` V → silent or U, `t20`, `dynAlias-*` | `p-plain`, `r-dyn` keeps `SC1001/v@8`; `t23 t24 t25 t26` stay as they are; `rc21-untrack-hook-pending`'s `dynamic(() => Remounted)` and `async-boundary`'s `dynamic(async …)` |
| 4 | `d19`; `b03 j16 d05` hint or U | `client1-*`, `clientLater-body` (throws) must stay U |
| 5 | New: `portalDyn-body-L` stays open after step 3 | `portal-jsx-L` certified |
| 6 | Witnesses `j02 j03`. New: `o05 o07 o08 o12 o13`, `o01 o04` V → U. `n01 n03 n06` | `o02`: **loses V, becomes U, see below**. `o09 o10 o03 o06` go U, quiet: accepted |
| 7 | `b04 j14 j15 d32` | an `<L />` under `Loading`, `lazy1-jsx-L` |
| 8 | Witness `j06` | `sc1001.md`'s `c01`-`c06` |
| SC5006 | Witness `b03`. New: `h01 h03 h05` | `k01 k03 b01 b02` regress stay open; `h07` certified |
| SC5008 | Witnesses `l05 l07`. New: `l07-getter-in-try` | `l01 l02` regress; `l12 l13` go U, quiet; `l03` certified |

Not closed here: `v49`, `e28`, `e29`, `f02` are the census or SC1002. `c16-spread-unread-key*` are SC1003's: the rest destructure runs a pending getter and throws, so that V is the wrong rule. `cases3-p03` (a pending read in a timer: an uncaught `NotReadyError`, no code) has a home in neither the plan nor the code. A read of an async source on a fresh stack is the honest U; decide it in step 1's commit. `f01` is rejected by `tsc`.

## 4. The warning direction

**Corpus: there is nothing to lose.** 0 violations of SC5001, SC5003, SC5006, SC5008 in 383. I read every uncertifiable row of the first three and all 196 of SC5008, by group:

| Rule | Rows | Why U | After this note |
|---|---:|---|---|
| SC5001 | 8 | 5 top-level await, 2 unreadable options (`Starred.tsx:426-427`), 1 nested callback | U. Text changes |
| SC5003 | 7 | 4 top-level await, 2 creation pass (`auth()` in a route), 1 no root | U |
| SC5006 | 2 | a component under the boundary | U |
| SC5008 | 196 | 180 of 195 unique rows in `async` functions, mostly handlers: `navigate`, `setError`, `toast.success` | U. See below |

Step 2's proofs could only add V where every other flag clears. All 15 rows hold another blocker. Expect 0; read the sweep.

The 180: "this computation catches the throw of `navigate`" is false wording, on a function no computation runs. A proof that the function is reached from no compute would drop them. Not in the plan, and not needed for the flip; I would tell the owner.

**SC1001 overlap: the one place step 6 reaches user warnings.** An open source's body read goes U. Of the 48 accessor sources behind the 105 corpus sites, 18 are signals, 8 are `createMemo`, 3 are router hooks and 19 are flow parameters. All 8 memos have a closed return: `<Hit[]>`, `new Map`, `new Set`, `<Journey[]>`, `.some()`, and declared returns for `routeAt`, `routeStops`, `boardStops`. **0 corpus V lost.** Read by hand, not type-checked.

Steps of each violation:

| Rule | Step | Today | After |
|---|---|---|---|
| SC5003 | P: the read runs | proved, except conditional positions (`c01`-`c05`) | **Becomes U** unless step 2 proves it. Local proof exists for literal and first-value conditions |
| | M: root mount pass | **assumed** | Proved by the chain to `direct_render_sites`, else U |
| | W: source pending then | proved for own and module memos in the first pass | Unchanged. Top-level await stays U (`settle_unproven`). The dormant fact could prove module memos; not needed |
| | Boundary absent | proved | Unchanged |
| | E: warns | proved given the above | Provenance: `Async` only. `Open` is U |
| SC5001 | P | proved | Unchanged |
| | M: window | **assumed**: every non-`dynamic()` mount | **Proved locally** for exclusive `<Dynamic>`: silent. Tag mounts: proved. `clientOnly`, mixed, unknown: U |
| | W: strict window | proved by role | `t23`-`t26` hold: the role is not `UntrackedRendering` there, or a child's own body. A Show callback function has no binding, so `dynamic_component_entry` returns `None` |
| | Pending | proved in the creation pass | Unchanged. A later mount is U, and `t11`, an own memo, stays V |
| | E: throws | proved | `Open` is U |
| SC5006 | all | U only | No V exists |
| SC5008 | P | proved: `computation_runs_where_written` | Unchanged |
| | Pending | proved: `created_by_async_literal_around` | Unchanged |
| | E: pass completes | proved: no `throw` in handler, others kept | Unchanged. `l04` U |

**Fixtures.** I read the 14 SC5003 V rows. 13 are unconditional in a root's chain: `<p>{user()}</p>`, `[1].map(() => user())`, a render effect's compute. `<Show when={memo()}>` (`rc21-prop-boundary:50`) is the attribute, measured true by `c04`. Four fixtures rely on `main.tsx` or a variant, and a `render` of anything but a bare `<X />` fails `direct_render_sites`: `handed-residuals`, `effect-apply-first-run`, `strict-read-async-apply`, `pending-source-first-run` are the ones to watch in `make coverage`.
SC5001's 109: only `async-boundary` and `rc21-untrack-hook-pending` mention `dynamic`, and those are the `dynamic()` route step 3 does not change. Read them anyway.

`o02` is the real trade: `createMemo(() => load())` with `load: any`, sync. Chrome says STRICT, and it becomes SC1001 U. `o01` is the same text and throws `PENDING`, so V is wrong there, by the ruling on C-D4. No fact separates them. Local proof that recovers the common case: `JSON.parse`'s result is plain data (the report's section 3A), and `as any` over a closed expression. Do it if the sweep shows V moving.

## 5. How it could go wrong

| Edit | Failure | Program | Guard first |
|---|---|---|---|
| Step 1 | Following the plan keeps V on a later mount | `t10 t14 t17` | Those three, before the edit |
| Step 1 | Gate over-applied to SC5001 loses true throws | `t11`, `showLater-body` | SC5003 only |
| Step 1 | A `render` wrapped in a provider fails `direct_render_sites`, and every V in that app goes U | `render(() => <Ctx value={1}><App /></Ctx>)` | Extend it to synchronous holders, or accept; no corpus V today |
| Step 1 | A nested `render()` in a handler is true and goes U | `t18` | Accept: U is honest |
| Step 2 | `when={on()}` proved true while written before mount | `setOn(false)` at module scope, then `render` | "Never written outside a function", or U |
| Step 3 | Silent on a mixed mount | `dynMixed-body`, `dynMixed-body-L` throw | Assert U |
| Step 3 | Silent with no boundary | `dyn-body`, `t20` | Assert an SC5003 row |
| Step 3 | Silent under Portal | `portalDyn-body-L` | Step 5, same commit if possible |
| Step 3 | `dynamic_read_role` reaches a read in a window | `t25 t26` | Assert V |
| Step 3 | An exported `K` is treated as exclusive | `x01` | `flow_binding_exported` stays |
| Step 4 | `clientOnly` takes the unwindowed role, a false hint | `d06`, `clientLater-body` | No census row |
| Step 6 | Widening `computation_is_async` flips SC2007 and SC7002 | `static_api.rs:131`, `:1053` | New function |
| Step 6 | `Sync` claimed with no fact | `o07`, `o08` must not be Sync | Missing fact is `Open` |
| Step 6 | `async_sources` feeds other readers: the transport filter at `reactive_analysis.rs:224`, SC2009's `optimistic_async_setter`, `caught_read_silent` | any `createOptimistic` with an `any` compute | Read each `async_sources` reader; SC9012 will rise |
| Step 6 | The message says "options argument" | `o01` | The wording flag |
| Step 7 | V on a preloaded `lazy` | `L.preload()` before render | U only |
| SC5006 | Passing the call site to `unplaced_run` | `b03` stays silent | Target's body |
| SC5008 | Every `props.x` in an async handler's `try` adds U | | Same role gate as the calls |

**Why the fuzzer missed all this.** `BODY_LABEL` removes the `dynamic` mount for exactly the two codes that matter. `show-later` × async is filtered as settled in pairs. `<Dynamic>` is not a mount at all. Each defect still needs its rediscovery row, and step 0 is what makes that possible.

## 6. `<Portal>`

Keep `portal` in `LEAN_DEMOTED`: removing it costs 48,612 programs. The plan's "a program in a kept grid" is right: `flip-read.mjs` contexts under `PENDING_ASYNC_UNTRACKED_READ`, `ASYNC_OUTSIDE_LOADING_BOUNDARY`, `STRICT_READ_UNTRACKED`.

| Program | Chrome | Checker today | After |
|---|---|---|---|
| `portal-body`, `-L`, `portalMount-pend` | throws | SC5001 U | V with step 2 |
| `portal-jsx` | warns | SC5003 U | V with step 2 |
| `portal-jsx-L` | quiet | certified | same |
| `portalLater-body` | throws, dormant memo | U | same |
| `portalLater-jsx` | quiet | U | same |
| `portalDyn-body` | warns, V as SC5001 | wrong rule | SC5003 |
| `portalDyn-body-L` | **warns, with a `Loading` above** | V, wrong rule | Step 5, U |

`<Portal>{K}</Portal>` halts, and `tsc` rejects it. W's owner claims under `Portal` run after the merge, as the plan says. Not run here: `mount={…}` changing later, `useShadow`.

## 7. How this fits with the other notes, and Track W

- **SC7002** looks in progress (`sweep-7002.json` exists) and reads `return_open` too. `compute_provenance` should be its helper, or the same test exists twice.
- **SC1001:** `sc1001.md` step 8 is this note's step 3. Do not build it again. Its step 9, C-D4, is this note's step 6.
- **F0:** SC5006 waits on `function_summary_complete` only for step 2 there. Until F0, use `unplaced_run` on the target's body.
- **Census:** `e28 e29` are its rows. A pending source in a `Map` is a held value.
- **`creation_pass.rs`, `async_context.rs`, `source_discovery/`, `projection.rs`, `rules/src/lib.rs`, `web`-facing mount code** are R's. The plan's table lists only `projection.rs`.
- **W:** `server_rules.rs:191` `loading_wrapped_components` is W's (SC7005, SC4002's walk). It is a separate walker from `loading_cover_at`; do not merge them on R. SC2009 reads `async_sources`, so step 6 reaches W's rule: an `any`-computed `createOptimistic` starts to hold. Tell W.
- `AsyncRead` gains fields: `local_access.rs` has 5 constructors plus `reactive_analysis.rs:463`. R-owned hunks, but near W's.

## Not checked

SSR, hydration (`hydrate` was not read past its start). `clientOnly` with a real `import()`: `run.mjs` copies only `App.tsx`, so `d19`, `j16` come from the recorded lane. `L.preload()`. Why `plain-body-L` is `SC1001/u` ("call sites cannot be enumerated"): a tag under `Loading` should be enumerable, and this is the inversion Q6 names. After step 3 the legal side is silent, and the illegal side stays U, so the inversion ends, but that U could be V. The `Portal` cover cause. Corpus effect of step 6 on SC9012.
