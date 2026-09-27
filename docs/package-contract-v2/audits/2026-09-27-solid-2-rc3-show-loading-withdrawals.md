# Audit: `solid-js@2.0.0-rc.3` — `Show` and `Loading` rows withdrawn on rc.3's own bytes

Date: 2026-09-27. Status: **for the repository owner's review**, as the audits
it follows were. It withdraws rows that the negative table
(`rust/crates/solid-dialect/src/solid_2.rs`, `NEGATIVE_ROWS`) carried for
`solid-js@2.0.0-rc.3`. Each of them rested only on a summary in
`pkg/contracts/bundled/solid-v2/solid-js.json`, and each is contradicted by
rc.3's own runtime bytes.

**Why it exists.** Reading the same exports on `solid-js@2.0.0-rc.9`
(`2026-09-27-solid-2-rc9-core-and-web-negative-rows.md`) found reads and
creates that the rc.3 summaries do not publish. The rc.3 bodies
involved are byte-identical to rc.9's, or reach the same operation by the same
path. So the rows were re-read on rc.3, and every row that fails is withdrawn
here, one row per commit.

The rows rest on summaries alone:

- The `reads` rows were admitted on 2026-09-10 with "nothing new was read"
  (`phase21/2026-09-10-reads-negative-rows-audit-worksheet.md` § 5).
- `solid-js.json` captures `browser/development` only, which is the
  approximation `NEGATIVE_ROWS`' doc comment already names.

A withdrawal is the table's answer to a guarded reach, not a qualification of
it: "a guarded reach still counts, and a flat row must therefore withhold"
(`semantic-model.md` § creates).

**Identity.** `solid-js@2.0.0-rc.3`,
`sha512-pmW6bRoTvfp/rN4jN7JmLvSaoIpFt7wm0Hi3j508S/smuJqUbRg3dQEjOPTkAwHW+McYnXrMG7cJ4AMNpLevtQ==`,
`package.json` sha256
`e703e7986516ac05ee91fdd64897c2d150aea948cb5bf77eae8673da5008ee4b`. On
2026-09-27 the tarball was re-downloaded with `npm pack solid-js@2.0.0-rc.3`,
and its SHA-512 is that integrity. Beside it, `npm pack
@solidjs/signals@2.0.0-rc.3` produced
`sha512-/yPhTf3xS1FRR4MX8kTYCd4MjsFxzwkO+KyOTfbu35lTEiaJ4Fxy+JL91XonDzt31GV1mYaZ9CGD2TQIzvXuNA==`,
the rc.3 signals archive. The whole-file sha256 of every file cited below
equals `benchmarks/package-contract-v2/phase0/rc3/{solid-js,solidjs-signals}/files.json`:

| File | sha256 |
| --- | --- |
| `solid-js` `dist/solid.js` | `14af2d696eb0669c64973874601f691737aa1df359fced6dec55a523f34cfa1b` |
| `solid-js` `dist/dev.js` | `dfc362391cbc0b069cef8b8d0d72c99d34310231a76fd66ef615533424d3ac18` |
| `solid-js` `dist/solid.cjs` | `d155966bc29d2bf46e3cb32c8839d885933a50b60c28c6d8abd70a7ac1147333` |
| `solid-js` `dist/dev.cjs` | `4ca1b958df30ef4b0fa9cfd17206293073846d33147ad164208805dafde22e51` |
| `solid-js` `dist/server.js` | `63269da73b61b71fd775ef811f8ab88417c6ea6dda2de1e6f3c10d86b66fc8a8` |
| `solid-js` `dist/server.cjs` | `2e2ed5833323d48a43454b89f03de9f31346a3549a9934d8e67b3b9fe4f231a7` |
| `@solidjs/signals` `dist/prod/core/core.js` | `1726b40ebf79cf15b8d09ce2078a78a6a8ca71bf48e4b3ba12880736ae281a46` |
| `@solidjs/signals` `dist/dev.js` | `cc68ed0f0c5de86411555af407ac7acf4d1c10206f24bab4e1793c22553f1a79` |
| `@solidjs/signals` `dist/node.cjs` | `bc0e35d32add395dc1c4dc3d6cd0fb4ea4a19bd582d68de3b44c708bb4b75c1c` |

