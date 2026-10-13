# Review: where do `solid-js` 2.0.0-rc.1, rc.2 and rc.4-rc.8 sit between rc.3 and rc.9?

Date: 2026-09-26. Status: **for the repository owner's review**. This is a
review, not a dialect change: no row, release classification or dialect
answer was changed. The lead decides any classification change with the
owner.

**Why it exists.** Commit `a55a24d9` classifies the installed `solid-js` 2.x
release (`rust/crates/solid-dialect/src/solid_2/releases.rs`): rc.0 and rc.3
are audited, rc.9 is reviewed with gaps and analysed under `Solid2::RC9`, and
every other 2.x release gets the SC9014 "not reviewed" notice under
`Solid2::AUDITED`. The module comment says of rc.4-rc.8 that the
`Store<T> = T` change "is in the rc.7-era *source*, not in any published
rc.4-rc.8 bytes anyone here read". This review reads those bytes. It walks the
rc.9 review
([`2026-09-26-solid-2-rc9-vocabulary-review.md`](2026-09-26-solid-2-rc9-vocabulary-review.md),
"the rc.9 review") item by item, and its § 3 inventory.

Every claim is tagged **[M]** (measured: cited to a file and line, a digest,
or a command run on the named bytes) or **[E]** (estimated: inferred from code
reading or from a neighbouring version, not executed).

## 0. Result

### 0.1 Table

"Triple" means `solid-js`, `@solidjs/signals` and `@solidjs/web` at the same
rc. That is the lowest version each `^2.0.0-rc.N` range admits (§ 1.2). § 5
covers mixed installs, which a fresh install produces today.

| release | B1 `Store<T> = T` | B2 `dynamic` `static` | B3 `omit` predicate | B4 `until` | N2 broken re-exports | rc.9 packaging | verdict |
| --- | --- | --- | --- | --- | --- | --- | --- |
| rc.1 | no | no | no | no | no | no (rc.3 layout) | **behaves as rc.3** |
| rc.2 | no | no | no | no | no | no (rc.3 layout) | **behaves as rc.3** |
| rc.4 | no | no | no | no | no | no | **new difference**: unmodelled patch-channel callback exports (§ 4.2) |
| rc.5 | no | no | no | **yes** | no | partial (`./types/*` subpath gone) | **mixed**: B4, plus the rc.4 difference |
| rc.6 | no | no | no | **yes** | no | partial | **mixed**: B4 and N1, plus the rc.4 difference |
| rc.7 | **yes** | no (`DynamicOptions` holds only `deferStream`) | no | **yes** | no | partial (bundle renames, `server.dev.js`) | **mixed**: B1, B4, N1 |
| rc.8 | **yes** | no (same as rc.7) | no | **yes** | no | **yes**, except `solid-js/internal` | **mixed**: B1, B4, N1, N4 and the packaging changes |

Every entry is [M] (§ 3). In the triple, rc.1-rc.8 agree with rc.3 on every
timing, tracking, ownership and write-guard premise in the review's § 3.3-3.4
(§ 2.4: 37 probes, dev and prod, all ten releases). Two guards do change, and
no rule relies on either:

- rc.8 adds N4, `FLUSH_IN_ACTION`;
- N3, the store setter in a root body, changes only on rc.9.

### 0.2 Recommendation

| release | recommended classification | why |
| --- | --- | --- |
| rc.1 | `Solid2::AUDITED`, no notice | Between the two audited releases, and equal to one of them on every dialect premise. Signals declarations equal rc.3's except `Owner`/`Dev`/diagnostic internals. The web helper bodies equal rc.0's (§ 4.1). [M] |
| rc.2 | `Solid2::AUDITED`, no notice | The signals declared surface is identical to rc.3's: 106/106 exports. 486 of 492 signals dev slices are byte-identical; the other 5 are internal optimistic/lane slices. [M] |
| rc.4 | `Solid2::AUDITED`. The notice can go only if the owner rules on the patch channel | Every vocabulary answer is rc.3's. rc.4 adds `registerPatch`, `registerRowOps` and `registerSlotPatch` (`solid-js`), and `installListDriver` and `driveList` (`@solidjs/web`). All five take callbacks and are neither modelled nor excluded, the same completeness failure B4 was. Either exclude them as renderer-internal and drop the notice, or keep a reviewed-with-gaps notice that names them. [M facts, E recommendation] |
| rc.5 | as rc.4 | B4 is already modelled unconditionally (`2990ec25`). `until`'s body is byte-identical from rc.5 to rc.9 and its probes agree (§ 3.4). [M] |
| rc.6 | as rc.4 | As rc.5, plus N1. N1 needs no change. `@solidjs/signals@2.0.0-rc.6` is already an audited archive for negative rows. [M] |
| rc.7 | **`Solid2::RC9`**, reviewed with one gap (no negative rows) | B1 is present: `tsc` stops reporting root writes (TS2540 gone) while the runtime still drops them. Under `AUDITED` the checker reports nobody: measured in § 3.1. B2 and B3 are absent, and the rc.9 models for them are additive (§ 6). The rc.9 gaps "omit predicate" and "broken re-exports" do not apply. [M] |
| rc.8 | **`Solid2::RC9`**, reviewed with one gap (no negative rows) | As rc.7. The packaging changes and N4 need no vocabulary change. [M] |

