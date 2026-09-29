# Audit: `reads` rows for seven rc.9 dialect primitives

Date: 2026-09-30. Status: **for the repository owner's review**, as the rc.3, rc.6
and earlier rc.9 audits it follows were. It establishes, on the exact bytes of
`@solidjs/signals@2.0.0-rc.9` and `solid-js@2.0.0-rc.9`, seven `reads` rows for
the dialect primitives that the `reads` census's call walk
([ADR 0165](../../adr/0165-the-reads-census-walks-calls.md)) refuses for want of
a row and that the ADR 0165 census names most often: `getOwner`, `onCleanup`,
`runWithOwner`, `untrack` and `createSignal` of the signals archive, and
`solid-js`' own `createSignal` and `useContext`. Nothing else of rc.9 was read
for `reads` here, so no other row exists
([ADR 0168](../../adr/0168-argument-scoped-reads-rows.md) lists what stays
refused).

**What a `reads` row denies.** The same as the rc.9 parity audit's group B
(`2026-09-27-solid-2-rc9-signals-negative-rows-parity.md` § 0): that one
invocation of the export, **at its call event, observes a reactive source's
current value through the archive's own code**. A read a caller-supplied
callable performs is that callable's author's (`semantic-model.md` § reads), and
so is a read of a callable this export authored that the census walks as the
export's own code. Two things follow, and they are why four of the seven rows are
*argument-scoped* ([`RowScope::Arguments`](../../../rust/crates/solid-dialect/src/lib.rs)):