Slices below use the 2026-09-23 convention: from the `function` token through
the closing `}`, inclusive.

**What a withdrawal moves.**

- A withdrawn row stops answering `primitive_performs_no_operation`, so the
  census stops using it as a terminator.
- A withdrawn `creates` row also stops answering
  `some_audit_denies_primitive`, so the generator stops proposing a closed
  `creates` on its strength.
- `reads` is not proposable (`ClaimDomain::PROPOSABLE`) and has no reviewed
  observation, so a withdrawn `reads` row moves no certification today.
- The coverage compare for each withdrawal is reported in its commit message.

---

## 1. `Show` — `reads` — archive `solid-js@2.0.0-rc.3` — **WITHDRAWN**

### 1.1 The rc.3 body, client builds

`dist/solid.js:1153-1174` (bytes `36715..37409`, slice sha256 `5e3245ae…1d37`):

```js
function Show(props) {
  const keyed = props.keyed;
  const conditionValue = createMemo$1(() => props.when, undefined);
  const condition = keyed ? conditionValue : createMemo$1(conditionValue, {
    equals: (a, b) => !a === !b,
    sync: true
  });
  return createMemo$1(() => {
    const c = condition();
    ...
  }, {
    sync: true
  });
}
```

The same body appears in every client build:

- `dist/dev.js:1180-1205` (`38448..39215`, `faf07392…54a3`) adds only `name`
  options and the `"<Show>"` untrack label.
- `dist/solid.cjs:1154-1175` and `dist/dev.cjs:1181-1206` are the CommonJS
  twins.

`createMemo$1` is `@solidjs/signals`' `createMemo`, `accessor(computed(e, t))`
(`dist/prod/signals.js:75-77`, `dist/dev.js:5730-5732`,
`dist/node.cjs:4434-4436`).

### 1.2 The memos compute on the call's own stack

`computed` ends in `setupComputedNode`, which runs `!t?.lazy && recompute(e,
true)` in every rc.3 signals build: `dist/prod/core/core.js:518`,
`dist/dev.js:3749`, `dist/node.cjs:2799`. `Show` passes no `lazy`. So each of
the three `createMemo$1` calls runs its compute before `Show` returns:

| Site | Compute | What it reads |
| --- | --- | --- |
| first `createMemo$1` | `() => props.when` | a property of the caller's `props`, a parameter-rooted receiver: the **caller's** read (§ reads [Decision 2026-09-10]) |
| second `createMemo$1` (unkeyed) | `conditionValue` itself | **`conditionValue()` — the accessor of the memo `Show` created one line earlier** |
| returned `createMemo$1` | `Show`'s own arrow | **`condition()` — a memo `Show` created**; then `props.children` / `props.fallback` (caller's) and, for a function child, `untrack(() => child(...))` (a caller-supplied callable) |

Two observations of a reactive source's current value are performed by code
`Show` authored, on sources `Show` created, during the invocation itself:

- `conditionValue()` inside the second memo;
- `condition()` inside the returned memo, which is `conditionValue()` itself
  when the call is keyed.

The accessor handed to an unkeyed function child is also `Show`'s own arrow:
`untrack(condition)` and `conditionValue()`. It runs when the caller's child
invokes it, so it is the weaker case, and the verdict does not rest on it.

`semantic-model.md` § reads:

> A read of a source the export *created* is unaffected and still counts.

That is exactly the reason `solid-js` `createEffect` `reads` is already
withheld. Neither of the two reads is performed by a caller-supplied callable,
and neither is a property access on a caller-supplied receiver. The summary's
`reads: []` is therefore a false denial.

### 1.3 The server builds

`dist/server.js:1853-1872` (and `server.cjs:1854-1873`, the same slice
`b1c7dfb9…24bd`) calls the server's own `createMemo`. The returned memo is
`sync: true`, so it is `createSyncMemo`, whose `pull()` runs eagerly. Its
compute runs `const when = conditionValue();`, a read of the memo `Show`
created. So the server builds read too. The client builds alone decide the
verdict.

### 1.4 Verdict