**One finding outranks the table (§ 5) [M].** The Store typing (B1) belongs to
`@solidjs/signals`, but the classification reads `solid-js`. Every
`solid-js@2.0.0-rc.N` depends on `@solidjs/signals: ^2.0.0-rc.N`, so a fresh
install today resolves `@solidjs/signals@2.0.0-rc.9`, including for the
audited `solid-js@2.0.0-rc.3`. Measured on such a fresh rc.3 install: `tsc`
accepts `store.a = 2`, and the checker reports `status: certified` with no
finding and no notice. The runtime drops that write. Per-release mapping alone
cannot close this. `StoreRootTyping` has to follow the resolved
`@solidjs/signals` version: `Readonly` through rc.6, `Mutable` from rc.7.

## 1. The bytes compared

### 1.1 Tarballs and integrity [M]

All 30 tarballs (rc.0-rc.9 of the three packages) were fetched with
`npm pack <name>@<version>` from `https://registry.npmjs.org/` (npm 11.6.2,
Node 24.11.1). They were unpacked under the session scratchpad
`rc-releases/x/<pkg>-rcN/`. For each one, the tarball's own
`sha512-<base64(sha512(tgz))>` was computed and compared with
`npm view <name>@<version> dist.integrity`. **All 30 match.**

The rc.3 identity check: all three rc.3 unpacks match
`benchmarks/package-contract-v2/phase0/rc3/*/files.json` by sha256, file for
file, with no extras. The counts are 43/43 (`solid-js`), 107/107
(`@solidjs/signals`) and 103/103 (`@solidjs/web`). The rc.6 signals
`package.json` sha256 is `de11cde1…c163c`, the value
`2026-09-25-solid-2-rc6-signals-negative-rows.md` pins. The rc.9 integrities
equal those the rc.9 review copied from the lockfile.

| release | `solid-js` integrity | `@solidjs/signals` integrity | `@solidjs/web` integrity |
| --- | --- | --- | --- |
| rc.1 | `sha512-UD+UfqfiuuOTaDw01YeT+LwsYJC2ilTlMfs6h8EC8FFLmZD0ZjeZIoJXdZEo9uMzIof2tu0Rfh3dnzI7FAmuJQ==` | `sha512-KQpgUbn9xuzFaXupwej9MvUnQV+H6wcCgvrERf+dygco3T9JWP9S02g/UoYwwmJ6Vh+LE1b82ZlSHYR2Bd1O8A==` | `sha512-wLuxGtQUxaFfqxqhIUJGGSZB/upd3GzokQRFJKvO7biJGNZLAws+eanMqi0kK2Amg+MZZ7aVyPPKydf8mzdhkg==` |
| rc.2 | `sha512-1C++20cho5f+omXbX6MIl+VrBZ+by6QkWpCtpbVmmUR80EGgH4irqV0UdP88GAYHx1U+FzpCcHxIGoA9raCyQA==` | `sha512-nJyXdBZJaYGqtDuQt8csE9LY9oNZV2fZfPRmJHQT9QjQEuHKQ1OaZScZ8YFaeATrnavzys4Ws95LnUdv8pCidQ==` | `sha512-JazuW9NCI+kcpZXRq9J/DAGPmMd0pEWr3kuXFmPZV9tOviiTOHDXOf56pjbUFVDZd+120AKoe/ULmYdSDpJmsA==` |
| rc.4 | `sha512-hSkDmtduesjFtvzjNyqLphrqiSvhSD5+njkPQUR+9YEoR60MMWtuv36SbLL4OucI0Ronof3Jc+AsUp0oWffwMg==` | `sha512-l7P0g8+2pnNscaIPOGDMhv0boaidGAsvR8QN66JySIWNMvVnuq2ayv7ZnvP+IR00DJC+RqCNnnTG/UCdxf+h8g==` | `sha512-acM9W0FByf0aJDMG5kMbHr0BLTjvFIg8lxwLwHzsu3ybWSmsP1FiOOs2RHNItve/AiX+MGR9CQBCazmnfPykEA==` |
| rc.5 | `sha512-AI9ndOlUtXXeFf0/MUADM5HfMYwXtxfitIayOkjBCZl67ZyD4ct5+4kqv0EF2xeyIrUwlsrFTuKpHs54BNA6iw==` | `sha512-Ks+97LbyN2vYqPEHWpy2YY+MYgoSCge0kHOUFN/Bzdz1Ex/M9Is1HIyBZGBVhf5y35394List5WWlU44yEnm4w==` | `sha512-0Q9QNJXgXDTl4dH8Nr972XdL3ET+GBV/UlxhBG1ZV7U54/PP2yhTJggWxubJNqfeWel8p/W8laNgtS3nLiZpbw==` |
| rc.6 | `sha512-Z/M8s9ypLBf+6Bl3AAb5upgkYCl73HkpM+UxdUJ5uFGctTwpOQVCM9Gpj3Mjbiaki270HHUfA3Z0Lyn4w+fDtg==` | `sha512-lPqwZNLPq1Z9CBvgXkMvi1ZFr5OHUiFNz1X40+yehszDWEbJkneZx7BGKIe9eMT/AN1NSL+PMjOiMyZaqVB2xw==` | `sha512-JgQ2NCjygQpZizZrVjcwPqH3dhIZQZoMRGoGSC6+Tr622cvbuXthDC3pKdNMsjCKCVp19UbT0kkPX15FOdT0pw==` |
| rc.7 | `sha512-3APJcwGbJ3YzXzPXwl0R3cAiogXLacXdXSFasdE2uw1Gzj5xqDW/0bJu4fs75KK5WzXg+JfpkcsxBjTxkbnLQQ==` | `sha512-JY0OJ5nGeqxGKOAqCtuoHkFBkaWQYxmf+yBQE8vw/o9C7yUF+Kan9PwcrL0ZHR6+MgyixBUtTfBigwou5hwW5w==` | `sha512-qsKKWR4PzzPw8ZGFR0Oc2776G1ONMMBKPJWO3Opf3cWHoczgfhknW6NED0WtdCuR6T8usFTWCZgMM5Cc0h14Dg==` |
| rc.8 | `sha512-0DwASKxvwXWAxCiea71cN3bmsyBIOUGmmg/pjSfhxPg+2pBVCh/ZKmwLNjKlPjc8SHeu52ZTio6tBBp1TcPB8A==` | `sha512-EGk9WkxnlQtqdERjPgS8gvWVjKLZnBQr95e65FOAs9MqkxVhHIjgipan7svZZ6n0rkp6vSc9GxyeY836boaT6w==` | `sha512-GeJEHtSjsbvRLpREfbQa5ksn6EuJKjxY/kXxuOP5Sn5FlXz6fuL3wHJr88PYlCOPV1KxxxWSc5h5jzKMKQQXDg==` |