- A row that runs a caller's callable is silent about **which** callable. The
  census can attribute the callable's reads only when the slot holds a function
  literal inside the implementation under the walk (whose calls are walked with
  its frame) or a value rooted at a parameter of the censused export (the
  caller's own). A callable passed *by reference* — a module-level helper, or an
  accessor of a memo this very call created — reaches code the census does not
  see, and it can be a read of a memo the call created (§ 8, probes).
- `solid-js`' own `createSignal` reaches the hydration gate (§ 6) only when its
  first argument is a function, so the row states that the first argument is a
  primitive by grammar.

**Identity.** `@solidjs/signals@2.0.0-rc.9`,
`sha512-o3pqiTgpH5NR2DstiKrt9s/6+0YOFtv+MfvLONwLsS247I+EWMMyTu9BkRcgd35UR5Pa1DM16lI1/5uaIMY6Gw==`,
`package.json` sha256 `c612461c9264f2b3509ced91ea91019ed7b1ea0df0d64bba7f0f30c8d00bb1c6`;
`solid-js@2.0.0-rc.9`,
`sha512-J/oHWnWqe7S0FeIEdIRKDvyyo+HY/TYKr2PrIB8VlePMWuErDg78QHqdsAV7f6HKa9qhWR/23eqzR/ZRV9ep0g==`,
`package.json` sha256 `c8d6224bd4bd63ed38f0cec77eb4d30d3f78debec091338dc4e466a2618e11bc`.
Both are the provisioned audited installs
(`rust/target/audited-archives/solid-v2/2.0.0-rc.9`,
`make audited-archives-provision`, each refused unless its tarball hashes to the
pinned integrity), and every cited file is checked against
`benchmarks/package-contract-v2/phase0/rc9/{solidjs-signals,solid-js}/files.json`
by the citation test. In the readings below `<sig>/` is the signals tree and
`<js>/` the `solid-js` tree.

## 0. For the owner's sign-off

| # | row | scope | verdict | the one claim |
| --- | --- | --- | --- | --- |
| 1 | `@solidjs/signals` `getOwner` `reads` | every condition | **GRANT** | `return context`. No read site is reachable in any build. |
| 2 | `@solidjs/signals` `onCleanup` `reads` | every condition | **GRANT** | Stores the caller's function on the owner (`cleanup`); dev adds diagnostics that read no reactive source. |
| 3 | `@solidjs/signals` `runWithOwner` `reads` | **argument-scoped**: slot 1 invoked | **GRANT** | Swaps `context` and `tracking`, runs `fn`, restores. Reads are `fn`'s. |
| 4 | `@solidjs/signals` `untrack` `reads` | **argument-scoped**: slot 0 invoked | **GRANT** | Clears `tracking` (or calls the external-source hook), runs `fn`. Reads are `fn`'s. |
| 5 | `@solidjs/signals` `createSignal` `reads` | **argument-scoped**: slot 0 primitive-or-attributable | **GRANT** | A non-function first argument builds a plain signal; a function one a memo whose compute is the caller's, the closure of `createMemo`'s granted row. |
| 6 | `solid-js` `createSignal` `reads` | **argument-scoped**: slot 0 primitive; delegate signals `createSignal` | **GRANT (narrowed)** | A primitive first argument never reaches the hydration gate or the server memo; it forwards to row 5's plain path. The flat row stays withheld. |
| 7 | `solid-js` `useContext` `reads` | every condition | **GRANT** | Plain property reads of the owner's context map; no reactive source. |

**Not granted, read and refused** (§ 9): `solid-js` `createMemo` (the hydration
gate is reached from any function argument unless `options.transparent`),
`solid-js` `createComponent` and `createRoot` (the dev and observe builds route
through `createRoot`, whose hydrated variant is not read here), `@solidjs/signals`
`createRoot` (a `solid-js` import does not resolve into it).

### What the owner is asked to agree with

1. **The `flush` cut, and the hook slots.** Exactly the rc.9 parity audit's § 0:
   `flush` is reached from `createSignal`'s closure only through
   `handleAsync`'s landing continuations, which run when the caller's own thenable
   or iterator settles, and its drain runs registrants' computations
   ([Decision 2026-09-10]). The `GlobalQueue._wireExternalSource` and
   `_externalUntrack` slots are third-party hooks, dispositioned **hook**, as in
   B1 § 1.3 there.
2. **Argument scope is a premise the census replays.** A scoped row is reachable
   only through `solid_dialect::argument_row`; `denies` and
   `primitive_performs_no_operation` never answer for it, so no caller that lacks
   the call site can read it as a flat denial (test
   `argument_rows_state_checkable_premises`).
3. **The `solid-js` pairing.** As for every rc.9 `solid-js` row, `solid-js`
   re-exports `getOwner`, `onCleanup`, `runWithOwner` and `untrack` from
   `@solidjs/signals`, so a `solid-js` import resolves its declaration into the
   signals archive while the `node`/`worker`/`deno` conditions run
   `solid-js`' own server bodies. Those were read (§ 1.3, § 2.3, § 3.3, § 4.3 of
   the rc.9 five-row audit for `creates`; re-read here for `reads` in § 1–§ 4). Row
   6 states its dependence on the signals archive as a delegate, which the census
   binds to one authenticated audited archive; rows 1 and 2, like their `creates`
   twins, do not, and rest on the rc.9 pairing.

## Method

**Read-site call graph.** The rc.9 parity audit's aid (§ 0.4 there) was re-run
independently: acorn, name-based, over-approximating (it links nested closures into
their enclosing function and any identifier to a same-named top-level function),
over `dist/prod/**`, `dist/observe/**` and `dist/dev.js` with `dist/dev-shared.js`.
It reports calls that resolve to the engine's read entry points (`read`,
`readNodeFast`, `serve`, `link`, `pendingCheckRead`, `latestRead`,
`getLatestValueComputed`) and value uses of them. The script is checked in beside
this audit
(`2026-09-30-solid-2-rc9-reads-rows-probes/callgraph.mjs`). Results, with `flush`
cut and no other cut:

| Root | prod | observe | dev (+ `dev-shared.js`) | read-entry calls |
| --- | --- | --- | --- | --- |
| `getOwner` | 1 fn | 1 | 1 | none |
| `cleanup` | 1 | 1 | 1 | none |
| `onCleanup` | 2 | 2 | 8 | none |
| `untrack` | 1 | 1 | 1 | none |
| `runWithOwner` | 1 | 1 | 6 | none |
| `createMemo` (the row it must equal) | 83 | 164 | 171 | none |
| `createSignal` | 85 | 166 | 173 | none |

Without the `flush` cut, `createMemo` and `createSignal` reach `run` and the
boundary evaluators through `handleAsync -> flush`, and no other path: the printed
path to `run` is `createSignal -> computed -> setupComputedNode -> recompute ->
handleAsync -> flush -> run`. The only value use of a read entry point is
`accessor`'s `read.bind`, which invokes nothing (and, in dev, `link` passed as a
value in `unlinkSubs`, `markNode` and `notifyStatus`, which link nodes and read no
value). It does not follow a call through a node's `_fn`/`_equals`/`_run`, a queued
function or a hook slot; those are the caller's, a drain, or a hook.

**Probes** (step 3 of the task; `…-probes/probe.mjs`,
`probe-solid-js.mjs`). Each primitive is run as the compute of a tracking memo `M`
created under a fresh root, as ADR 0163's synthesized veto does, and the memo's
own dependency links are inspected. The link fields are found at run time by
comparing a memo that read a signal with one that read nothing, so the build's
mangled names are its own answer, and a control (`s()` inside `M`) must be seen
tracked. All of rc.9's builds were run: signals `default`, `--conditions
development` and `observe`; `solid-js` `browser`, `browser`+`development`,
`browser`+`observe`, `node`, `node`+`development`, `node`+`observe`. Every probe
came out as stated (16/16 per signals build, 7/7 per `solid-js` client build, 2/2
per server build). The results are in each row's section.

### 1.5 The `solid-js` server bodies of § 1–§ 4

Under `node`, `worker` and `deno`, `solid-js@2.0.0-rc.9` runs `dist/server.js`,
`dist/server.dev.js` or `dist/server.observe.js` and not the signals archive, while a
`solid-js` import's declaration resolves into the signals archive. The four bodies
were extracted from all three server builds and are byte-identical across them:

| Name | `server.js` | `server.dev.js` | `server.observe.js` | sha256 (prefix) | Body |
| --- | --- | --- | --- | --- | --- |
| `getOwner` | 107-109 | 155-157 | 142-144 | `f21df202…` | `return currentOwner;` |
| `onCleanup` | 113-118 | 161-166 | 148-153 | `21cd3b7a…` | reads `currentOwner`, pushes `fn` onto its `_disposal` |
| `untrack` | 1560-1562 | 1661-1663 | 1634-1636 | `4849f0c6…` | `return fn();` |
| `runWithOwner` | 87-98 | 135-146 | 122-133 | `17977b59…` | sets `currentOwner`, `try { return fn(); } catch (error) { stampThrower(error, owner); throw error; } finally { restore }` |

`stampThrower` tests the error and does a `WeakMap` `has`/`set`; none reads a reactive
source. The only invocation in `untrack` and `runWithOwner` is `fn`, so the argument scope
of rows 3 and 4 is the server bodies' scope too. The server build has no tracking to
clear, and its `getOwner`, `onCleanup`, `untrack` and `runWithOwner` are its own bodies,
not `@solidjs/signals`', which is why § 1–§ 4 list them here and not in each section.

### 1. `getOwner` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`

- `default`: `dist/prod/core/owner.js:211-213`, bytes 8615..8663, `function
  getOwner() { return context; }`. `dev`: `dist/dev-shared.js` (131908..131950),
  the same statement. `observe`: `dist/observe/core/owner.js` (8635..8683),
  byte-identical to prod (`67fcbebd…`).
- Read-site closure: the function alone; no call. No invoking form.

**Sign-off.** `(@solidjs/signals@2.0.0-rc.9, getOwner, Reads)` denies that
`getOwner()` observes a reactive source. Probe: `getOwner()` in `M`: not tracked,
all three builds. **GRANT.**