**WITHDRAWN.** `Show` reads the memos it creates, at its own call event, in
every rc.3 build. The row `(solid-js, 2.0.0-rc.3, Show, Reads)` is removed from
`NEGATIVE_ROWS`, and it is listed in `WITHHELD` and in `IMPLEMENTATION_AUDITED`
under this section.

---

## 2. `Show` — `creates` — archive `solid-js@2.0.0-rc.3` — **WITHDRAWN**

### 2.1 The client builds perform no `create`

The client `Show` (§ 1.1) calls only `@solidjs/signals`' `createMemo` and
`untrack`, besides the caller's children. The rc.3 signals archive-wide
host-boundary census (`2026-09-04-solid-2-rc3-core-primitives-creates.md`
§ 1.5) finds no handle to a document or a server runtime in the archive. The
browser side is clean, and it is the side `solid-js.json` captured.

### 2.2 The server builds reach `ctx.serialize`

`dist/server.js:1853-1872` (`58768..59264`, slice `b1c7dfb9…24bd`, and the
same slice in `server.cjs:1854-1873`):

```js
function Show(props) {
  const conditionValue = createMemo(() => props.when);
  ...
}
```

This is the server's own `createMemo`, not `@solidjs/signals'`. Its call table:

| Callee | Site | Reach | Disposition | What it does |
| --- | --- | --- | --- | --- |
| `createMemo(() => props.when)` | `server.js:1854` → `:306-395` | always | local | no `sync`, no `lazy`, no `ssrSource`, so `update()` runs at once (`:378-380`) |
| `createOwner(options)` | `:311` → `:44-` | always | local | `id` is `nextChildIdFor(parent, true)` (`:26-33`) whenever the parent owner has an id, which it does under a hydrating render |
| `update()` → `run()` | `:341-360` | always | local, then **caller value** | the compute returns `props.when`, a value the caller supplied, which may be a thenable |
| `processResult(comp, result, owner, ctx, …)` | `:349` → `:500-803` | always | local | when `result` is a thenable and no slot settled it: `const serializes = !!(ctx?.async && ctx.serialize && id && !noHydrate)` (`:555`), then **`if (serializes) ctx.serialize(id, deferred.promise, deferStream)`** (`:558`). An async iterable takes the sibling arm to `ctx.serialize` at `:699`/`:760` |

`ctx` is `sharedConfig.context`. Under `@solidjs/web@2.0.0-rc.3`'s
`renderToStream` that is the per-request context (`web` `dist/server.js:1325`,
`async: true`). Its `serialize` (`:1383-1397`) adds a thenable to
`blockingPromises` or batches it, and then `serializer.write(id, p)` writes it
into the response stream. `semantic-model.md` § creates **[Decision
2026-09-04]** settles that this is a `create`:

- it is the export's own act;
- the memo's pending result is an `async-computation` landing in the response
  `stream`;
- the hydration serializer acts on it after the call returns.

Here the memo is `Show`'s own. The caller supplies a value, not a callable.

**Type-correct.** `Show<T>(props: { when: T | undefined | null | false; … })`
(`types/client/flow.d.ts:90`, `:96`, `:102`, `:108`; `.` has one `types` entry
for every condition) leaves `T` unconstrained. So `<Show when={promise}>`
type-checks, and a reader of the rc.9 twin ran `tsc` to confirm it. Nothing
in `processResult`'s thenable arm needs an option: the `ssrSource` guard the
2026-09-04 audit lists belongs to `serverEffect` (`createEffect`), not to
`createMemo`.

The guard is `node`/`worker`/`deno` ∧ `renderToStream` (`ctx.async`) ∧ an
owner with an id ∧ no `NoHydrate` ancestor ∧ a thenable or async-iterable
`when`. It is a guarded reach, and a flat row has nowhere to put the guard.

### 2.3 Verdict

**WITHDRAWN.** The row `(solid-js, 2.0.0-rc.3, Show, Creates)` is removed
from `NEGATIVE_ROWS`, and it is listed in `WITHHELD` and in
`IMPLEMENTATION_AUDITED` under this section. A browser-scoped row, as
`createSignal` has, would need `@solidjs/signals`' `createMemo` `creates` as a
delegate. No audited archive carries that row, so none is added.