The references, rc.0, rc.3 and rc.9, were fetched and verified the same way:

- `solid-js` rc.0 is `sha512-3enTJ71V…zdQ==`, rc.3 is `sha512-pmW6bRoT…LevtQ==`, and
  rc.9 is `sha512-J/oHWnWq…9ep0g==`.
- The signals and web values are those in `AUDITED_ARCHIVES` (rc.3) and in the
  rc.9 review.

`package.json` sha256 prefixes, `solid-js` / signals / web:

| release | `solid-js` | `@solidjs/signals` | `@solidjs/web` |
| --- | --- | --- | --- |
| rc.1 | `4b527809` | `f50cb6c2` | `48c98628` |
| rc.2 | `551714d3` | `d9d0603c` | `602450a9` |
| rc.4 | `fc9d277f` | `03f3d261` | `4cbb27e5` |
| rc.5 | `2afd6c83` | `386abecb` | `c44cef7c` |
| rc.6 | `45ddd552` | `de11cde1` | `2865947b` |
| rc.7 | `465c07cf` | `3d5154fa` | `5008e7ee` |
| rc.8 | `3ffba0f4` | `ed9a64eb` | `720b6695` |

Below, `<jN>`, `<sN>` and `<wN>` are the rc.N roots of `solid-js`,
`@solidjs/signals` and `@solidjs/web`.

### 1.2 What each release pins [M]

`npm view <pkg>@<v> dependencies peerDependencies` gives the ranges below.

| `solid-js` | `@solidjs/signals` range | `seroval` | `@solidjs/web` `peerDependencies.solid-js` |
| --- | --- | --- | --- |
| rc.1-rc.7 | `^2.0.0-rc.N` | `~1.5.4` | `^2.0.0-rc.N` |
| rc.8 | `^2.0.0-rc.8` | `~1.6.7` | `^2.0.0-rc.8` |

`@solidjs/web` does not depend on signals. `^2.0.0-rc.N` admits every later
2.0.0 prerelease and stable 2.x. **These are ranges, not pins.** A fresh
`npm install solid-js@2.0.0-rc.N @solidjs/web@2.0.0-rc.N`, run in the
scratchpad for N = 1, 3, 6 and 8, resolved `@solidjs/signals@2.0.0-rc.9`
every time (§ 5).

## 2. Method

1. **Declared surface [M].** A TypeScript 5.9.3 program (the main checkout's
   `packages/cli/node_modules/typescript`) imports each subpath from a per-rc
   `node_modules` with the three packages copied in, plus `csstype@3.2.3` and
   `seroval@1.5.6`/`1.6.7`. The subpaths are `solid-js`, `/refresh`,
   `/attribution`, `/internal`, `@solidjs/signals`, `/attribution`,
   `@solidjs/web`, `/server-functions{,/client,/server}`, `/storage` and
   `/jsx-runtime`. For every export the program records two things:
   - the printer-normalized declaration text, comments removed;
   - the checker's resolved call signature, so a `typeof core` alias that
     changes underneath is still seen.

   Every release was compared with rc.3, and rc.1-rc.3 also with rc.0.
2. **Runtime namespace [M].** Node imported every ESM bundle that the
   `exports` map can select. The 52 `TABLE` names plus `until` were checked
   in each namespace.
