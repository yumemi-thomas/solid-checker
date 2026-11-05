# The flip: every rule, both directions

Owner's rule, both ways. **Clean only with proof, warn only with proof.** What is not proven is a clear uncertifiable with a
named blocker: never a silent clean, never an assumed violation. Prefer proving where the fact is local. Counts are not a reason
to narrow or skip a rule. Delete this directory when both tracks are merged.

Line numbers are from `5b8a1d7b5`. The base is later: find each spot by the quoted code.

Files here: `PLAN.md`, `witnesses.json` (311 programs: source, Chrome result, checker result before), `check.mjs`, `run.mjs`.
`docs/` is an allow-list under `scripts/doc-budget.mjs`, so this lives in `scripts/flip/`.

```
node scripts/flip/check.mjs SC5007        # judges one rule's witnesses against a fresh release checker
node scripts/flip/check.mjs all           # at 5b8a1d7b5: 213 fail by design (201 clean, 5 warn, 7 pref); all keep/regress hold
node scripts/flip/run.mjs <casesDir> out.json   # real Chrome: <casesDir>/<id>/App.tsx, optional `export async function after()`
```

Kinds in `witnesses.json`: `clean` (was certified, runtime complained), `warn` (violation the runtime contradicts), `keep` (true
finding must stay), `regress` (open today, must stay open), `pref` (preference site with no runtime truth).

## How every rule is built

One rule, both directions, one local commit. The tree is green after each commit, so work can stop at any rule.

1. Do both columns of the rule's entry. Prefer a proof over a withheld warning where the fact is local.
2. `node scripts/flip/check.mjs <code>` passes, `make test-focused`.
3. Fuzzer: add the rule's shape to the track's generator file, old build fails `make fuzz-focus CODES=<runtime code>`, new passes,
   `scripts/fuzz/rediscovery.json` entry with ids, or the reason the generator cannot express it (AGENTS.md).
4. `node scripts/corpus-sweep.mjs --manifest scripts/corpus/rc21-manifest.json --apps-root <frozen apps> --order-from <last>`, then
   `node scripts/corpus-sweep-compare.mjs <before> <after> --list 200`. **Read that rule's moved findings** before going on. A moved
   finding you cannot explain stops the track.
5. `make coverage`, read, `make coverage-update`, `make ownership-gate`; `make verify-delta`. Rule page, `docs/open-work.md`,
   a short ADR. Report in the commit message. Local commit, no push unless told.

`run.mjs` was moved to repo-relative paths and has not been run since; `check.mjs` has. The Chrome results in `witnesses.json` are from the original harness.

Stop conditions: a `keep` witness fails; a moved violation is true in Chrome and you have no local proof left to try; a false
positive appears anywhere. Machine rules stay: one heavy run at a time, `sc-heavy` lock, no broad `pkill`.

## Prelude: lands before the branch is cut

One commit, no behaviour change, kept small on purpose.

| Piece | Where | Why it cannot wait |
|---|---|---|
| Named blockers, every one below, as `pub const` | new `rust/crates/solid-reactive-ir/src/blockers.rs` | both tracks would append to `lib.rs` otherwise |
| `unplaced_run(file, span, lookup) -> bool` | `execution_role/site_reach.rs`, `#[allow(dead_code)]` until adopted | both tracks read it |
| Split the no-row arm | `local_access.rs` `discover_call_as`, the `else` of `let Some(multiplicity) = …` at `:2294` | reads and writes share that block: move it unchanged into `fn unreached_read`, add empty `fn unreached_write` beside it, so the tracks edit different functions |
| Two generator stubs | `scripts/fuzz/flip-read.mjs`, `flip-write.mjs`, wired into the grid list | neither track edits the other's or `misuse.mjs` |
| Copy this directory | `scripts/flip/` | the other machine reads only the branch |

`unplaced_run`: `SiteReach::site(…) != Unreached` and a layer from the site out to the function that holds it is any of: an object
method, accessor or slot (`object_slot_value` misses accessors, `property.data` is false there), a class member, field initializer
or constructor (a field initializer has no containing function: test `ast.classes` elements), a tag function, a function in an
array or `Map` literal, or `function_escapes_unfollowed`. Hidden call sites (getter, `toString`, `valueOf`, `Symbol.iterator`,
tag) have no call node, which is why this is decided from the function, not the call.

Also check first what the fixes head already carries from `queue.md` section 1 (below as Q1-Q6), and skip what is done.

## Two tracks