### 2. `onCleanup` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`

- `default`/`observe`: `function onCleanup(e) { return cleanup(e); }`
  (`dist/prod/signals.js` 2598..2651, `dist/observe/signals.js` 2646..2699,
  byte-identical, `89ddda30…`). `cleanup` (`core/owner.js:219-223`) reads `context`
  and stores the argument on its disposal field; it calls nothing.
- `dev`: `dist/dev.js:2174-2203` (96698..97559) adds `getOwner()`, a flag test on
  the owner, `emitDiagnostic` + `reportDiagnostic` (console output and installed
  diagnostic listeners: a hook and the host), or `emitDiagnostic` + `throw`.
  `emitDiagnostic` builds an entry, reads the owner's path and `_devElement`, calls
  each listener the caller installed and queues a console footer. None is a read
  site (closure of 8 functions, above).
- The server body is `solid-js`' own (`server.js:113-118`), § 1.5.

**Sign-off.** Row 2 denies that `onCleanup(fn)` observes a reactive source. Probe:
`onCleanup(() => {})` in `M`: not tracked, all three builds. **GRANT.**

### 3. `runWithOwner` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`

- `default`/`observe`: `core/core.js` 87558..87770 / `observe/core/core.js`
  89058..89270 (byte-identical, `5c40363e…`): save `context` and `tracking`, set
  `context = owner` and `tracking = false`, `try { return fn(); } finally {
  restore }`. `dev`: `dev-shared.js` 287156..287864 adds a disposed-owner branch
  whose only effect is `reportDiagnostic(emitDiagnostic(…))`.
- The only invocation is `fn` (slot 1). Its reads are its author's.
- **Argument scope.** `invoked_slots: [1]`. Where slot 1 is a function literal
  inside the implementation under the walk, the walk disposes of its calls and
  forms with the frame; where it is rooted at a parameter of the censused export
  it is the caller's callable. Anything else (an accessor of a memo the call
  created, a module-level helper) is refused.

Probes (all three builds): `runWithOwner(owner, () => s())` and
`runWithOwner(null, () => 1)` leave `M` untracked; `runWithOwner(owner,
createdMemoAccessor)`, a lazy memo the call created passed by reference, *ran the
memo's compute once* (`count=1`): a read the row does not deny, which the scope
declines. **GRANT, argument-scoped.**

### 4. `untrack` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`

- `default`: `core/core.js:985-995`, 49018..49298; `dev`: `dev-shared.js`,
  242868..243344; `observe`: 50382..50662. Toggles the module `tracking` flag
  (dev: also `strictRead`) around `fn()`, or hands `fn` to the hook
  `GlobalQueue._externalUntrack` (mangled `Yt` in prod), which
  `enableExternalSource` installs from the caller's `config.untrack` (a
  third-party callable; disposition **hook**).
- The only invocation is `fn` (slot 0).
- **Argument scope.** `invoked_slots: [0]`, exactly as § 3.

Probes: `untrack(() => s())` and `untrack(() => 1)` leave `M` untracked;
`untrack(createdMemoAccessor)` by reference computed the created memo once
(`count=1`) and so did `untrack(() => createdMemoAccessor())`; the literal form is
the census's to refuse *by the walk* (the call of a created accessor has no
walkable declaration), the by-reference form is what the scope refuses.
**GRANT, argument-scoped.**

### 5. `createSignal` — `reads` — archive `@solidjs/signals@2.0.0-rc.9`

- `default`: `signals.js:67-75`, 2747..3027; `dev`: `dev.js:2209-2218`,
  97659..98008; `observe`: `observe/signals.js:69-78`, 2795..3109 (as prod plus
  `registerGraph(r, getOwner())`, a field stamp).