3. **Function slices [M].** The TypeScript parser split each bundle set into
   top-level function, class and const slices. Comments were stripped and
   bundler `$N` suffixes ignored. Slices were compared by name, and each
   differing slice was read as a diff. The bundle sets are:
   - signals: `dist/dev.js` (plus `dist/dev-shared.js` from rc.8) and
     `dist/prod/*.js`;
   - `solid-js`/web: `dev.js` or `solid.dev.js`/`web.dev.js`,
     `solid.js`/`web.js`, and `server.js`.
4. **Runtime probes [M].** 37 cases ran in a fresh Node process each, on
   `dist/dev.js` and `dist/prod/index.js` of every signals rc.0-rc.9. The rc.8
   and rc.9 `dist/observe/index.js` bundles were also run. The cases re-state
   the rc.9 review's probes: J, Y, AH, AI, X, K, L, M, N, AD, AE, AA, Q, AG, W,
   P, U, V, H, T, T2, E, D and R, the guard set, `until` ×4, the `omit`
   predicate and the `merge` copy.
5. **`tsc` over the published typings [M].** `strict`, `skipLibCheck: true`,
   bundler resolution, `jsxImportSource: "@solidjs/web"`, 21 case files per
   release. A second pass with `skipLibCheck: false` collected errors inside
   the packages.
6. **Checker probes [M].** The main checkout's release
   `rust/target/release/solid-checker-rust` (sha256 `169190a9…5928`, built
   2026-09-26 18:12) ran with `bin/solid-typefacts` (sha256 `fc93317f…aaf`)
   over scratch projects. The binary's source revision was not re-derived. It
   is estimated to include `a55a24d9` and `2990ec25`, because it emits SC9014
   and SC2005 [E].

### 2.1 Surface counts against rc.3 [M]

Rule-irrelevant server-functions churn is left out. The columns are added,
removed, changed and unchanged exports.

| release | `solid-js` `.` +/−/~/= | `@solidjs/signals` `.` +/−/~/= | `@solidjs/web` `.` +/−/~/= |
| --- | --- | --- | --- |
| rc.1 | 0/1/2/127 | 0/0/4/102 | 0/7/28/103 |
| rc.2 | 0/1/0/129 | 0/0/0/106 | 0/6/26/106 |
| rc.4 | 7/0/0/130 | 7/0/1/105 | 3/0/3/135 |
| rc.5 | 11/0/1/129 | 11/0/2/104 | 6/0/7/131 |
| rc.6 | 11/0/2/128 | 11/0/5/101 | 7/0/8/130 |
| rc.7 | 7/0/10/120 | 7/0/14/92 | 4/2/8/128 |
| rc.8 | 35/0/12/118 | 17/0/16/90 | 5/2/9/127 |

What the counts contain:

- **rc.1/rc.2 removals** are `resetErrorHalt` (`solid-js`, new in rc.3) and
  web internals rc.3 added (`rowProof`, `patchDriver`, `installHydrationRuntime`
  and others).
- **rc.1/rc.2 web changes** are the rc.0 declaration layout (§ 4.1).
- **rc.7/rc.8 web removals** are `rowProof` and `patchDriver`, as in rc.9.
- **rc.8 `solid-js` additions** are the attribution and diagnostics types, plus
  `mergeSources` and `OBSERVE`.
- **Nothing a `TABLE` name resolves to is removed in any release.**

### 2.2 Runtime namespaces [M]

Every `TABLE` name is a runtime export of every selectable client, dev,
server and observe bundle in rc.1-rc.8. `until` is exported from rc.5 on.

### 2.3 `tsc` results [M]

The table shows only the cases whose result changes. The other 10 cases give
the same result on every release:

- clean everywhere: the full `TABLE` import, the nested store write,
  `omit(props, "a")`, one-argument `dynamic`, two-argument `createEffect`, the
  `Signal` tuple, the store-setter draft, `refresh(m)`, and the
  `For`/`Show`/`Loading`/`Errored` JSX;
- TS2554 everywhere: one-argument `createRenderEffect`.

| case | rc.0-rc.2 | rc.3 | rc.4 | rc.5 | rc.6 | rc.7 | rc.8 | rc.9 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `store.a = 2` (store root) | TS2540 | TS2540 | TS2540 | TS2540 | TS2540 | clean | clean | clean |
| `projected.a = 5`, `optimistic.a = 3` (roots) | TS2540 | TS2540 | TS2540 | TS2540 | TS2540 | clean | clean | clean |
| `omit(props, (key) => …)` | TS2345 | TS2345 | TS2345 | TS2345 | TS2345 | TS2345 | TS2345 | clean |
| `dynamic(src, { static: true })` | TS2554 | TS2554 | TS2554 | TS2554 | TS2554 | **TS2353** | **TS2353** | clean |
| `const o = { static: true, deferStream: true }; dynamic(src, o)` | TS2554 | TS2554 | TS2554 | TS2554 | TS2554 | clean | clean | clean |
| `dynamic(src, { deferStream: true })` | TS2554 | TS2554 | TS2554 | TS2554 | TS2554 | clean | clean | clean |
| `import { until }` and a call | TS2305 | TS2305 | TS2305 | clean | clean | clean | clean | clean |
| `createEffect(() => x())`, one argument | clean | clean | clean | clean | TS2554 | TS2554 | TS2554 | TS2554 |
| `createErrorBoundary(() => …, (err) => …)` | clean | clean | clean | clean | clean | clean | clean | TS7006 (N2) |
| `render(…, { owner: null })` | TS2353 | clean | clean | clean | clean | clean | clean | clean |