**Track R, reads and async** (values, rendering, facts): SC5007, SC8003, SC1005, SC1007, SC2007, SC7002, SC1004, SC1003, SC5008,
SC5006, SC5003 + SC5001 (one commit, one fact), SC1002, SC8014, SC8015, SC1001, **then SC9012 last**.

**Track W, writes and owners** (roles, stacks, servers): SC7001, SC2004/5, SC3002, SC7006, SC7005, SC7007, SC2006, SC2008, SC6002,
SC6001, SC3001, SC2009, SC4002, SC4001, SC2002, SC2003, **then SC2001 last**.

Project-level, on neither: SC9005, SC9014, SC9015, SC9016 need no edit. SC9013 goes first on W (one small edit in the backend).

**SC9012 and what spans both.** F0 and the value-use census are on R, last, because they read transport tables and reads. W must
not build them. W witnesses that only the census closes stay failing on W, and are marked below: `SC2003` `c04 c15 c16 j05 j07
j08`, `SC2009` `a02 a03`. They pass after the merge. `check.mjs` failures on exactly those are expected on W.

| File both could touch | Who owns it, or how they stay apart |
|---|---|
| `local_access.rs` | R: `unreached_read`, `discover_caught_reads`, `push_accessor_read*`. W: `unreached_write`, `:4050`, `discover_optimistic_reach`. No shared hunk after the prelude |
| `static_api.rs` | W, except R's two hunks at `:126-168` (SC7002) and `:262-281` (SC2007). Disjoint |
| `upstream_compat/shared_reactivity.rs` | R `:792-960`, `:1062-1110`. W `:1115-1525`, `:3299+` |
| `projection.rs`, `lib.rs` | R only. W needs no new row fields |
| `apps/solid-typefacts`, `typefacts` | R only (`return_open`). W's `AssignmentFact` field and for-target fact are in `solid-facts/src/ast`, which R does not touch |
| `findings-snapshots`, `cases.json`, `rediscovery.json` | Both, append-only. **Conflict certain.** At merge take union for the JSON lists; never hand-merge snapshots: regenerate with `make coverage-update` after reading |
| `docs/decisions.md`, `open-work.md` | ADR numbers: R 0650-0699, W 0700-0749. Budget bumps in `doc-budget.mjs` conflict: take the larger |

## The rows

Steps a violation must prove: **P** position (runs where claimed, no escaping closure), **M** mount form, **W** window or owner,
**E** effect (the operation really does what is claimed). Each row: what is assumed today, the edit, the blocker.

### Track W

**SC9013.** Clean: `Detection::Defaulted { manifest: Some }` (`dialect.rs` `detect_detailed`) analyses a `solid-js` whose version
names no major, certified (measured: `"version":"nightly"`). Edit: `Defaulted` with a manifest emits the SC9014 notice, blocker
`solid-version-unclassified`. Warn: decided by the manifest. Test: a fixture, no Chrome.

**SC7001.** Clean: `static_api.rs:93` asks only `CreateEffect`; `effect_api.rs` returns `No` for the rest. Edit: Dialect question
`effect_function_slot(primitive)` (createEffect 1, createRenderEffect 1, createReaction 0); `:93` and `classify_effect_call` run
`v2_effect_function_status` on it. Blocker `effect-argument-shape-uncertain` (exists). Warn: P M W are the call; E proven by
`runtime_type_escape`. Nothing assumed. Witnesses `f01 f02 f03`.

**SC2004, SC2005.** No flip needed. The final default of `resolve_scope_unplaced` is an obligation, and `past_suspension` and roles
are positive. Tried: alias, object, method, HOF, class, `Map` (`regress`, all open). Warn: all four proven; run `regress` only.

**SC3002.** No flip needed. `reports_disallowed || reports_unplaced` and no reachability gate. 8 programs open or true. Warn: W is
a role; `runWithOwner(null, …)` clean and `untrack` reported, both measured right.

**SC7006.** Fully decided, with one edit. Enumerates every export and every directive, default `Unresolved` is an obligation.
Tried 8 spellings. Edit: `export_shape`, `binding.initializer_function` counts only for a binding never reassigned, else
`Unresolved` (`export let f = async () => {}; f = 1 as any` reads as a function). Inferred, not run on a server. Warn: compiler bytes.