- `typeof first === "function"`: `computed(first, second)`, clear one config bit,
  `[accessor(node), setMemo.bind(null, node)]`. Otherwise `signal(first, second)`
  and `[accessor(node), setSignal.bind(null, node)]`. `accessor` is `read.bind`,
  which invokes nothing; `setMemo` and `setSignal` are only bound.
- The function path is `createMemo`'s: `computed` runs the caller's compute
  eagerly (unless `lazy`), through the closure `createMemo` `reads` is granted on
  (rc.9 parity audit group B § 1); the two closures were compared by the
  call-graph aid above (85 vs 83 functions in prod: `createSignal` adds `signal` and
  `linkFirewallChild`, neither a read site).
- The plain path is `signal`: builds the node literal from the options
  (`t?.equals`, `t?.ownedWrite`, `t?.unobserved`), no read.

Probes: `createSignal(0)`, `createSignal(0, { equals: false })` and
`createSignal(undefined)` leave `M` untracked in all three builds;
`createSignal(() => { inner++; return s(); })` also leaves `M` untracked (the
compute's read is the inner memo's) and ran its compute once at the call;
`createSignal(createdMemoAccessor)` ran the created memo's compute at the call
(`count=1`).

**Argument scope.** `callable_or_primitive_slots: [0]`: slot 0 is proved a
primitive by grammar, or (when it may be a function) is attributable as in § 3.
**GRANT, argument-scoped.** It is the delegate of row 6.

### 6. `createSignal` — `reads`, argument scope — archive `solid-js@2.0.0-rc.9`

The flat row was withheld on 2026-09-28
(`2026-09-28-solid-2-rc9-server-builds-createsignal-creatememo.md` § 3): under
hydration with `ssrSource` `client` or `hybrid`, the browser builds hand the
signals `createMemo` an archive-authored compute that reads `hydrated()`, the
signal `withHydrationGate` created on the same call. This row narrows it.

**The bodies.** `createSignal` is `solid-js`' own declaration
(`types/client/hydration.d.ts`), not a re-export. The three client builds define
`const createSignal = (...args) => (_createSignal || createSignal$1)(...args)`
(`<js>/dist/solid.js:773-775`, `solid.dev.js`, `solid.observe.js`, byte-identical,
`96473897…`), where `_createSignal` is `hydratedCreateSignal` once
`enableHydration()` ran and nothing else (`solid.js:105-106,705,774`;
`hydratedCreateSignal` is byte-identical in the three, `a185bff0…`,
`solid.js:602`, bytes 18133..18329, `solid.dev.js` 18971..19167, `solid.observe.js`
18453..18649):

    function hydratedCreateSignal(fn, second) {
      if (typeof fn !== "function" || !sharedConfig.hydrating) return createSignal$1(fn, second);
      return hydrateSignalLike(createSignal$1, fn, second);
    }

So a first argument that is not a function reaches `createSignal$1` (the signals
export, row 5) and nothing else, whether or not the runtime is hydrating.
`hydrateSignalLike` (`c14803d9…`, `solid.js:571`) is reached only when `fn` is a
function; it is where `withHydrationGate` and `hydrated()` live. The three server
builds define `function createSignal(first, second)` themselves
(`server.js:302-320`, `server.observe.js:346-364`, identical `38028a6c…`;
`server.dev.js:371-391`, `6002fc5a…`, differing by two `warnServerWrite` calls that
sit only in the setter): `typeof first === "function"` builds the server memo
(`createMemo(prev => first(prev), opts)`, the reach the 2026-09-28 audit read);
otherwise `[() => first, v => { … }]`, two closures, no read and no call.