With `skipLibCheck: false`, **only rc.9** reports errors inside a package: the
five TS2305 at `<j9>/types/index.d.ts:3,8`. rc.1-rc.8 compile clean.

### 2.4 Probe results [M]

**29 of 37 probes give the rc.3 answer on every release**, dev and prod alike,
rc.0-rc.9. Four more are the `until` probes, which have no rc.3 answer. The
29 cover the review's § 3.3 premises:

- compute during the call, for `createMemo`, `createSignal(fn)`,
  `createOptimistic(fn)`, `createProjection` and `createStore(fn)`;
- `createEffect` order `compute,returned,apply`;
- `createRenderEffect` order `compute,apply,returned`;
- `createTrackedEffect` order `returned,run`;
- `untrack` and `runWithOwner` stopping re-runs;
- `resolve` owning its thunk and throwing in a memo;
- `flush(fn)` running inline;
- `onSettled`'s two cleanup throws;
- `createReaction`'s null owner and `mapArray`'s owned rows;
- store writes dropped outside a setter, the draft committing, and a foreign
  store's write dropped;
- the signal setter in a root throwing;
- and the § 3.4 guard set: setter, store setter, action and refresh in a memo
  throw; a write and an action in a tracked effect are allowed; one-argument
  `createEffect` throws `MISSING_EFFECT_FN` in dev and crashes on `.effect` in
  prod.

The four probes that move:

| probe | rc.3 | first release that differs |
| --- | --- | --- |
| E: store setter directly in a `createRoot` body (dev) | `ok` | **rc.9 only** throws `REACTIVE_WRITE_IN_OWNED_SCOPE` (N3). rc.0 also throws; rc.1-rc.8 give `ok`. |
| R: `flush()` inside an action body (dev) | `resolved` | **rc.8** rejects with `FLUSH_IN_ACTION` (N4). Prod resolves on every release. |
| `omit(p, fn)` | `keys=ab calls=0` | **rc.9 only** (`keys=b calls=5`). |
| `merge` of a plain source, then a source write | `a=1` (eager copy) | **rc.9 only** (`a=5`, a view). |

The `until` probes are `no-export` through rc.4. On rc.5-rc.9 all four give
the same result, dev and prod:

- `fn` runs during the call;
- dev throws "Cannot call until inside a reactive scope";
- the promise resolves on the first truthy value (`calls=2`);
- `fn` runs under an owner.

The rc.8 and rc.9 observe bundles give 37/37 answers equal to the same
release's dev or prod answer.

## 3. The rc.9 review's differences, per release

### 3.1 B1: `Store<T> = T` from rc.7 [M]

- **Declaration.** `<s1..s6>/dist/types/store/store.d.ts:4` is
  `export type Store<T> = Readonly<T>`; `<s7>` and `<s8>` at the same line
  are `export type Store<T> = T`. `StoreOptions` gains `shallow` and
  `ProjectionOptions` loses it in the same release, as in rc.9. `affects`'s
  second overload follows, taking `Store<T>` = `T`.
- **`tsc`.** It stops reporting store, projection and optimistic root writes
  at rc.7 (§ 2.3).
- **Runtime.** Probe H drops the write on every release, dev and prod.
- **Checker, triple installs.** The source was
  `store.a = 2; store.nested.b = 3;`:
  - rc.3, rc.6, rc.7 and rc.8 all report SC2003 at line 5, the nested write,
    and only there;
  - so on rc.7 and rc.8 nobody reports the root write, exactly the rc.9 B1
    gap;
  - with the root write alone, rc.7 is `uncertifiable`, and only because of
    the SC9014 notice.

So rc.7 and rc.8 need `StoreRootTyping::Mutable`, which today only
`Solid2::RC9` carries.

### 3.2 B2: `dynamic`'s `static` option is rc.9 only [M]

- **rc.4-rc.6.** `dynamic(source)` keeps one parameter (`<wN>/types/index.d.ts:82`).
- **rc.7 and rc.8.** They add `dynamic(source, _options?: DynamicOptions)`
  with `DynamicOptions { deferStream?: boolean }`
  (`<w7>/types/index.d.ts:81-90`). The client bundle never reads `_options`
  (`<w7>/dist/web.dev.js:2042`, `<w8>/dist/web.dev.js:2074`). The source is the
  rc.3 lazy tracked memo, and no bundle before rc.9 contains `options?.static`.
- **rc.6.** It also changes `dynamic`'s string-tag branch to create the
  element lazily under the call's owner (`<w6>/dist/dev.js:2320`). rc.7 reverts
  that. It does not touch how the source is read [M], and no rule reads that
  branch [E].

