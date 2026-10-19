# ADR 0323: `children` is a tracked lazy memo, and a props or store read in a callback follows its consumer

- Status: accepted and implemented (2026-10-18). Two independent parts; each
  was measured alone against `/tmp/sweep-final14.json`.
- Owners: part 1: the `Primitive::Children` rows of `callback_positions`,
  `callback_executions` and `callback_owners` (`solid-dialect/src/solid_2.rs`).
  Part 2: `unclassified_callback_reaches_modelled_consumer`
  (`solid-reactive-ir/src/execution_role.rs`) and its two call sites, the props
  member loop and the spread loop of `LocalAccessContext::discover`
  (`solid-reactive-ir/src/local_access.rs`).
- Relation: part 2 reads ADR 0315's `callee_use_site_role`, ADR 0319's flow
  role and ADR 0321's literal-property resolution through the existing
  `semantic_execution_role_within` and `callee_callback_timing`; it adds no new
  timing proof.

## Part 1: `children(fn)` is a tracked lazy memo

### Context

The dialect had no callback row for `children`. A function literal handed to a
primitive the dialect says nothing about keeps its lexical role, so for
`children(() => n())` written in a component body the read was classified
`UntrackedRendering` and reported as a **proven** `SC1001` violation. That is a
baseline false positive for signals (found while measuring part 2: the same row
would have turned `children(() => props.children)`, the idiom, into four
violations in four apps).

### Evidence

`solid-js@2.0.0-rc.13`, `dist/solid.js:21-34` (prod) and
`dist/solid.dev.js:20-34` (dev):

```js
function children(fn) {
  const c = createMemo$1(fn, { lazy: true });
  const memo = createMemo$1(() => flatten(c()), { name: "children", lazy: true, sync: true });
  ...
}
```