**The row.** `primitive_slots: [0]`, delegate `(@solidjs/signals, createSignal,
Reads)`. A call whose slot 0 the producer states primitive by grammar
(`argumentsPrimitiveSyntax`, ADR 0168's producer fact) takes none of the
withheld paths in any of the six builds. A zero-argument call, a spread, and any
slot not stated primitive are refused. Options (slot 1) are read by `signal`
only as property reads of the caller's object.

**The delegate.** `createSignal$1` is the signals export, so the row is sound only
beside the audited signals archive; `census_delegated_denials` binds the one
authenticated `@solidjs/signals` snapshot in the closure to row 5, and checks
row 5's scope on this call's own arguments (row 6 forwards `fn` and `second`
unchanged, so slot 0 is the same value).

Probes, every client build (browser, +development, +observe): `createSignal(5)`
leaves `M` untracked with hydration off and on, and reads back `5` with `{
ssrSource: "client" }` under `sharedConfig.hydrating = true`; and the contrast that
keeps the scope honest: `createSignal(fn, { ssrSource: "client" })` ran `fn` at the
call when not hydrating (`1`) and did *not* when hydrating (`0`), because the
archive's own compute read the gate it had created first. The server builds return
`5` from `createSignal(5)`. **GRANT, argument-scoped and narrowing the flat row,
which stays withheld.**

### 7. `useContext` — `reads` — archive `solid-js@2.0.0-rc.9`

- Client (`<js>/dist/solid.js:18-20` and its dev and observe twins, `03f0fc70…`):
  `return getContext(context);`, `@solidjs/signals`' `getContext`
  (`<sig>/dist/prod/core/context.js:28-39`, `observe/core/context.js:28-39`,
  `dist/dev.js:272-283`): `getOwner()`, `t.ze[e.id]` (a plain read of the owner's
  context object, built by `setContext` as `{ ...r.ze, [id]: value }`), the
  default, or a throw of `NoOwnerError`/`ContextNotFoundError`.
- Server (`server.js:1617-1627`, twins `3cbc46d0…`): the server's own
  `getContext` (`:119-126`, the same lookup) and, on `ContextNotFoundError`,
  `serverComponentContextError`, which reads a context entry and builds an
  `Error`.
- A context value is not a reactive source, and neither the map nor `context.id`
  is one (`context` is the caller's object; a getter on it is the caller's code).
  What a caller does with the value is the caller's.

Probes: `useContext(ctx)` in `M` leaves it untracked in every client build and
returns the default; the server builds return it. **GRANT.**

## 8. The by-reference finding, which the scope exists for

`createMemo`'s granted row (and the other flat rc.9 `reads` rows) is flat: it denies the
export's own code and leaves a callable *passed by reference* unattributed. The
probes show the reach: `untrack(lazyMemo)`, `runWithOwner(owner, lazyMemo)` and
`createSignal(lazyMemo)`, with `lazyMemo` a memo the call created, each computed it
once. No flat row this ADR adds is exposed to that, and the flat rows already
shipped are not changed (backlog note).

## 9. Read and refused

- **`solid-js` `createMemo`.** `hydratedCreateMemo` (`solid.js:596-601`) enters
  `hydrateSignalLike` whenever `sharedConfig.hydrating` and `!options?.transparent`.
  With no `ssrSource` the first two branches fall through to
  `hydrateSignalFromAsyncIterable` and `readSerializedOrCompute`, whose
  reactive reads were not walked, and a row would need the options argument
  proved absent or transparent, which needs a "no spread" producer fact this ADR
  does not add. Stays refused.
- **`solid-js` `createComponent`.** The client prod body is `untrack(() =>
  Comp(props || {}))` and clean; the dev and observe bodies wrap it in
  `createRoot(…, { transparent: true })` (`solid.dev.js`, `solid.observe.js`
  `observedComponent`) and read `getOwner()`; the server dev/observe bodies use
  `runWithOwner(createComponentOwner(…), …)`. Sound in outline, but its delegates
  are argument-scoped rows on a callable the census must attribute through a
  wrapper arrow; not written here. Stays refused.
- **`createRoot`.** A `solid-js` import resolves to `solid-js`' own declaration
  (`types/client/hydration.d.ts:372`). Client: `(...args) => (_createRoot ||
  createRoot$1)(...args)`, where `hydratedCreateRoot` wraps `init` in an arrow that
  calls `markTopLevelSnapshotScope()` when hydrating; server: `createOwner(options)`
  then `runWithOwner(owner, () => init(() => disposeOwner(owner)))`. Sound in outline
  (`init` is the only invocation), but a row would need a `reads` row for
  `createOwner` and a delegate that maps `createRoot`'s slot 0 onto `runWithOwner`'s
  slot 1, and this ADR's delegates check the delegating call's own arguments
  unmapped. The signals archive's own `createRoot` is the same two statements and would
  take row 3's scope, but no `solid-js` import resolves into it. Stays refused; the
  census still names it twice at rc.9 after this change.

## 10. Citations

Every citation is a range the rc.9 citation test re-reads; the rows for `getOwner`,
`onCleanup`, `runWithOwner`, `untrack`, `createSignal` (signals) and `useContext`
cite the same definitions as their `creates` twins (their bodies are what both
readings are about). New slices: `solid-js` `createSignal` in
`dist/server.js`, `dist/server.dev.js` and `dist/server.observe.js`, checked in
under `rust/crates/solid-dialect/audited-slices/solid-v2/rc9/solid-js/`.

| Package | Export | `archive_path` | `start_byte` | `end_byte` | `slice_sha256` |
| --- | --- | --- | --- | --- | --- |
| `@solidjs/signals` | `createSignal` | `dist/prod/signals.js` | 2747 | 3027 | `ce6ade13e9e77463067c7bac3727622e0ac32bd8e9bad801501a28365b9da388` |
| `@solidjs/signals` | `createSignal` | `dist/dev.js` | 97659 | 98008 | `d1292c1a9d7a916d932b8e2ae27b22c592daf612cc51ddf7848ecf64f63cd504` |
| `@solidjs/signals` | `createSignal` | `dist/observe/signals.js` | 2795 | 3109 | `0366fccdf03f70ca98cd1678d74800abeed9109c5499c65c626ee6868e6f565f` |
| `@solidjs/signals` | `getOwner` | `dist/prod/core/owner.js` | 8615 | 8663 | `67fcbebd02b9e57fe095ba4af938b3b4b27e52e9ecda46862ddce141f1e4c9e2` |
| `@solidjs/signals` | `getOwner` | `dist/dev-shared.js` | 131908 | 131950 | `e8cb95b765807fa14a97032551f4bbced263cc3d7837fc1258f156ffc71ba8bb` |
| `@solidjs/signals` | `getOwner` | `dist/observe/core/owner.js` | 8635 | 8683 | `67fcbebd02b9e57fe095ba4af938b3b4b27e52e9ecda46862ddce141f1e4c9e2` |
| `@solidjs/signals` | `onCleanup` | `dist/prod/signals.js` | 2598 | 2651 | `89ddda3041ae80177d8bea1730aeb504ddb62f57d37956e3cfea6232f0f8925b` |
| `@solidjs/signals` | `onCleanup` | `dist/dev.js` | 96698 | 97559 | `5f8d40b1c3165f17bc00846b5063eb55f8bdd0a6b49dceba4a60f5aeb6570e44` |
| `@solidjs/signals` | `onCleanup` | `dist/observe/signals.js` | 2646 | 2699 | `89ddda3041ae80177d8bea1730aeb504ddb62f57d37956e3cfea6232f0f8925b` |
| `@solidjs/signals` | `runWithOwner` | `dist/prod/core/core.js` | 87558 | 87770 | `5c40363ecc6eaf66378b57e0c387103fe67f51706d30dab3cb041fd10f8af3e5` |
| `@solidjs/signals` | `runWithOwner` | `dist/dev-shared.js` | 287156 | 287864 | `332a218ceec024a1d0c3464213b7d043d4a902c529b5f02a7d6ae0467070a11c` |
| `@solidjs/signals` | `runWithOwner` | `dist/observe/core/core.js` | 89058 | 89270 | `5c40363ecc6eaf66378b57e0c387103fe67f51706d30dab3cb041fd10f8af3e5` |
| `@solidjs/signals` | `untrack` | `dist/prod/core/core.js` | 49018 | 49298 | `28c2d9f3861b28ad08edaeb66a6d8f2caa5530d6e7cff226afda0a9a7b5d912a` |
| `@solidjs/signals` | `untrack` | `dist/dev-shared.js` | 242868 | 243344 | `2a929c2683ae93a3820bf2b4bf1a4455b41c6a928880eaa480a6820fe43a1852` |
| `@solidjs/signals` | `untrack` | `dist/observe/core/core.js` | 50382 | 50662 | `01672a8cba1c1d7a8800b0effde85a96cffd51ac0bb7025b40b208f805e98773` |
| `solid-js` | `createSignal` | `dist/solid.js` | 23906 | 23997 | `96473897517769f0074a40f4e1657dfb1d580a915549f978b7651ab38f3ec8a7` |
| `solid-js` | `createSignal` | `dist/solid.dev.js` | 25602 | 25693 | `96473897517769f0074a40f4e1657dfb1d580a915549f978b7651ab38f3ec8a7` |
| `solid-js` | `createSignal` | `dist/solid.observe.js` | 24226 | 24317 | `96473897517769f0074a40f4e1657dfb1d580a915549f978b7651ab38f3ec8a7` |
| `solid-js` | `createSignal` | `dist/server.js` | 8763 | 9388 | `38028a6c27dec146a1e4eb81e2e9bdfce1f1e90ce2d2701fe21c1d460148cf39` |
| `solid-js` | `createSignal` | `dist/server.dev.js` | 12317 | 13006 | `6002fc5a126d938f9e23b7d284d707d2448bf56a65da3e428bb02167efe9c979` |
| `solid-js` | `createSignal` | `dist/server.observe.js` | 10004 | 10629 | `38028a6c27dec146a1e4eb81e2e9bdfce1f1e90ce2d2701fe21c1d460148cf39` |
| `solid-js` | `useContext` | `dist/solid.js` | 1635 | 1697 | `03f0fc70f94b38bb7a4b6720e59c06f7cf491dc297ffe4e5918960d5b9621618` |
| `solid-js` | `useContext` | `dist/solid.dev.js` | 1664 | 1726 | `03f0fc70f94b38bb7a4b6720e59c06f7cf491dc297ffe4e5918960d5b9621618` |
| `solid-js` | `useContext` | `dist/solid.observe.js` | 1657 | 1719 | `03f0fc70f94b38bb7a4b6720e59c06f7cf491dc297ffe4e5918960d5b9621618` |
| `solid-js` | `useContext` | `dist/server.js` | 50326 | 50582 | `3cbc46d0ef0841dd0dab0674ca61438056a050d6d118d44c094404c42ad8dd5f` |
| `solid-js` | `useContext` | `dist/server.dev.js` | 54707 | 54963 | `3cbc46d0ef0841dd0dab0674ca61438056a050d6d118d44c094404c42ad8dd5f` |
| `solid-js` | `useContext` | `dist/server.observe.js` | 52445 | 52701 | `3cbc46d0ef0841dd0dab0674ca61438056a050d6d118d44c094404c42ad8dd5f` |

## 11. Residual approximations

- The call graphs are name-based over-approximations; dispatch through node fields,
  hook slots and caller values is dispositioned by hand.
- `useContext`'s `getContext` is the signals archive's, and row 7 is flat; it rests
  on the rc.9 `solid-js`/`@solidjs/signals` pairing, as the `creates` twin does.
- Row 6 does not cite `hydratedCreateSignal` or `hydrateSignalLike` as slices (the
  citation test requires a range that begins at the export's own definition); their
  digests are recorded in § 6.
- The argument premises rest on the producer's `arguments_primitive_syntax`,
  `argument_parameters` and `argument_callables` facts; a producer that states less
  refuses more.