On rc.7 and rc.8, the literal `{ static: true }` is TS2353. That is the only
form `call_form` refines to `DynamicStatic` (`call_option_literal`, exact
literal only: `solid-reactive-ir/src/lib.rs:2391-2442`). A tsc-clean
`static: true` through a variable reads as `Unknown`, which gives
`DynamicUnknownForm` and states nothing, so it fails closed.

### 3.3 B3: the `omit` predicate is rc.9 only [M]

`omit` has the single `(props, ...keys)` overload on rc.1-rc.8
(`<sN>/dist/types/store/utils.d.ts:73`, `:80` on rc.8). No runtime before rc.9
has `typeof keys[0] === "function"`. Probe `omit(p, fn)` gives
`keys=ab calls=0` through rc.8, so the function is treated as a key and never
called.

### 3.4 B4: `until` from rc.5 [M]

- **Declaration.**
  `until<T>(fn: () => T, options?: UntilOptions): Promise<Truthy<T>>`
  (`<s5>/dist/types/signals.d.ts:601`, `<s8>:600`). `UntilOptions`
  (`timeout`, `signal`) is textually equal to rc.9's. It is re-exported from
  the `solid-js` root (`<j5>/types/index.d.ts:1`).
- **Runtime.** The `until` slice is byte-identical, whitespace aside, from
  rc.5 to rc.9. It sits at `<s5>/dist/dev.js:6721`, `<s6>:7040`, `<s7>:8361`,
  `<s8>:2308` and `<s9>:2717`. The four probes agree (§ 2.4).
- **Checker.** SC2005 fires on rc.5 and rc.8 (§ 6 has the rc.3/rc.4 case).

The rc.9 model therefore describes `until` on rc.5-rc.8 too.

### 3.5 N2: broken re-export declarations are rc.9 only [M]

On rc.1-rc.8, all five names resolve to declarations:

- `createErrorBoundary`, `createLoadingBoundary`, `createRevealOrder` and
  `sharedConfig`, from `./client/hydration.js`;
- `$DEVCOMP`, from `./client/core.js`.

The surface walk finds no unresolved export in any subpath of any release but
rc.9. `skipLibCheck: false` is clean, and the `createErrorBoundary` case
type-checks (§ 2.3).

### 3.6 Packaging [M]

| change (rc.9 review § 2.2) | first release |
| --- | --- |
| `solid-js` `./types/*` subpath removed | rc.5 |
| `dev.js` renamed `solid.dev.js`/`web.dev.js`; `server.dev.js` and a `development` server condition added (`solid-js`, web); signals `node.dev.cjs` | rc.7 (not in the rc.9 list, but on the way to it) |
| CommonJS gone (no `require` condition, `.cjs` or `types-cjs/`) | rc.8 |
| `observe` condition and bundles | rc.8 |
| signals dev bundle split into `dev.js` + `dev-shared.js` | rc.8 |
| `solid-js/attribution`, `@solidjs/signals/attribution` | rc.8 |
| `solid-js/internal` | rc.9 only |

The rc.9 review found that none of this changes a vocabulary answer. That
holds for rc.5-rc.8. It does change which bundles a future negative row for
these archives must read: from rc.8 on, `dist/observe/**` and `dev-shared.js`.

### 3.7 The rc.9 review's other items [M]

| item | first release |
| --- | --- |
| N1: `createEffect(compute): never` overload removed, so one-argument `createEffect` is TS2554 | **rc.6** (`<s5>/dist/types/signals.d.ts:378` present, `<s6>` absent). `solid-js`'s `createEffect: typeof coreEffect` follows. |
| N3: store-setter root exemption removed | rc.9 only (`<s8>/dist/dev-shared.js:4854` still has `!context._root`) |
| N4: `flush()` in an action body | **rc.8** (`<s8>/dist/dev-shared.js:1896-1912`). Dev throws `FLUSH_IN_ACTION`. Prod runs `fn` and skips the drain. |
| N5: `refresh` returns a promise | rc.5 |
| N5: `createOptimisticStore` plain form gains `options` | rc.7 |
| N5: `render`/`hydrate` `onError` | rc.9 only |
| N6: `<For>` creates its `mapArray` lazily under `runWithOwner(owner)` | **rc.4** (`<j4>/dist/dev.js:1167`) |
| N6: `merge` always a view | rc.9 only |
| `createTrackedEffect` first run via `enqueueSub` + `schedule` | rc.7 (`<s7>/dist/dev.js:7552`). Probe M is unchanged. |
| `solid-js` `createRoot` as its own export | rc.9 only |

## 4. Other differences from rc.3 in what the dialect relies on

### 4.1 rc.1 and rc.2: each piece equals rc.0 or rc.3 [M]

- **Signals declarations.** rc.2's are identical to rc.3's, 106/106. rc.1
  differs only in `Owner` (private fields), `Dev` and `DiagnosticCode`/`Kind`.
- **Signals dev slices.** Against rc.3, rc.2 differs in 5 of 492:
  `captureStack`, `optimisticWrite`, `laneReadsCommitted`,
  `getLatestValueComputed` and `runProjectionComputedNext`. It lacks
  `wrapDraft` and adds `createWriteTraps`. On the 17 inventory slices rc.1
  mostly matches rc.0 (13/17). All probes match.