`createMemo$1` is `@solidjs/signals`' `createMemo` (`accessor(computed(compute,
options))`, `dist/dev.js:2356-2358`). `computed` builds a node that owns
`compute` as its function and, with `lazy`, starts it as `REACTIVE_LAZY`
(`@solidjs/signals@2.0.0-rc.13`, `dist/prod/core/core.js:737-773`, line 773):
it first runs when the result is read, under that node as the observer. A
read in `fn` therefore subscribes the memo, never the body that wrote the
literal, and Solid raises no strict-read warning for it. The same function
bytes are in the rc.0, rc.6, rc.8 and rc.9 archives read for this ADR
(`solid.js:21-34`). Releases nobody read (rc.10-rc.12, rc.14+) take this answer
under the SC9014 notice, like every other release-independent row of the
dialect. The Solid 2.0 RFCs (`documentation/solid-2.0`) do not describe the
helper, so no claim rests on them; only the audited bytes are claimed.

### Decision

`children` argument 0 is `Execution::Tracked`, owner `Creates` (the memo owns
`fn`'s runs, as `createMemo`'s row), position 0 in `callback_positions`.
`runs_callback_deferred` stays false. **No `TrackedCallbackTiming` is stated**:
the memo is lazy, so `fn` does not run during the call (`createMemo`'s
`DuringCall` row would be false for it). The contract word
(`contract_callback_execution_at`) stays unstated: package contracts are
deleted and this ADR does not restate them; the pinned dialect test now
distinguishes the two questions explicitly.

### Corpus

Alone: **0 removed, 0 added** (4,043 uncertifiable, 553 violations unchanged):
no baseline finding sat in a `children(...)` callback. Fixture
`children-tracked-memo`.

## Part 2: a props or store read in an anonymous callback follows its consumer

### Context

`LocalAccessContext::discover` dropped a props or store read whose innermost
function is an anonymous literal that no compiler role places
(`inside_unclassified_callback`), unless the role was an effect apply or an
untracked callback. A signal read in the same literal is not dropped: it
reaches `callee_callback_timing` and is a violation (the callee invokes the
callback in its body) or an uncertifiable `SC1001` ("passed to a consumer that
is not proven to invoke it"). So `now(() => console.log(props.x))` with
`now(f) { f(); }` was silent while `now(() => console.log(n()))` was reported,
and ADR 0321's options-literal resolution widened the gap
(`useThing({ onClose: () => console.log(props.x) })`).

### Decision

The old gate stays for every callback the new predicate does not name. The
predicate admits a read when **every function layer between it and its
component** is one of:

1. an IIFE (the literal is the callee of a call);
2. an argument, directly or inside an array or object literal argument, of a
   call that `project_callee_at_parameter` resolves for (a project function, or
   package source selected as project code under ADR 0284 and 0297);
3. an argument of a standard-library call that the audited
   `argument_behavior` rows model as an inline callback and whose callback is
   that literal (`forEach`, `map`, `filter`, `find`, ...).

and the layer above the last one is a proven component. An admitted read is
classified by the machinery signal reads already use (`semantic_execution_role`,
then `callee_callback_timing`): the role ADR 0315/0319/0321 place; the callback
invoked in the callee's body is a proven read; anything unproven is
uncertifiable; event, deferred and tracked roles report nothing.

Refused (the read keeps the silence it had): a literal on a component
attribute; an unknown, package-declaration or unmodelled callee; a primitive's
callback (the dialect arms own them); a standard-library call that may keep or
schedule the function (`setTimeout`, `then`, `addEventListener`); a stored
literal (object property, array element, returned closure, binding); a
default-parameter initializer; a named function or method layer (the
interprocedural summary owns those); an *innermost* anonymous layer that is
itself a possible component (`render: (row) => <tr/>`: its first parameter is
no props object); an anonymous layer that is only an uncertain component. One
unmodelled layer anywhere on the way up refuses the whole read
(`IifeInTimer`, `ForEachInTimer`).

### Remaining inconsistency with the signal twin (owner decision)

Props and store reads in callbacks on component attributes, unknown callees,
standard-library schedulers and stored literals **stay silent**, as before this
ADR. The signal twin of each is an uncertifiable `SC1001` today. Closing it for
props and store reads was measured (first draft, gate removed outright):
**+811 uncertifiable `SC1001`** (+100 en-passant artifacts counted inside; kui
265, openbot 161, app-game 52, derp 42; 643 of them "passed to a consumer not
proven to invoke it", mostly `onClick={() => props.x}` on components and
accessor options), plus 4 false violations (`children(...)`, fixed by part 1)
and 2 `SC8015` style findings that the extra reads fed. It was not shipped.
The silence is a miss, never a claim.

### Proof obligations

| Place | Decision | Premise | Fixture |
| --- | --- | --- | --- |
| `children` position 0 | tracked, owner `Creates`, no timing | `fn` is `createMemo(fn, { lazy: true })`: its own observer, first run at first read (rc.0-rc.13 bytes read) | `children-tracked-memo` |
| `children` homonym | a project `children2` is not the primitive | resolved by the `solid-js` import's symbol, never by name | `children-tracked-memo` (`HomonymControl`) |
| `children` contract word | stays unstated | contracts are deleted; attribution and contract words are different questions | dialect unit test |
| gate: IIFE layer | "runs where written" | the literal is the callee of a call; the walk continues from that call | `Iife`, `StoreIife`, `NestedModelled` |
| gate: project callee | "the callee resolves" | `project_callee_at_parameter`; the timing is *not* decided here, `callee_callback_timing` decides it afterwards | `ProjectDirect`, `ProjectOptions`, twin |
| gate: standard-library layer | "inline" | audited `InlineCallback` row for that declaration and argument; the literal is the argument | `StdlibForEach`, `StdlibMap` |
| gate: any other layer | refuse | one unmodelled layer makes the whole chain unproven | `Silent.tsx` |
| gate: component at the top | the read is in a component body | `ComponentStatus::Proven` on a layer that is not the innermost | `Silent.tsx` (`Renderer`) |
| "invoked in the body" | proven | unchanged: `invokes_parameter_during_call` on an exact, spread-free mapping; a member call on the parameter is not that (`ProjectOptions` stays uncertifiable) | `ProjectOptions` |
| "deferred" | not reported | unchanged: ADR 0315 use-site role of the callee (a timer) | `ProjectDeferred`, twin |
| violation vs uncertifiable | props backing | unchanged: a proven violation needs caller-proven reactive backing; an exported component's props are an obligation | `ExportedDirect` |
| a cached answer is current | the gate and the timing read the callee file | the existing per-file cache fingerprint; the retained twin edits the callee body, drops its body, and moves the read between shapes | `callback_props_reads_process.rs` |

Self-review against the bug families found so far:

- **Key/receiver/origin identity.** No key or receiver comparison is added:
  the callee is the resolution `callee_callback_timing` already trusts, and an
  inexact mapping (spread, `.call`, replaced binding) is reported uncertifiable
  by it.
- **Visible references only.** The predicate asks no census; it only decides
  which reads are considered. Whether the callback runs is still decided by the
  existing timing and flow answers, which carry their own censuses.
- **Script files.** Unchanged: ADR 0319's walk refuses scripts before the role
  is known; the predicate only widens consideration.
- **Span equality.** The outermost-literal and innermost-call choice mirrors
  `callee_callback_timing` byte for byte, so a read admitted here is the read
  that function judges.
- **Nested function inheriting an outer method name.** A layer with a method
  name or a binding name is refused outright.
- **`__proto__`/`super`, quoted and numeric keys, forwarding, shared literals.**
  Not decided here: they are properties of the role and timing answers.
- **Skippable awaits.** An `await` before the call does not matter here; the
  existing `named_helper_runs_during_body` / role arms refuse them.
- **Primitive-by-cast, destructured parameters.** A literal whose parameter is
  destructured is still a literal at that call; unchanged answers.
- **Merged alternatives.** A conditional holding a literal is not the argument
  itself; the walk finds no call argument and refuses (not an IIFE).
- **Retained staleness.** `callback_props_reads_process.rs`: a callee file
  turning body invocation on and off and dropping its body, and a local file
  moving a read between a project-independent inline callback, a timer, an IIFE
  and an IIFE in a timer, retained against cold under a closed and an open
  boundary.

### Corpus

Part 2 and part 1 together against `/tmp/sweep-final14.json`: **0 removed,
284 added, all `SC1001` uncertifiable** (4,043 to 4,327; 9 of them
en-passant, which has no installed `solid-js`; violations unchanged at 553).
kui 161, app-game 28, openbot 31, probus-hk 16, solid-groove 13, inferay 10,
finds-team 5, derp 3, jandibat-web 3, expenses-app 3, ai-memory-ui 2. All read
"passed to a consumer that is not proven to invoke it": project hooks that take
accessors (`useEta(() => props.active ...)`, `createVirtualizer({ get count() {
return props.items.length } })`, `createQuery(key, () => ({ connectionId:
props.connectionId }))`, `useAgentContext(() => _props.paneId)`), none a
violation. The two `SC8015` sites the first draft raised (inferay
`DiffRow.tsx:136`, `SidebarFooter.tsx:43`) do not appear under the narrowed
gate; they were fed by reads the first draft admitted from a layer this gate
does not admit, and since they do not persist they were not classified
further. A callee whose body the checker reads (project code or selected
package source) is the only thing that can turn one of these into a decision.

Fixtures: `children-tracked-memo`, `callback-props-reads` (types are the
published rc.13 declarations, `tsc --noEmit` clean). Residual: the
inconsistency above; `useThing({ onClose })` style options stay uncertifiable
because the callee reads the member, not the parameter, in its body.

## Review round 9 (2026-10-18)

An independent review of the version above found four gaps, fixed in this
order. Each was measured against `/tmp/sweep-final15.json` (binary
`/tmp/sc-final15`, 4,327 uncertifiable, 553 violations); the combined result is
**4,208 uncertifiable, 554 violations** (121 removed, 3 added, below).

### 1. A named helper inside an admitted callback

`now(() => { const g = () => props.x; console.log(g()); })` was clean while the
signal twin was a violation: the helper path of props reads (`project_helper_prop_read`)
admitted a call only from a component, a primitive callback, a control-flow
body or an event, so the call to `g` inside the admitted anonymous callback was
"unproven" and a helper's definition-site read is gated. An anonymous layer
that `unclassified_callback_reaches_modelled_consumer` admits is now an
admitted caller of its helpers; whether the call is during the body stays
`callee_callback_timing`'s question. Fixture `callback-props-reads`
(`NestedHelper`; refusals `HelperInUnknown`, `HelperInTimer` in `Silent.tsx`).
Corpus effect: none (no baseline site has the shape).

### 2. `omit` views

`const o = omit(props, "y"); o.x` was never reported, `merge` views were. The
binding of a call whose primitive the dialect says `splits_props` is a props
source of its first argument (a spread argument leaves the source unknown), and
a read of a key a string-literal hidden argument names, directly or through an
inner `omit` view of a `const`, reads nothing (`omit_view_hides_key`); a
predicate or other non-literal argument hides nothing nameable, so the key
stays a read. A read of a view is always uncertifiable (the view's keys are not
the parameter's: `exact_root` is false). A **spread of a view** is also never a
violation now, for `merge` and `omit` alike: the keys a view hides may be the
only reactive ones. Writing through the view is dropped (`set: trueFn`,
`@solidjs/signals@2.0.0-rc.13` `dist/prod/store/utils.js:850`), so SC2003 on
`others.ref = el` is a true finding; `tsc` types an `omit` result writable and
reports nothing on it. Fixture `omit-view-reads`. Corpus: **+1 violation**
(SC2003, app-game `solid-virtual/examples/shared/Item.tsx:68`, `others.ref =
element` after `others = omit(props, ...)`: verified true by the trap above),
**+2 uncertifiable** (app-game `Props.tsx:19` `rest.children?.(rest)`, a read of
a key `omit(props, 'ref')` keeps; `TypeGPUCanvasContext.provider.tsx:40`
`...configuration`, a spread of `omit(props, 'children')`). A first draft raised
that spread as a violation; the spread rule above is what made it an
obligation. No violation was removed.

### 3. Invocations in different non-reporting roles; primitive slots

ADR 0319 § 3 refused a callback whose invocations ran in more than one role.
The census the reviewer measured is mostly callbacks whose every invocation is
a non-reporting role in a *mix*: kui `useQuery`'s `key` (a lazy memo's compute,
and `untrack(options.key)` in `reload`), probus `useEta` (an effect compute and
caller-tracked accessors), jandibat's relay key, openbot `createDigitRoll`.
Two changes in `invocation_flow_role`'s engine (`function_role`, `uses`):

- **Mix.** Roles that are all `flow_safe_role` (tracked, deferred, event) merge
  into `ExecutionRole::Unknown`, the no-claim role. No single role describes the
  mix without overclaiming for some consumer (a tracked role claims a
  subscription the `untrack` run does not make; a deferred one denies the
  memo's), so none is invented. Consumers of the flow role: SC1001 strict reads
  and props reads (suppressed: `Unknown` is not a reporting role), SC1003
  destructuring (no claim), async pending reads (`async_execution_role` passes
  it through; no reporting role, as before for tracked), the dispatch-obligation
  paths `named_callback_invocations_hold` / `excluded_role_holds` /
  `await_continuation_decides_nothing` (they ask `flow_safe_role` of the answer
  and `Unknown` is not one, so they refuse a mix exactly as before: strictly
  less aggressive than the read rules, never more). Write legality never
  consults the flow. A caller merging a function whose own answer is the mix
  treats it as the mix it is (`non_reporting_mix`); `Unknown` is produced only
  by this merge. A reporting role among the invocations (the render body,
  an effect's apply) still refuses.
- **Primitive slots.** A function value handed to a primitive's callback slot
  (`untrack(options.key)`, `createMemo(f)`) was an unmodelled argument and
  refused the census. `primitive_callback_slot_role` gives it the slot's role
  when the dialect states one that keeps reads out of the window: tracked
  (`tracks_reads`) or deferred (`Execution::Deferred` or
  `runs_callback_deferred`, which is what `untrack` is). It refuses an apply
  slot, a slot `reports_untracked_reads_at`, `requires_return_invocation`,
  `stores_as_value`, `callback_runs_on_result_access`, an unresolved or invalid
  call, a spread, and any slot the dialect does not model, and a container
  holding the function (`path` not empty).

Fixture `mixed-role-flow`; retained twin `a_mixed_role_callback_follows_the_hook_file`
(a hook file gains a body invocation, loses its untrack, regains the body
invocation; retained equals cold, boundary closed and open). The updated
snapshot `invocation-flow`: `MixedRetained` (a timer and a click) is no longer
a pinned refusal.

Corpus: **121 removed** (88 `SC1001`, 33 `SC9012`), 0 added by this step. The
two halves overlap: with the slot half off, 95 of them come back (62 + 33);
with the mix half off, 91 (87 + 4); kui needs both. Every removed finding is an
uncertifiable. Read: kui `useQuery`'s `key` (`options.key()` in a lazy
`createMemo` compute and `untrack(options.key)` in `reload`; most of kui's 66
removals, in TopicsRoute, SchemasRoute, ConsumersRoute, ClustersRoute, BrokersScreen,
MessagesRoute, KsqlRoute, ConnectRoute), kui `createMutation`'s `call`
(invoked in `run`, which every caller runs from an event), kui
`const tab = () => new URLSearchParams(location.search)...` (a `createMemo`
compute and tracked JSX), every-deck-of-cards `Layout.tsx:8`
(`createEffect(viewport, ...)` compute and tracked JSX), probus-hk `useEta`'s
`target` (an effect compute through `subject()` and the returned accessor,
17 findings), civil `UrlBar.onNavigate` (`commit`, called from input handlers),
app-game's 33 `parameter-use-unmodeled` obligations of
`createEventListener(canvas, ...)` (the unmodeled forward is `access(targets)`
inside `compute`, a `createEffect`/`createRenderEffect` compute: tracked),
sefer `HistoryPanel.recompute` (named, handed to a tracked or deferred slot;
the call after its await is covered by ADR 0320's rule once it has an answer),
derp `MediaCenterFileBrowser.tsx:205`. None runs in the render window.

### 4. Reads after an unconditional await

`now(async () => { await 0; count(); })` was a proven violation at the
post-await read (and the props version an obligation). `follows_unconditional_await`
(statement-level awaits only, the fact ADR 0320 uses for dispatch obligations)
now turns every *reporting* role into `Unknown` in `semantic_execution_role_within`:
a read after the await runs on a later task, after `untrack`'s `finally` has
restored `strictRead` (`@solidjs/signals@2.0.0-rc.13` `dist/dev-shared.js:5801-5885`,
including the dev note that "deliberate post-await reads stay quiet"), so it
has neither a strict-read window nor a tracking scope. A skippable await
(`x ??= await p`, an optional-call argument, a labelled break) is not an
unconditional await: the read keeps its role and stays reported
(`SkippableAssign`, `SkippableOptionalCall`, `SkippableLabelled`). Tracked,
deferred and event roles are untouched. Fixture `await-continuation-reads`;
twin `await_continuation_edits_match_cold`. Corpus: none from the sweep itself
(the ablation is identical); two fixtures move: `package-tracked-optional-callback-consumer`
(`UnsupportedFrames`' async frame, whose read follows `await Promise.resolve()`)
loses its uncertifiable, as intended.

### Proof obligations added in round 9

| Place | Decision | Premise | Fixture |
| --- | --- | --- | --- |
| helper in an admitted layer | the anonymous layer is an admitted caller | the layer reaches a modelled consumer at every level; the call's timing stays `callee_callback_timing`'s | `NestedHelper`, `HelperInUnknown`, `HelperInTimer` |
| `omit` first argument is the source | the view reads through | `omit` of a props object is a proxy over it (`dist/prod/store/utils.js`: `omitView`, `sourceGet`) | `Direct`, `InCallback` |
| hidden string-literal key reads nothing | `isHidden` | `sourceGet` answers `undefined` for a hidden key without reading the source | `Hidden`, `Nested` |
| predicate or non-literal hidden argument | hides nothing nameable | the key set is unknown, so the key is a possible read | `Predicate` |
| a view's read is never a violation | `exact_root` false | the view's keys are not the parameter's | `Direct`, `Merged` |
| a view's spread is never a violation | the hidden keys may be the only reactive ones | `uncertain` when the symbol is not the props root | `Spread` |
| write through a view | SC2003 | the proxy `set` trap is `trueFn` | `Write` |
| a mix of non-reporting roles | no-claim role | every invocation is tracked, deferred or event; no single role is faithful | `TrackedAndUntrack`, `TrackedAndEvent`, twin |
| a reporting invocation in the mix | refuse | the body and an apply are reporting roles | `TrackedAndBody`, `TrackedAndApply` |
| a function handed to a primitive slot | the slot's role | the dialect states a tracked or deferred role, never an apply, storing or result-access slot | `SlotByName`, `SlotApply` |
| unconditional await | no reporting role after it | `strictRead` is restored when the synchronous window ends | `AfterAwait`, `StoreAfterAwait`, `PropsAfterAwait`, `AsyncIife` |
| skippable await | the role stays | the read may run in the window | `SkippableAssign`, `SkippableOptionalCall`, `SkippableLabelled` |

Residual: `useThing({ onClose })` options callbacks stay uncertifiable (the
callee reads a member, not the parameter); a mix that includes an effect apply
or the render body stays refused; the mix answer is not consumed by the
dispatch-obligation paths (they keep asking for one safe role); `children` and
`createMemo` slots handed a function by name inside a container are not
followed. Unknown consumers stay silent for props and store reads (option B).

## Review round 10 (2026-10-18)

A second review attacked round 9. Fixed in this order, each measured against
`/tmp/sweep-final16.json` (binary `/tmp/sc-final16`, 4,208 uncertifiable, 554
violations); the combined result is **4,210 uncertifiable, 554 violations** (1
removed, 3 added, none a violation).

### 1. `createRoot` and `runWithOwner` inherit the window of their call

Round 9's slot-following carried an older false negative into hook-passed
callbacks: the dialect listed `createRoot` as `runs_callback_deferred`, so
`createRoot(() => count())` in a component body was clean in every binary, and
`use(cb) { createRoot(() => cb()) }` made `cb`'s reads clean (`OnlyRootSlot`).
The runtime evidence (`@solidjs/signals@2.0.0-rc.13`, `dist/dev-shared.js`):

- `createRoot(init)` is `runWithOwner(createOwner(), () => init(dispose))`
  (`:3478-3481`);
- `runWithOwner` sets `context = owner` and `tracking = false`, runs `fn()`,
  restores both in `finally`, and never reads or writes `strictRead`
  (`:6919-6947`);
- a component body runs under `untrack(() => Comp(props), label)`, which sets
  `strictRead = label` for the synchronous call (`:5863-5888`);
- every reactive read warns whenever `strictRead` is set (`:6511-6516`).

So a read in either callback warns exactly where the same read written at the
call would, and nowhere else: not in an event handler, a timer or a promise
continuation. (`runWithOwner` used to be reported wherever it sat, including
there: a second false positive, `RunWithOwnerInEvent`.) `createRevealOrder` is
`createOwner()` then `runWithOwner(owner, () => { const value = fn(); … })`
(`dist/dev.js:10170-10179`) and joins the set (it is not in `solid-js`'s published
typings, so no fixture imports it).

New dialect fact `callback_inherits_calling_window(primitive, argument)`:
`(CreateRoot, 0)`, `(RunWithOwner, 1)`, `(CreateRevealOrder, 0)`. It is a
different question from `runs_callback_deferred`: that one says reads in the
callback subscribe nothing of the caller's; this one says whether a read in it
still warns. The read role (`semantic_execution_role_within`) of such a
callback is the role at the call, with a tracked call mapped to the deferred
role (the listener is cleared) and module initialization to no claim; writes
never consult it (`CreatedRoot` decides a write directly in a root before the
read role is asked, and the branch refuses while a write is being classified).
The flow walk makes the call the invocation of the callback
(`Invocation::Call`), `primitive_callback_slot_role` never answers for it, a
named function passed to such a slot is no longer admitted as a deferred or
untracked named callback, and the props/store gate treats such a callback as a
layer like an IIFE.

The other primitives the dialect lists as deferred were read the same way:
`untrack(fn)` sets `strictRead = strictReadLabel || false` (`:5863-5888`), an
explicit untracked intent with no warning, so it stays out; `onSettled`,
`action` and `clientOnly` run from a queue or an invocation, never at the call;
`resolve` and `until` evaluate the thunk as an effect compute and `recompute`
clears `strictRead` (`:4890-4896`); `createReaction`'s invalidation callback and
`lazy`'s loader are reporting slots already; `flush`, `latest` and `isPending`
inherit the caller's role already; `batch` is not an export of this vocabulary.

Fixture `root-window-reads`; retained twins `root_window_edits_match_cold` (a
root moved from the body to an event, a memo, a promise continuation and back)
and `hook_slot_edits_match_cold` (a hook's slot changed between `createMemo`,
`createRoot`, `runWithOwner`, a memo and an untrack mix). Corpus: **no new
violation**: no corpus component reads a signal inside `createRoot` in its
render window. **+1 `SC9012`** (app-game `PaintApplication.tsx:33`:
`createPaintRuntime` runs `modules.document()` inside `createRoot` and the
component calls it in its body: a true, undecidable obligation), **−1 `SC9012`**
(sefer `ProjectContext.tsx:1425`: `makeShell` runs inside `runWithOwner` in a
`.then` callback, no window: the removed finding was the position-insensitive
false positive).

### 2. A helper kept inside an admitted layer

`now(() => { const g = () => props.x; saved = g; })` and `const h = g; sink(h())`
were clean (the helper may run in the body through the reference that keeps it).
Inside an admitted anonymous layer, a reference to the helper that is not a
call and not a pass to a call (an initializer, an assignment, a return, a
property) makes the helper's read an uncertifiable obligation. A pass to a
call (an unknown consumer) stays option B's silence, and a helper only called
from an event or a timer stays silent. Fixtures `callback-props-reads`
(`HelperAliased`, `HelperStored`; `Silent.tsx`: `HelperEscapes`,
`HelperEventOnly`). Corpus: none.

### 3. Views

- **Inline is bound.** `omit(omit(props, "y"), "ref")`, `merge({ z: 1 },
  merge({ w: 1 }, props))`, `omit(props, "y").x` and `{ ...omit(props, "y") }`
  resolve through one function, `view_root` (the expression itself when it
  names a source, else an `omit`'s first argument or a `merge`'s first
  resolvable argument, to any depth); a spread cannot be a source, and a spread
  among an `omit`'s hidden keys hides nothing nameable.
- **Stores.** A view over a store reads the store (`viewSource(e)[key]`,
  `@solidjs/signals@2.0.0-rc.13` `dist/prod/store/utils.js`: the omit and merge
  proxies' `get`), so a binding of one is a store alias, as an effect apply's
  store parameter already was, and an inline store view is the store read.
  Hidden-key skipping covers stores too. A store read is a proven violation
  like `s.x` (`OmitOfStore`, `InlineStoreView`).
- **The key argument is evaluated.** `omit(props, props.k)` was clean because
  `result_access_callback_contains` took any expression written at an omit's
  predicate slot for the predicate. Only a function literal written there runs
  on result access; anything else is evaluated by the call, where it is written.
- **A non-literal key list hides nothing nameable**
  (`VariableKey`: the read through the view stays a read).
- **Wrappers.** Type Facts keeps no entity at an identifier under a TypeScript
  wrapper (`(props as P).x`, `props!.x`), so those reads were never classified
  at all: an older recall gap, now resolved through the binder and the
  reference's declaration (`reference_symbol`). `helper-prop-read`'s
  `WrappedRoot` pinned it as a gap and is now the violation its comment
  predicted.

Fixture `omit-view-reads`; twin `inline_view_edits_match_cold`. Corpus: **+2
`SC1001` uncertifiable** (inferay `DiffPanels.tsx:22-23`,
`() => omit(_props, "diff", "mode", "scrollRef").externalScrollTop`: reads of a
key the inline view keeps, in callbacks handed to a hook), no violation.

### 4. Self-check of the remaining slots

Probed with a hook handing `cb` to each slot (`/tmp/p0.noindex/e1`): a
`createRenderEffect` compute is tracked (silent, as the runtime), its apply is
a reporting slot (uncertifiable), `createReaction`'s callback and `lazy`'s
loader are reporting slots (uncertifiable), and `untrack(cb)` is explicit
intent (silent).

**`For`, `Show`, `Match` and `Repeat` children callbacks are strict-read
windows, not silent ones** (this section first said the opposite, wrongly:
a round 11 review corrected it). In `solid-js@2.0.0-rc.13`,
`dist/solid.dev.js` runs a `<Show>` child as
`untrack(() => child(c), "<Show>")` (`:1477-1480`) and a `<Match>` child as
`untrack(…, "<Match>")` (`:1524-1527`); `For` and `Repeat` pass
`options.name = "<For>"` / `"<Repeat>"` (`:1446`, `:1459`) and
`@solidjs/signals`' `mapArray` and `repeat` wrap the mapper in
`setStrictRead(options.name)` (`dist/dev.js:8890-8899`, `:9333-9340`). A
labelled `untrack` sets `strictRead`, so a direct read in such a callback
warns in dev, and the checker already reports it (an SC1001 violation for a
signal, an uncertifiable for a props read in a `For` mapper). A callback that
reaches one of these slots through a prop or a hook is an SC1001
uncertifiable (`ForSlotViaProp`, `ForSlotDirectProp`, `ShowSlotViaHook`; the
reviewer's repros under `/tmp/adr319-review.noindex/r9/e5`). My own probe was
silent for `ForChildSlot` and `ShowChildSlot` only because it called the hook
inside a JSX child, a tracked region whose role the hook's call takes; that
says nothing about the slot. Nothing in this ADR changes these slots.
`createAsync` and router `query` are not in the vocabulary, so a callback
handed to one is unmodelled and refuses. `unwrap` is not an export of 2.0;
`snapshot(state)` reads the store's internal target (`snapshotWalk` through
`$TARGET`), never the proxy `get`, so it raises no strict-read warning and is
rightly not a read. The mixed-role and slot-following twins are
`a_mixed_role_callback_follows_the_hook_file` and `hook_slot_edits_match_cold`.

Accepted over-report: `merge(omit(props, "y"), { y: 5 }).y` is an `SC1001`
uncertifiable although `props` is never read (`HiddenReAddedByMerge`): a hidden
key is not skipped through a `merge`, because another argument may supply it,
and the checker does not tell which one does. Conservative, never a violation.

Not covered by the round 11 review: the `lazy`, `Dynamic` and
`createAsync`/`query` callback slots, and the retained twins for two of the
round 10 twins.

### Proof obligations added in round 10

| Place | Decision | Premise | Fixture |
| --- | --- | --- | --- |
| `createRoot`/`runWithOwner`/`createRevealOrder` callback | the role of the call | `strictRead` is never touched by `runWithOwner`; a read warns whenever it is set | `InlineCreateRoot`, `InlineRunWithOwner`, `NestedRoots` |
| the call has no window | nothing | event, timer and promise continuation have `strictRead` clear; a memo clears it | `RootInMemo`, `RootInEvent`, `RunWithOwnerInEvent`, `RootInTimer`, `RunWithOwnerInThen` |
| `untrack(fn)` | no warning | it sets `strictRead` to `false` | `UntrackInline` |
| a hook runs `cb` in a root | the call is the invocation | `callback_inherits_calling_window` | `HookRootSlot`, `HookRunWithOwnerSlot`, `HookMemoAndRoot`, `HookRootInEvent` |
| write directly in a root | unchanged | decided by `CreatedRoot` before the read role | the `created-root` write fixtures |
| a kept helper in a layer | obligation | the reference may run it in the window | `HelperAliased`, `HelperStored` |
| a helper passed to unknown code, or event-only | silent | option B | `HelperEscapes`, `HelperEventOnly` |
| inline view = bound view | `view_root` | one expression, one answer | `OmitOfOmit`, `InlineView`, `MergeOfMerge` |
| view over a store | store alias | the proxy `get` is `viewSource(e)[key]` | `OmitOfStore`, `InlineStoreView` |
| key argument | evaluated in the body | only a function literal runs on result access | `VariableKey` |
| wrapper on a receiver | resolved | binder and declaration | `CastReceiver`, `HiddenThroughCast` |

Residual: a `createRoot` callback read in an exported component's props is an
obligation, never proven; a view's hidden key through a `merge` is not skipped
(another argument may supply it); `useThing({ onClose })` stays uncertifiable;
unknown consumers stay silent for props and store reads (option B).

## Amendment (2026-10-19): the silence is closed

ADR 0328 made uncertifiable results internal, so the cost that kept the
props/store gate (`inside_unclassified_callback` in
`LocalAccessContext::discover`) no longer reaches users. The gate is removed:
a props or store read in an anonymous callback is classified exactly like its
signal twin, and `inside_unclassified_callback` is deleted. Named helpers and
methods still go through the interprocedural summary
(`inside_non_component_function`). Corpus: **0 removed, +505 `SC1001` and +5
`SC9012` uncertifiable** (openbot 124, kui 99, en-passant 91 artifacts, derp
38, sefer 30), **0 new `SC1001` violations**, and **+2 `SC8015`** style
findings (inferay `DiffRow.tsx`, `SidebarFooter.tsx`: a `_props.x` condition
in JSX, the shape the rule flags elsewhere). The first draft's four false
violations (`children(...)`) stay fixed by part 1.