**SC7005.** Clean: `server_rules.rs:116` drops any role but rendering, including `Unknown`; `:133` drops what `dominated` misses.
Edit: `:116` skips only `EventCallback`, `DeferredCallback`, `EffectApply`, `DirectiveApply`. `:133`: extend `loading_wrapped_components`
through project calls and computes, and a JSX child of a component that renders `Loading` around `props.children`; otherwise emit
unless the function is proven a shell root. Blocker `response-head-position-unproven`. Warn: already uncertifiable only. Witnesses
on the real SSR host (`scripts/fuzz/ssr-host.mjs`, status 200 not 410): `helper method memo wrapper`.

**SC7007.** Clean: `server_rules.rs:301` `function_called_at` else, `:334`. The value check itself defaults to an obligation. Edit:
loop like `primitive_value_references` over references of a server-function symbol not in callee position, emit
`Unresolved`, blocker `server-function-value-handed`, unless a serializer is proven. Warn: P E proven; a call in a server scope
is excluded. Runtime not measured: the harness plugin does not extract `"use server"`. Say so in the commit.

**SC2006.** Clean: `action_step_body` is `None` for methods (`action_step_body_through` filter), a function handed by name, a
module-level class (`crate::owners::containing_ast_function(&file.ast, anchor)?`). Edit: after `:293`, `kind` throws in a step,
no proof, `unplaced_run(call.span)` and the program has a stepped callback: emit uncertain, blocker `function-run-unplaced`.
Witnesses `a02 a03 a05 a06 a08 a09 a10`. Warn: W lexical, E dev throw. `f10` (behind a `return`) is unexecuted, not false.

**SC2008.** Clean: `:365` net fires only under `synchronous_event_flush_dispatch_present`. `b05`, an object method called from an
apply, certified, warns. Edit: gate becomes `dispatch_present || unplaced_run`; the rest of the block stays. Blocker
`function-run-unplaced`. Also effect_phase `None` arms for an apply passed by name or in a `{ effect }` bundle: same net. Warn:
claim is universal and measured, `flush(fn)` included.

**SC6002.** Clean: `ownerless_effect_phase_origin` `None` at a module-level method (`effect_phase.rs:215`, `unplaced_layer`'s
`containing_ast_function(anchor)?`). Throws in Chrome, certified. Edit: `else` after `:463`, client build, creating call,
`unplaced_run`: uncertain, blocker `function-run-unplaced`. Witnesses `f04 f07 e05`. Warn: W walks no owner-setting layer,
measured (`runWithOwner` clean), flush veto holds.

**SC6001.** No flip needed: all 7 are open through SC4001 at the same site with a blocker, and the claim is SC4001's. Warn: P is a
compiler callback role; an owner supplied inside goes uncertain (measured).

**SC3001.** No flip needed on the clean side: 11 programs across alias, method, class, `Map`, getter, `Proxy` all open or true.
Warn, **E assumed: the primitive's identity.** `SC3001 createOwner` at `solid-three/core/index.tsx:118` (`store[1](storePath(…))`) and
`inferay PerformanceRecorder:199` (`recordUiTimings`, no Solid primitive in its text) name a `createOwner` no source I read holds.
First step of this rule: trace `Primitive::CreateOwner`'s attribution there. If it cannot be shown, uncertain, blocker
`leaf-operation-identity-unproven`. My `storePath` probe under a tracked effect did not throw and got SC9012.

**SC2009.** Clean: `:3156` `callee_reaches` false. Edit: after `transported_writes` in `reactive_analysis.rs`, a transported write
of an async optimistic setter pushes uncertain; in `discover_optimistic_reach`, a handed member callee in a project naming one
pushes uncertain. Blocker `optimistic-reach-unresolved`. `a04`. Census closes `a02 a03`. Warn: W proven in the creating pass;
`loadingValue` measured quiet and clean.

**SC4002.** Clean, **the largest hole in this track**: the census asks "is there a provider anywhere", `context_census.rs:103`
`escaped || provided` → no verdict → silent. `g02`, a provider in a sibling subtree: certified, throws. Edit: keep every candidate
in the map with a third state `Open`. `owner_emission.rs:334`: `context_key_at` `None` is `Open` too (`g06`); `Open` reads push
SC4002 uncertain, blocker `provider-not-proven-above`. Then the local proof, in this commit: every render of the reading function
resolved and under `<Ctx value>` (reuse `loading_wrapped_components`' walk with all callers; `a07` shows a conditional provider
must not pass). Warn: needs closed program, no provider, read runs: all proven today.