- **Web declarations.** rc.1 and rc.2 ship the rc.0 layout. `@solidjs/web`
  declares its own
  - `untrack(fn)`,
  - `getOwner(): unknown`,
  - `effect(fn, effect)`,
  - `memo(fn, equal)` and
  - `createComponent`

  in `types/client.d.ts`. `render` takes `init?: unknown` and no `owner`
  option, and `hydrate` is `typeof hydrateCore`.
- **Web runtime.** On the helpers the execution facts model (`render`,
  `hydrate`, `insert`, `ref`/`applyRef`, `template`, `delegateEvents`,
  `effect`, `memo`, `dynamic`, `useHead`), every web dev slice in rc.1 and rc.2
  equals rc.0's. `clientOnly` equals rc.3's.
- **Compiler facing.** `@solidjs/web` rc.1 and rc.2 export neither `rowProof`
  nor `patchDriver`. The pinned compiler emits `rowProof` only with the
  opt-in `patch_driver` (rc.9 review § 6), so they are not needed [M for the
  exports, E for the emission path].
- **`solid-js` runtime.** `devComponent` in rc.1 lacks rc.3's non-function
  `Comp` guard. Otherwise it is the same `createRoot(…, { transparent: true })`
  + `untrack(() => Comp(props), label)`.

### 4.2 rc.4-rc.6: patch-channel exports that take callbacks [M facts, E effect]

rc.4 adds the following to the `solid-js` root (`<j4>/types/index.d.ts:1`,
from `@solidjs/signals`):

- `registerPatch(record, fn: PatchFn): () => void`;
- `registerRowOps(array, fn: RowOpsFn): () => void`;
- `registerSlotPatch(arr, fn: (index, next, prev) => void): () => void`;
- `patchableRaw`, `storeIsShallow`, `storeHasFamily` and
  `storeHasOptimisticFamily`.

It adds `installListDriver(driver)`, `driveList(parent, listFn, marker?,
lateClassic?)` and `listDriver` to the `@solidjs/web` root.

- **What `registerPatch` does.** It captures `getOwner()` and stores `fn`
  (`<s4>/dist/dev.js:9632`), which runs from store-commit dispatch (the rc.6
  audit's `applyEntries` → `entry.fn(next, prev, force)`).
- **Lifetime.** rc.7 drops `registerPatch`, `registerRowOps`,
  `registerSlotPatch`, `patchableRaw` and the three web names. The
  `storeIs*`/`storeHas*` predicates stay.
- **Why it matters.** `every_callback_taking_export_is_modelled_or_excluded`
  (`solid_2.rs:3997`) reads the checked-in rc.3 contract, so it cannot see
  these. None is in `TABLE` or `UNMODELLED_CALLBACK_TAKERS`. That is the
  completeness failure the rc.9 review called B4. They are renderer and
  patch-driver plumbing rather than application API [E]. The owner can exclude
  them with that justification or keep a notice that names them.
- **Related internals.** `For` sets `list.$ll` on rc.4-rc.6, and `insert`
  hands such accessors to a `listDriver` only when one is installed
  (`<w4>/dist/dev.js:686`). The default path is rc.3's [M for the code, E that
  no default build installs a driver].

### 4.3 Smaller items with no rule effect [M facts, E effect]

- **rc.1: store declarations.** `snapshot`, `reconcile`, `deep` and `lazy`
  change declarations from rc.0 to rc.1, and rc.1 already equals rc.3.
- **rc.5: `refresh`.** Its guard moved into a deferred waiter. Probe G6 still
  throws `REACTIVE_WRITE_IN_OWNED_SCOPE` in a memo on rc.5-rc.9.
- **rc.7: `getContext`.** It was rewritten from `hasContext`/`isUndefined` to
  `value === undefined` checks. `isUndefined` is `typeof value ===
  "undefined"` (`<s3>/dist/dev.js:4487`), so the semantics are the same.
- **rc.7: `runWithOwner`/`onCleanup` diagnostics.** They now use
  `reportDiagnostic`, as rc.9 does.
- **rc.8: component names.** `createComponent` gains `name?` and
  `devComponent` becomes `observedComponent`. The body keeps the same
  `createRoot(…, { transparent: true })` + `untrack(() => Comp(props),
  label)`.
- **rc.8: effect default names.** `createEffect`/`createRenderEffect`/
  `createTrackedEffect`/`createReaction` stop defaulting `name`.
- **Server bodies.** `serverEffect`, `createEffect`, `createRenderEffect`,
  `createRoot`, `untrack`, `onSettled` and `createTrackedEffect` in
  `solid-js` `dist/server.js` are byte-identical to rc.3's on every release.
  `createProjection` (rc.4+), `lazy` (rc.5+) and
  `createStore`/`createOptimisticStore` (rc.7+) change. SC7001's server
  premise stays unchanged.
- **`@solidjs/web` `effect`.** It records the binding node from rc.7
  (`<w7>/dist/web.dev.js:64`), the same `createRenderEffect` shape as rc.9.

## 5. The Store typing follows `@solidjs/signals`, not `solid-js` [M]

- **Where the detection looks.** `detect_detailed`
  (`rust/crates/solid-facts-backend/src/dialect.rs:553-590`) reads the nearest
  `node_modules/solid-js/package.json` and asks
  `review_release(&installed)`.
- **Where `Store` comes from.** `@solidjs/signals`, re-exported through
  `solid-js`'s `createStore` declaration.
- **What a fresh install resolves.** Every `solid-js` rc depends on
  `^2.0.0-rc.N` (§ 1.2). `npm install solid-js@2.0.0-rc.3
  @solidjs/web@2.0.0-rc.3` in an empty directory today resolves
  `@solidjs/signals@2.0.0-rc.9`, whose `store.d.ts:4` is `Store<T> = T`. The
  same happens for rc.1, rc.6 and rc.8.
- **What `tsc` says** on that fresh rc.3 tree:
  - `store.a = 2` is clean;
  - `import { until } from "solid-js"` is TS2305, because the rc.3 root does
    not re-export it;
  - one-argument `createEffect` is TS2554, because the signals rc.9 overload
    set applies.
- **What the checker says** on the same tree, with only `store.a = 2`:
  `status: certified`, no finding, no notice. The runtime drops the write.
  The exact rc.3 triple is also `certified`, and correctly so: `tsc` reports
  TS2540 there.

Consequences:

- `Solid2::AUDITED`'s `Readonly` answer is right only while the resolved
  signals is rc.0-rc.6. `Solid2::RC9`'s `Mutable` is right only for signals
  rc.7-rc.9. For a `solid-js` rc.1-rc.6 install either can be installed.
- The SC9014 hint "Pin solid-js 2.0.0-rc.3, the audited release, to certify"
  does not produce the audited bytes. It also has to pin `@solidjs/signals`
  (to rc.3 or rc.6, the audited archives).
- The other signals-owned rc.9 differences also reach a `solid-js@2.0.0-rc.3`
  install through skew: the `omit` predicate typing and runtime, N3, N4 and the
  `merge` view [E, by the same resolution]. B3's model is unconditional, so
  only B1 carries a per-release answer.

**Suggested direction [E].** Answer `StoreRootTyping` from the resolved
`@solidjs/signals` version, found from the resolved `solid-js` directory the
way a bundler resolves it: `Readonly` for rc.0-rc.6, `Mutable` for rc.7-rc.9.
If that version is unknown or has not been read, state nothing, or give the
notice. The `solid-js` release review stays as it is for the vocabulary it
does own (`until`'s re-export, `dynamic` in `@solidjs/web`, the component
wrappers). This is a seam decision for the lead and owner, and this review
changes nothing.

## 6. Additive models on releases whose typings reject the form

The rc.9 review argued: "B2, B3 and B4 are additive. rc.3's own typings
reject each rc.9 form…, so modelling them cannot change an rc.3 answer." That
holds for type-correct code. On code `tsc` rejects, the models do produce
findings, and the finding's claim is false on those bytes. Measured with the
same binary:

- **`until`.** `import { until } from "solid-js"` and `until(() => ready())`
  inside a `createMemo` give SC2005 at line 4 on the rc.3 and rc.4 triples.
  `tsc` reports TS2305 on the import, and neither runtime has `until`. On rc.5
  and rc.8 the same code is tsc-clean and SC2005 is correct.
- **`dynamic`.** `dynamic(() => { createEffect(…); return Plain; },
  { static: true })` at module scope gives SC4001 (missing-owner) at line 6 on
  the rc.3 triple (TS2554), the rc.7 triple (TS2353) and rc.9 (clean). On
  rc.3 and rc.7 the runtime ignores the option and creates the effect under
  `dynamic`'s memo, so the missing-owner claim holds only on rc.9.

Neither case duplicates a `tsc` diagnostic, since the claims differ. Both
state rc.9 behaviour on bytes that lack it, and both follow from a callee
that resolves to no declaration or a rejected overload. This is recorded for
the lead [M for the findings, E for the interpretation]. No classification
depends on it.

## 7. Not done

- **Other bundles.** The CommonJS bundles of rc.1-rc.7 (`node.cjs`, `*.cjs`)
  were not probed or sliced. They are selectable through `require`. Only
  their presence was recorded.
- **Observe bundles.** They were probed (37/37) but not sliced.
- **Server functions.** Beyond the export counts, the `@solidjs/web`
  server-functions subpaths were not compared.
- **Compiler.** It was not examined per release. No `solid-js` rc declares a
  compiler dependency, so the rc.9 review's § 6 conclusion is assumed to carry
  over [E].
- **Checker scope.** Checker probes ran on the rc.3, rc.4, rc.5, rc.6, rc.7,
  rc.8 and rc.9 triples and the fresh rc.3 tree only, for the cases named in
  §§ 3.1, 3.4 and 6. No census, contract or negative-row reading was done for
  any rc.1-rc.8 archive, and none is in `AUDITED_ARCHIVES` except
  `@solidjs/signals@2.0.0-rc.6`.
- **Skew.** Mixed installs other than the four fresh trees in § 5 were not
  enumerated.