**SC4001.** Clean: `owner_emission.rs:327` `context == 0` is a default arm. Live witnesses: none reached; 6 tried are open. Edit:
`context == 0 && !unreached` → unowned, arm `owner_emission.unplaced`, blocker `function-run-unplaced`. Warn, **W: one proven
unowned run, with owned callers.** `kui session.ts:749`: `createBrowseSession` has an owned caller (`MessagesRoute:137`) and story
callers. A mixed hook is V only if an unowned call is proven to run (`a01` U, `a02` V): check what proves it there; else uncertain,
blocker `owner-varies-by-caller`. `runWithOwner` and `createRoot` clean, measured.

**SC2002.** Clean: as SC2001. `e03 e06 d05 d06 e06-getter`. Edit: `unreached_write` handles actions too. Warn: `x-*` 17 mounts all
throw, `<Dynamic>` included; nothing assumed.

**SC2003.** Clean: (1) `no_direct_mutation` walks `ast.assignments` only, so `for (s.a of …)`, `for (s.a in …)` are invisible
(`c02 c03 j01 j02`): new `for_targets` fact in `solid-facts`, chained into `assignment_write_targets`. (2) `no_store_standard_library_write`
needs a member callee: resolve by `standard_library_declaration` for `const as = Object.assign` (`j04`). (3) `handed_container`
`return false` tails: a call of a project function (`get().a = 2`, `j08`), a callback parameter (`j06`). Follow queue item 10:
forward flow from proven and handed stores. **Queue item 10 rejected "any parameter may be a store" and the owner's later word says
cost is no reason.** Owner's decision: forward flow, because it reaches `j06`, with a parameter of a function open to unseen callers staying a clear uncertifiable. No blanket. Blocker
`unproven-container-root` (exists). Census closes `c04 c15 c16 j05 j07 j08`. Warn, Q1, **E assumed: every operation writes.** Needs
`AssignmentFact.logical`; `??=`, `||=`, `&&=`, `sort reverse fill copyWithin`, `Object.assign` are V only under ADR 0491's literal proof,
no elisions, no range arguments, closed literal source with an own key; else uncertain, blocker `store-operation-may-not-write`.
These stay proved: `push()`, `splice(0,0)`, `delete` absent, `a = a`.

**SC2001.** Clean, **the largest hole in the checker**: `discover_call_as` `let Some(multiplicity) = …` has an obligation arm for
reads and none for writes, so any setter in a function no call edge reaches is dropped. Object method, class method, constructor,
field, getter, `toString`, iterator, tag, `Map`-held: certified, Chrome throws. Edit `unreached_write`: setter or action, site
`MayRun`, call `push_setter_write`/`push_action_invocation` with `alias_withheld = true`, multiplicity 1, which forces
`ExecutionRole::Unknown` through `reports_unplaced`. Same at `:4050` and `static_api.rs:691`. Blocker `function-run-unplaced`. Better:
add a tag-template edge in `reachability.rs` so `e10` is proved, not open. Warn: P M W E all measured: 17 mounts throw; same value
and a no-op store setter throw; `runWithOwner(null)`, `ownedWrite`, a timer in a memo all stay clean. Nothing assumed.

### Track R

**SC5007.** Clean: `loading_hold.rs:118` skips `Unknown`/`None`; spreads never read; `Loading({…})` never read. `any`, `unknown`,
prop, spread, call form: all warn in Chrome, certified. Edit: `:118` skips only `NonCallable`; a spread not closed against `on`;
a loop over `Loading` calls reading the `on` property. Blocker `loading-on-value-unproven`. Warn: `Callable` proven, E first run.

**SC8003.** Clean: `syntax.rs:86-88` `.unwrap_or_default()` and `.filter(!computed)` read only closed literal spreads. Edit: an open
spread on an intrinsic beside any content source, or two open spreads: uncertain, blocker `spread-content-unproven`. Skip only when
Type Facts prove no content key. `violation()` hardcodes `uncertain: false`: set the field. Pin in `cases.json` (invariant 5,
`upstream-parity` skill). Warn, **E assumed: the attribute has a value.** `f04` `textContent={undefined}`: children render, nothing
lost. A possibly nullish value is uncertain, blocker `content-source-value-unproven`. `innerHTML={undefined}` does lose the child
(`<div>undefined</div>`): keep.

**SC1005.** Clean: `:1077` needs a proven accessor; `:1091` `is_argument` exempts everything; `:1094` two positions. Edit: (1) in
`:1091`, a standard-library parameter that cannot invoke (`may_invoke_argument`, `:239`) is a position. (2) `coercive_operands` and
template expressions whose value Type Facts call `Callable` and `handed_value` says came from a caller. (3) equality operand,
JSX spread value, `as any` on a native attribute. All uncertain, blocker `accessor-value-coerced`. Witnesses `a02 a04 a05 a06 a07 a10
a11`. Warn, **E assumed: stringifying gives the source.** `f07` assigns `c.toString`. Refuse the proof when a member write on the
accessor exists: local, one check.

**SC1007.** Clean: `member_symbol` `None` for a hook or `Map` root falls through. Edit: root `handed_value` → uncertain ReactiveHandlerRead,
blocker `handler-root-unproven`. Warn: `recreated` proven; `<Dynamic>` handler is live, not reported.

**SC2007.** Clean: `static_source_promise` → `None` for all but two shapes (page calls this silent), and `DynamicUnknownForm` is
unread. Edit: return `Promise | Sync | Open`. `Sync` only for a literal non-async function with a closed return type and no
`return_open`. `Open` → uncertain, blocker `static-source-unproven`. Same for `DynamicUnknownForm` on `:262`. Prove `n02 n03 n05` by
`computation_is_async`. Warn: proven `async`.

**SC7002.** Clean: `async_compute` false for `any`/`unknown` returns. Needs **`return_open`** (next rule). Edit: `:134`, `:150`.
Blocker `compute-return-unproven`. Warn, **E assumed: "can return" is "returns".** `f01`: a branch not taken, quiet, V. `:150`:
`uncertain: computation_may_answer_sync(…)`.

**SC1004.** Clean: `:1426` no name → `continue`. Edit: fall back to an empty label. Tests with no read row follow SC1001. Warn: M is a
JSX render; `{K()}` stays clean, measured.

**SC1003.** Clean: `:270` `parameters.len() <= 1`; `:346` only `const {a} = root`. Edit: (1) first parameter whatever the length;
(2) one generic arm: any pattern whose initializer, assignment value, `for…of` iterable, default or rest holds a reference to a
prop or store root outside nested functions: uncertain, blocker `destructure-root-unproven`. Needs rest-parameter patterns
(`rest_parameter` exists) and the for-of iterable. (3) `:464` tri-state: a call of a project or unresolved function → open. `o12`
closes with the census. Warn: PropUse proven, `<Dynamic>` true-but-quiet stays uncertain.

**SC5008.** Clean: `:3609` `continue`s a handed callee; member reads never enumerated. Edit: handed callee or prop member in the
`try`: uncertain, blocker `caught-read-unproven`. `l05 l07`. Warn: proven literal `async`, per pass.

**SC5006.** One edit. `loading_hold.rs:207` asks only no-argument calls and `:218` trusts a resolved function. `b03` `o.r(2)`: warned,
no SC5006. Edit: drop `:207`, replace `:218` with `function_summary_complete` once R has it, `unplaced_run` until then. Uncertain only.

**SC5003 + SC5001.** Clean: `computation_is_async` misses `any`/`unknown`. Edit: producer sets `return_open` on `AsyncFunctionFact`
(`source_facts.rs:572`), schema bump; sources get `provenance_open`; `async_read_reported` projects uncertain, blocker
`async-provenance-unproven`. `j06` by `unplaced_run`. Warn SC5003, **new false positive, W assumed: the source is pending.** `e02`:
mounted after it settled, quiet, V. V only when the read is on the creating pass's stack (`written_in_creating_pass`'s test); else
uncertain, blocker `source-pending-unproven`. Warn SC5001, Q2, **M assumed.** `p-dyn`: quiet, suspends, retries, V. `<Dynamic>`, and
`clientOnly` before load, are no-window mounts; the `component` prop joins the exclusive-mount census; `clientOnly` uncertain. Q6: the
inversion follows. Blocker `mount-form-window`. Measured throw on 11 other mounts.

**SC1002.** Clean: `c05` census. `c08` `.then(rd)`: extend `after_await.rs`'s literal callback to a resolved named function. `c12`: async
helper calling its parameter after await, from a compute (`:591` `continue`): uncertain, blocker `handed-read-after-await`. Warn:
ADR 0195 measured (`f19 f20`).

**SC8014, SC8015.** No runtime truth, so no Chrome witness. Clean: `has_proven_read` is the gate at `structure.rs:78,521,551`. Edit:
tri-state, `Open` when the expression holds a non-literal root the reads do not settle: uncertain, blocker `preference-read-unproven`.
Invariant 5: `upstream-parity` skill, pin in `cases.json`. Store receivers (`d04`) are missed today: prove them.

**SC1001.** Clean: `unreached_read` asks `function_escapes_unfollowed || inside_class_member`; widen to `unplaced_run`. `c01-c06`.
`props.*` path at `:2577` and `discover_member`: same. Warn, Q3, **P assumed.** The two fixes in `inside_non_component_function`
(`async_context.rs:1282`) and `inheriting_call_role`: see queue. Widen to **store-setter callbacks**: `CloudStorageContext:238`, and
`a01`. `a02` is clean, so the shape is the method. M: `<Dynamic>` stale is true, keep, fix the hint. Measured V and warns: apply, `Show`
and `For` callbacks, `index()`, `ref`, `.bind` in a handler, helper before an await, `t(() => …)`. E: nothing assumed.

**SC9012.** Last on R. Order: F0 (`function_summary_complete`, drop at `reactive_analysis.rs:219-230`), F-C (`callees.rs:908`,
`result_reads.rs:613`, a computed callee never takes the inner call's fact), F-D (`children`, `mapArray`, `repeat` results are
accessors; a primitive's result called directly is a read), the census, F-B (`handed_value` total, `local_access.rs:200-342`).
Census: new module, called after the `dispatch_obligations` loop at `:327`; position `match` whose **default arm is the obligation**,
blocker named per position. Followed and unfollowed tables below. Do not edit the ported `reactive_source_uncaptured`. Exclude
`splitProps`/`mergeProps` views. Premise to record: `eval`, `new Function`, string global channels. 83 witnesses.

| Followed | Unfollowed, the default |
|---|---|
| callee, wrappers, comma, `.call`/`.apply`/`.bind` | assignment to a `let` or member |
| member Get on a store or props | `??`, `\|\|`, `&&`, `?:` as a value |
| JSX attribute, child, spread | class field, `this.x =` |
| primitive argument, `const` alias | `yield`, `await`, `new`, parameter default |
| return or holder with `ClosedUses` intact, no `this` | standard-library carrier (`Object.assign`, `Map.set`, `Array.from`) |
| `typeof`, `===`, `!`, a test | argument to an unresolved callee, or a literal handed to one |
| reviewed fresh-stack (`setTimeout(set)`, `.then(set)`) | value returned from a primitive's callback |
| void or primitive std parameter, project parameter where `function_summary_complete` | |

**SC9005, SC9014, SC9015, SC9016.** Fully decided or already obligations. SC9016: 4 spellings tried. SC9015: refusals from the compiler,
2 corpus violations proven.

## What loses a warning among the 139 non-style corpus violations

Read by shape against Chrome probes, not by running the flip.

| Site | Rule | Why | Lose? |
|---|---|---|---|
| `app-game/.../cloud-storage-tree/CloudStorageContext.tsx:238` | SC1001 | read in a store-setter callback in an object method; `a01` quiet | **Yes, false today** |
| `app-game/packages/solid-three/core/index.tsx:118` | SC3001 | `createOwner` not in the text | **Probably**, unless traced |
| `inferay/.../PerformanceRecorder/index.tsx:199` | SC3001 | same | **Probably**, unless traced |
| `kui/.../feature-messages/src/session.ts:749` | SC4001 | owned caller exists | **Possibly**, depends on the story path |

The other 135 keep: SC1001 118 (apply, `Show`/`For` callbacks, `ref`, seeds, body and helper reads, shapes measured), SC4001 9 (`ref` apply, an apply, after `await`), SC3001 2 (`Header`,
`glasselated`), SC1004 2, SC5007 1, SC6001 1, SC9015 2. All shapes that warn or throw in Chrome.

## New wrong warnings this audit found

`e02` SC5003, `f01` SC7002, `f04` SC8003, `f07` SC1005, plus the corpus site above. Reach is 0 on the corpus except the corpus site.

## Tool warning

`api-census/tsc.sh` always printed `tsc=0`: `$?` is read after `$(basename)`. Read `tsc.txt`. All 311 witnesses here passed that
check. Earlier audits relied on the same script; 20 of their programs have non-empty `tsc.txt`.

## Not checked

SC7007 at runtime. SC7006's `let` edit on a server. SC4002 as a violation (needs a closed program). Every `continue` in `cleanup.rs`
line by line: SC3001 rests on 13 programs, not a full reading. `effect_phase.rs` `None` arms beyond the method path. Rows with `Live: none`
have no proven defect, only a reachable default.
