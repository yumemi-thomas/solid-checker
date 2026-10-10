# Surface review: does the rc.9-audited Solid 2 vocabulary hold for `2.0.0-rc.13`?

Date: 2026-10-05. Read-only review; no repository file, dialect answer or
archive was changed. Template: `docs/package-contract-v2/audits/2026-09-26-solid-2-rc9-vocabulary-review.md`
(§§ 1, 2, 2.1-2.3, 3.1, 3.2, 3.5, 3.6, 4, 5), redone for rc.9 -> rc.13.

Tags: **[M]** measured (file:line, digest, or command output on the named
bytes), **[E]** estimated (inferred from code reading, not executed).

Path shorthand. `<r9js>`, `<r9sig>`, `<r9web>` are
`rust/target/audited-archives/solid-v2/2.0.0-rc.9/node_modules/{solid-js,@solidjs/signals,@solidjs/web}`.
`<r13js>`, `<r13sig>`, `<r13web>` are
`rust/target/audit-rc13/node_modules/{solid-js,@solidjs/signals,@solidjs/web}`.
Scratch and raw outputs are under `rust/target/audit-rc13/surface/work/`
(scripts `decls.cjs`, `cbs.cjs`, `slices.cjs`, `rt2.mjs`, `probe-rt.mjs`, and
their `*.json`/`*.txt` outputs).

## 0. Recommendation

**rc.13 is vocabulary-compatible with the rc.9 vocabulary. Every
release-dependent answer the dialect gives for rc.9 (B1-B4, N3-N5 of the rc.9
review) is measured to hold unchanged on rc.13's bytes [M] (§ 3.2, § 3.5,
§ 4).** The five broken `solid-js` re-exports of rc.9 (N2) are fixed (§ 2.3).
What does *not* hold is the dialect's *bookkeeping*: it only knows rc.9, so an
rc.13 installation today is analysed under the conservative vocabulary and
loses all seven answers (§ 4, B1). That is a release-table change, not a new
semantic reading. Two smaller items follow (B2 archive rows, B3 module
ownership of three moved primitives), and one conditional item (B4). Details
and the N-items are in §§ 4 and 5.

What this review did **not** measure is listed in § 6. In particular it
reads **only rc.13**; rc.10, rc.11 and rc.12 were not available and nothing
here says anything about them [E].

## 1. The bytes compared

| package | rc.9 (audited) | rc.13 (under review) |
| --- | --- | --- |
| `solid-js` | `<r9js>` | `<r13js>` |
| `@solidjs/signals` | `<r9sig>` | `<r13sig>` |
| `@solidjs/web` | `<r9web>` | `<r13web>` |

- **rc.9 identity [M].** The archive's three `package.json` sha256 values
  equal the dialect's `AUDITED_ARCHIVES` `manifest_sha256` tuples
  (`solid_2.rs:239-282`; `solid-js` `c8d6224b…11bc`, signals `c612461c…b1c6`,
  web `5de9244e…73dc`). A byte copy of the three trees (plus `seroval`
  1.6.8, see § 6) under `work/rc9/` has the same sorted-file digest as the
  archive (`solid-js` `01dc9dd115f80b83`, signals `0935750d32cd530a`, web
  `4535e3500e480ceb`), so every "rc.9" measurement below is on archive bytes.
- **rc.13 `package.json` sha256 [M]:**
  - `solid-js`: `ce43a022f763e5995d1ac8a5f78f135804edd01b627a2bcbee8ff9530e79d284`
  - `@solidjs/signals`: `6783c3c632cfebf3a0fc26e18925887462b5925c481c6535ae0186845efb95a6`
  - `@solidjs/web`: `8b45ed71ed7a369883e7e00bb48b01fd7bda935bd5ba889cbecafc8eaa122bf5`
- **rc.13 integrities [M, as recorded; not re-derived].** From
  `rust/target/audit-rc13/node_modules/.package-lock.json`. No tarball was
  downloaded (no network), so these are the lockfile's claims, not a
  verification [E for authenticity]:
  - `solid-js@2.0.0-rc.13`: `sha512-62bYOI4JZ15KOqL5eReKyWSwAXrGb0fbX8SDnHsbJk2UX+SzSyy1gobas2cxW/0BXcobGfQ1V6ffPyQkMIBdoQ==`
  - `@solidjs/signals@2.0.0-rc.13`: `sha512-4+pRdrAHtfyE9BUJWBup3TpzJojJjgW2mV1vm/Jik4tWa5epxXB/YrLkwqP1v8+S9XjyKKZu5BSLqmcpswMYeQ==`
  - `@solidjs/web@2.0.0-rc.13`: `sha512-vI/7v/XM8B/3U/oKAzCW2VjTLFkM5IArB2DFnrjxyPRYD3FX9KAXg9QuAnPIPQHC/xvg/LgxmvoVfZUfJ96gOQ==`
  - `seroval@1.6.8` `sha512-HlSgSAkT…Sa+3wg==`, `seroval-plugins@1.6.8`
    `sha512-N7mWAMyd…d5Fg==` (full values in the lockfile), `csstype@3.2.3`.
- **File counts [M]** (excluding the archive stamp `.solid-checker-archive.json`
  that the rc.9 archive carries and rc.13 does not):

  | package | rc.9 | rc.13 | added | byte-identical | changed |
  | --- | ---: | ---: | --- | ---: | ---: |
  | `solid-js` | 33 | 34 | `types/recovery.d.ts` | 7 | 26 |
  | `@solidjs/signals` | 119 | 119 | none | 39 | 80 |
  | `@solidjs/web` | 67 | 75 | `performance-tracks/{dist/*.js (3),package.json,types/index.d.ts}`, `skills/server-components/SKILL.md`, `types/frames/tree-rewrite.d.ts`, `types/request-error-hook.d.ts` | 19 | 48 |

- **Dependency pins [M].**
  - `solid-js` rc.9: `@solidjs/signals ^2.0.0-rc.9`, `seroval ~1.6.7`,
    `seroval-plugins ~1.6.7`, `csstype ^3.1.0`. rc.13: `@solidjs/signals
    ^2.0.0-rc.13`; the other three unchanged.
  - `@solidjs/web`: `seroval`/`seroval-plugins ~1.6.7` and peer `solid-js`
    `^2.0.0-rc.9` -> `^2.0.0-rc.13`.
  - `@solidjs/signals`: no dependencies, either version.
  - The rc.13 install resolved `seroval`/`seroval-plugins` **1.6.8** (inside
    `~1.6.7`).
  - Outside `version` and those pins, `solid-js` and `@solidjs/signals`
    `package.json` are identical; `@solidjs/web` differs only by the
    `./performance-tracks` export and four `files` entries (§ 2.2).
- **Main bundles [M]** (sha256 prefix, bytes; full digests in
  `work/bundles-full.md`):

| file | rc.9 | rc.13 | |
| --- | --- | --- | --- |
| `solid-js/dist/solid.js` | `0238f90858359bc7…` 41418 | `81111de698a29afc…` 47821 | changed |
| `solid-js/dist/solid.dev.js` | `7baf8808f6bd8701…` 45011 | `0489d4c57801d5d3…` 52305 | changed |
| `solid-js/dist/solid.observe.js` | `c337cd4b1b90e5dd…` 41964 | `fa9a7c25d1827aa1…` 48884 | changed |
| `solid-js/dist/server.js` | `9c25fe06f9aab765…` 74707 | `f65a92da939c10cb…` 82108 | changed |
| `solid-js/dist/server.dev.js` | `129cbe735cceed1f…` 84772 | `be726daa949ae51c…` 94291 | changed |
| `solid-js/dist/server.observe.js` | `0d1519f0613584c5…` 79326 | `e30d10de1732fe70…` 87463 | changed |
| `solid-js/dist/internal.js` | `b068dc5cbdbe9953…` 928 | `e04f9e241a8cee5b…` 1417 | changed |
| `solid-js/dist/attribution.js` | `3e9a2c2384c8407d…` 46 | `3e9a2c2384c8407d…` 46 | identical |
| `solid-js/types/client/flow.d.ts` | `81af6e73951ea01b…` 11188 | `963ee0984c76c0d3…` 12936 | changed (comments only, § 3.2) |
| `@solidjs/signals/dist/dev.js` | `f08c227c5c64baad…` 418037 | `8dc03b1090035806…` 464911 | changed |
| `@solidjs/signals/dist/dev-shared.js` | `70b88ba97dcb1107…` 295301 | `c19016529420dcc5…` 340007 | changed |
| `@solidjs/signals/dist/prod/index.js` | `aec0c86a89ff4f7f…` 2444 | `6ed02c8d890e3663…` 2460 | changed |
| `@solidjs/signals/dist/observe/index.js` | `52ee1b4037b0d10c…` 2499 | `c83e729da4130d51…` 2515 | changed |
| `@solidjs/web/dist/web.js` | `32083d72a93231d8…` 78071 | `7006f19d7b2a4294…` 79782 | changed |
| `@solidjs/web/dist/web.dev.js` | `bcbe02189e31a7ce…` 86356 | `bd417d367b53da35…` 91395 | changed |
| `@solidjs/web/dist/web.observe.js` | `b5b2950b906244f1…` 79035 | `2ee52aa0219d1eb3…` 81505 | changed |
| `@solidjs/web/dist/server.js` | `b96fc8399d4c9909…` 132975 | `f798fc997b031f48…` 151974 | changed |
| `@solidjs/web/dist/server.dev.js` | `4636ed7864cad6f1…` 143973 | `d71df58be3ffb831…` 171584 | changed |
| `@solidjs/web/dist/server.observe.js` | `9919deaaa0c6b73d…` 136002 | `ab31eaf25a26ed90…` 157051 | changed |
| `@solidjs/web/server-functions/dist/client.js` | `552e7babfae6d9ed…` 38145 | `ac5825f1d29ea4d7…` 46463 | changed |
| `@solidjs/web/server-functions/dist/server.js` | `3a185599499b4926…` 97380 | `3bd9cc377ccdf2a1…` 109474 | changed |
| `@solidjs/web/frames/dist/client.js` | `cccf274442f18ff8…` 66554 | `782f9567d4959a7a…` 79414 | changed |
| `@solidjs/web/frames/dist/server.js` | `e9626518184c7be3…` 152399 | `6c18d489beaaf180…` 57545 | changed |

  Every runtime bundle except `solid-js/dist/attribution.js` changed, so no
  rc.9 file-level digest transfers. Everything below is therefore a function-
  or declaration-level comparison.

## 2. Method

1. **Declared surface [M].** TypeScript 5.9.3
   (`packages/cli/node_modules/typescript`, run on Node 24.21.0). One program
   per version imports every non-wildcard `exports` subpath of the three
   packages (`work/specs9.txt`, `specs13.txt`; 19 specifiers on rc.9, 20 on
   rc.13). For each export the checker resolves aliases and prints the
   comment-stripped declaration text (`work/decls.cjs` -> `decls-rc{9,13}.json`).
   Real module resolution (bundler) over real `node_modules`; no stubs.
2. **Runtime surface [M].** Node 24.21.0 imported every JS target of every
   `exports` entry and listed `Object.keys`. Dev bundles ran under
   `--conditions=development`, observe bundles under `--conditions=observe`
   (without them `solid.dev.js` and the server bundles throw on load because
   `@solidjs/signals` resolves to its prod build) (`work/rt2.mjs`, `rt-*.json`,
   `rtdiff.txt`).
3. **Function slices [M].** The TypeScript parser extracted top-level
   function/class/const slices from each bundle and compared sha256 by name
   (`work/slices.cjs`, `slicediff.cjs`, `slicecmp2.cjs`, `tierdiff.cjs`).
   A differing slice was read as a unified diff. Prod builds are minified and
   mangled: a differing slice there can be a pure alpha-rename (§ 4, B2).
4. **`tsc` against the published typings [M].** `strict`, `moduleResolution
   bundler`, `jsx preserve`, `jsxImportSource "@solidjs/web"`, `types: []`,
   `skipLibCheck` false and true (`work/p9`, `work/p13`, `probe.tsx`,
   `probe2.tsx`, `probe3.ts`).
5. **Runtime probes [M].** 15 cases on `@solidjs/signals` dev and prod entries
   of both versions (`work/probe-rt.mjs`, `rt-probe-rc{9,13}-{dev,prod}.json`).
6. **Dialect reading [M for line numbers, E for effect].** The dialect was
   read, not executed: no cargo, make or rustc was run, so no claim about what
   the checker prints on rc.13 is measured. Current line numbers:
   `solid_2.rs` `TABLE` 120-180, `AUDITED_ARCHIVES` 239-282, `callback_positions`
   4950, `modules()` 4804, `export_modules` 6373, `NAMESPACE_SOLID_JS` 6424,
   `NAMESPACE_SOLIDJS_WEB` 6476, `UNMODELLED_CALLBACK_TAKERS` 7003,
   `every_callback_taking_export_is_modelled_or_excluded` 7019;
   `solid_2/releases.rs` `AUDITED_INSTALLATION` 120, `NEWEST_READ` 447,
   `KNOWN_GAPS` 465, `Release::of` 546, `vocabulary_for` 591-650;
   `exports/solid_v2_solid_js.rs`, `exports/solid_v2_solidjs_web.rs`;
   `lib.rs:3283` `callback_exports_from_bundles`.

### 2.1 Surface counts [M]

Declaration names per subpath (value and type exports counted separately by
name; aliases resolved) (`work/declsdiff.json`):

| subpath | added | removed | declaration changed | unchanged |
| --- | ---: | ---: | ---: | ---: |
| `solid-js` `.` | 6 | 17 | 12 | 141 |
| `solid-js/internal` | 7 | 0 | 2 | 27 |
| `solid-js/attribution` | 11 | 5 | 6 | 24 |
| `solid-js/refresh` | 0 | 2 | 1 | 9 |
| `@solidjs/signals` `.` | 3 | 3 | 10 | 142 |
| `@solidjs/signals/attribution` | 11 | 5 | 6 | 24 |
| `@solidjs/web` `.` | 11 | 1 | 9 | 151 |
| `@solidjs/web/jsx-runtime`, `/jsx-dev-runtime` | 0 | 0 | 1 (`JSX`) | 1 each |
| `@solidjs/web/server-functions` (and `/client`) | 9 | 0 | 3 | 47 |
| `@solidjs/web/server-functions/server` | 7 | 0 | 4 | 58 |
| `@solidjs/web/frames` (`/client`, `/server`) | 3 | 0 | 1 | 11-12 |
| `@solidjs/web/performance-tracks` (new) | 2 | n/a | n/a | n/a |
| `@solidjs/web/{storage,serialization,serialization/decode,server-functions/rich-args}` | 0 | 0 | 0 | 1, 27, 16, 1 |

Reading the `solid-js` `.` row [M]:

- **Removed (17):** `$DEVCOMP`, `sharedConfig`, `createErrorBoundary`,
  `createLoadingBoundary`, `createRevealOrder`, plus 12 types
  (`AttributionHooks`, `OriginRef`, `Acknowledgement`, `AttributionRecords`,
  `AttributionRecordType`, `ChangeRecord`, `HeldWrite`, `HoldEvent`,
  `InteractionEvent`, `NavigationEvent`, `NavigationHop`, `RerunEvent`).
- **Added (6):** values `isHydrating`, `isHydratable`; types
  `RecordSubscribeOptions`, `RecoveryEvent`, `RecoveryLive`, `RecoveryListener`.
- The five removed *values* are **moved, not deleted**: all five are declared
  in `solid-js/internal` on rc.13 (`<r13js>/types/internal.d.ts`) and are still
  runtime exports of the root bundle (below).

Runtime namespace changes, distinct names across every entry and build [M]
(`work/rtdiff.txt`):

- **`solid-js` root bundles** (`solid.js`, `solid.dev.js`, `solid.observe.js`,
  `server*.js`): added `isHydrating`, `isHydratable`,
  `inLiveServerComponentScope`, `shareAsyncIterable` (81 -> 85 names client,
  82 -> 86 server); removed none. `$DEVCOMP`, `sharedConfig`,
  `createErrorBoundary`, `createLoadingBoundary`, `createRevealOrder` are
  **still exported at runtime from the root** on all six builds and are now
  also exported from `solid-js/internal` (26 -> 33 names).
- **`@solidjs/signals`:** added `$RECORD`, `setConsoleFooter` (and `graphSize`
  in the `attribution` entries); removed `ownerPath` (88 -> 89 names prod).
- **`@solidjs/web`:** 28 distinct runtime names added, 1 removed (`CLAIM_PROP`):
  - Client root (117 -> 120): `getHydrationWriter`, `ssrElementAttribute`,
    `takeHydrationValue`.
  - Server bundle (123 -> 132): the three above plus `SLOT_FACE_DATA`,
    `SLOT_FACE_MARKUP`, `SLOT_FACE_STREAM`, `SLOT_MARKER`, `SLOT_VALUE`,
    `isSlotValue`, `slotValue`. Of these the `SLOT_*`/`slotValue`/`isSlotValue`
    names are exported by the server bundle with declarations only in
    `types/server.d.ts:406-433`, which is not an entry (so undeclared at every
    entry).
  - Frames: `FRAME_HAVE_BUDGET`, `FRAME_HAVE_HEADER`.
  - Server functions client: `EVENT_STREAM_HEARTBEAT`, `EventStreamReader`,
    `LAST_EVENT_ID_HEADER`, `LIVE_WIRE`, `createEventChunk`,
    `deliverFlightData`, `hasFlightMetadata`, `isEventStream`, `positionDigest`.
  - Server functions server: `ChunkReader`, `armLiveBody`, `createChunk`,
    `createEventChunk`, `frameAddress`, `serializeStream`, `textDigest`.
  - New subpath: `enablePerformanceTracks`.
  - Declarations: 31 distinct names added across the web subpaths, 18 changed.
  The "~40" in the brief is closer to the declaration count (types included)
  than to the runtime count.
- **The four names in the brief that are not exports.** `callerRenderContext`
  (`<r13js>/types/server/shared.d.ts:99`), `installServerWithOrigin`
  (`shared.d.ts:116`), `devPeekNextChildId` (`types/server/signals.d.ts:68`) and
  `ownerId` (`types/server/signals.d.ts`, `@solidjs/signals`
  `dist/types/core/dev.d.ts:37`) appear in no entry's declaration surface and
  in no bundle's runtime export list [M]. They are module-internal
  declarations of the server typings. `ownerId` is also an *interface field*
  in signals' dev types, present on rc.9 too. Class I (irrelevant).

### 2.2 Packaging [M]

- **`exports` maps are identical** for `solid-js` and `@solidjs/signals`:
  same subpaths (`.`, `./refresh`, `./attribution`, `./internal`; signals
  `.`, `./attribution`) and same conditions in the same shape (`types`, `worker`,
  `deno`, `node`, `browser`, `observe`, `development`, `default`; signals
  `types`, `test`, `development`, `observe`, `default`). The targets are the
  same files: `dist/solid{,.dev,.observe}.js`, `dist/server{,.dev,.observe}.js`,
  `dist/dev.js` (+ `dist/dev-shared.js`), `dist/prod/index.js`,
  `dist/observe/index.js`. `dist/solid.dev.js` and `dev-shared.js` still exist.
  `main`, `module`, `types`, `sideEffects`, `type`, `typesVersions`, `browser`
  are identical.
- **`@solidjs/web` changed in exactly one place:** a new subpath
  `./performance-tracks` with `browser`/`deno`/`node`/`worker`/`development`/
  `observe`/`default`/`types` conditions (same condition set as `.`), targets
  `performance-tracks/dist/performance-tracks{,.dev,.observe}.js` and
  `performance-tracks/types/index.d.ts`, plus four `files` entries
  (`performance-tracks/{dist,types,package.json}`, `skills`). All other web
  subpaths (`.`, `jsx-runtime`, `jsx-dev-runtime`, `storage`, `serialization`,
  `serialization/decode`, `server-functions{,/server,/client,/rich-args}`,
  `frames{,/server,/client}`) are unchanged.
- **No `require` condition, no `.cjs`, no `types-cjs`** in any rc.13 package,
  as on rc.9.
- **Tiers.** `dist/prod/**`, `dist/dev.js`+`dist/dev-shared.js`, and
  `dist/observe/**` are still the three signals builds, so "every bundle the
  `exports` map can select" for a row is the same set of file kinds as on rc.9.
- The new `performance-tracks` prod build is a no-op: its entire source is
  `function enablePerformanceTracks(options = {}) { return noop; }`
  (`<r13web>/performance-tracks/dist/performance-tracks.js`, 126 bytes) [M].

Dialect effect [E]: nothing in `modules()` (`solid_2.rs:4804-4821`) names
`./performance-tracks`, so imports from it are not owned; the export is not a
reactive primitive, so that is correct (N5).

### 2.3 `tsc` over the published typings [M]

Probe `work/probe.tsx` imports all primitives of `TABLE` that live in
`solid-js` and `@solidjs/web`, plus `sharedConfig` and `$DEVCOMP`, and
exercises the shapes the rows restate (`Signal` tuple, `createEffect(compute,
apply)`, `createStore` draft setter, keyed `For`/`Show`, `Loading`, `Errored`,
`Repeat`) and the rule-premise cases. Both versions were run with the same
harness.

- **Whole published surface, `skipLibCheck: false`** (every subpath of all
  three packages imported, `work/p*/all.ts`):
  - **rc.9: 5 errors**, all inside the package, exactly the template's N2:
    TS2305 at `<r9js>/types/index.d.ts(3,10)` (`$DEVCOMP`) and `(8,10)`,
    `(8,53)`, `(8,74)`, `(8,97)` (`sharedConfig`, `createErrorBoundary`,
    `createLoadingBoundary`, `createRevealOrder`).
  - **rc.13: 0 errors.**
- **N2 is fixed, by removal.** rc.13 `types/index.d.ts` no longer re-exports the
  five names (line 3 is `export { children, createContext, useContext }`,
  line 8 has no `sharedConfig`/boundary names); they are declared, correctly
  typed, in `<r13js>/types/internal.d.ts`:
  - `createErrorBoundary<T,U>(fn: () => T, fallback: (error: Accessor<unknown>,
    reset: () => void) => U): Accessor<T | U>`
  - `createLoadingBoundary<T,U>(fn: () => T, fallback: () => U, options?: {
    on?: () => any }): Accessor<T | U>`
  - `createRevealOrder<T>(fn: () => T, options?: { order?: () => RevealOrder;
    collapsed?: () => boolean }): T`
  - `sharedConfig: SharedConfig`, `$DEVCOMP: symbol`.
- **But importing them from the root is now a real `tsc` error**, with either
  `skipLibCheck`. `import { createErrorBoundary, createLoadingBoundary,
  createRevealOrder, sharedConfig, $DEVCOMP } from "solid-js"` gives 5 x
  TS2305 at the import (`probe.tsx(2,59)`, `(2,80)`, `(4,23)`, `(7,39)`,
  `(7,53)`); `Solid.createErrorBoundary(...)` through a namespace import gives
  TS2339 (`probe3.ts`). On rc.9 the same code type-checked silently
  (untyped `any`, because the re-export was unresolved). From
  `solid-js/internal` all five type-check on rc.13 (`probe2.tsx`, no errors)
  and are TS2305 on rc.9.
- **`skipLibCheck: true`:** rc.9 and rc.13 both report only the expected
  probe cases below (rc.13 adds the five root-import TS2305 above).

Rule-premise cases, `tsc` result on each version (strict, either
`skipLibCheck`):

| case | rc.9 | rc.13 |
| --- | --- | --- |
| `createEffect(() => a())`, one argument | TS2554 | TS2554 |
| `createRenderEffect(() => a())`, one argument | TS2554 | TS2554 |
| `st.a = 2` on a `createStore` root | clean | clean |
| `proj.a = 5`, `opt.a = 3` (projection / optimistic root writes) | clean | clean |
| `omit(props, (key) => key === "a")` | clean | clean |
| `dynamic(() => "div", { static: true })` | clean | clean |
| `import { until } from "solid-js"` | clean | clean |
| `createErrorBoundary(...)` imported from `"solid-js"` | clean (untyped) | TS2305 |
| the same five from `"solid-js/internal"` | TS2305 | clean |

All other rows equal rc.9 [M]. The `createEffect` declaration is a single
overload in both (`signals.d.ts:384` on rc.13), so N1 stands.

## 3. Inventory: what the dialect assumes, rc.9 against rc.13

Classes: **U** unchanged; **X** export renamed/added/removed/moved;
**S** signature change; **R** semantic change that affects a rule; **I**
irrelevant change.

### 3.1 Export vocabulary

| assumption | source | rc.9 -> rc.13 evidence | class |
| --- | --- | --- | --- |
| The `TABLE` names are exports of `solid-js` / `@solidjs/web`. | `solid_2.rs:120-180`: **53** rows (the template's 52 plus `until`) | All 53 are runtime exports on rc.13 [M]. The set of runtime entries (`solid.js`, `solid.dev.js`, `server.js`, `web.js`, `web.dev.js`, web `server.js`, signals prod) that exports each name is identical on rc.9 and rc.13 for **all 53** [M] (`work/vocab.cjs`). Declaration status changed for 3: `createErrorBoundary`, `createLoadingBoundary`, `createRevealOrder` were unresolved root re-exports on rc.9 and are absent from the root typings and declared in `solid-js/internal` on rc.13 [M]. | U (runtime); X for those 3 (moved) |
| Namespace-import lists | `solid_2.rs:6424-6473` (46 names), `:6476-6492` (16 names) | Every name is a runtime export of every client, dev and server bundle of its module on rc.13, as on rc.9 [M]. Three `solid-js` names (`createErrorBoundary`, `createLoadingBoundary`, `createRevealOrder`) are not declared at the root on rc.13, so `Solid.createErrorBoundary` is TS2339 there (§ 2.3) [M]. | U; X for those 3 (B3, N1 of § 5) |
| Where-exported tables | `exports/solid_v2_solid_js.rs` (`VALUES` 81, `TYPES` 61 rows), `exports/solid_v2_solidjs_web.rs` (175, 57) | These are frozen rc.3-era indices, already incomplete on rc.9 (see counts below), so a gap is not itself an rc.13 finding. The rc.9 -> rc.13 delta is listed below [M]. | X, I except the three above |
| Every callback-taking export is modelled or excluded | `solid_2.rs:7003`, `:7019-7060`; reads `lib.rs:3283` | The test reads the **bundled `rc.3` contracts**, not `node_modules` (`rust/crates/solid-dialect/contracts/solid-v2/*.json`: `solid-js` and `@solidjs/web` documents both `package.version` `2.0.0-rc.3`), and from them only 16 exports (`solid-js`: `For`, `Loading`, `Match`, `Repeat`, `Show`, `affects`, `createEffect`, `isPending`, `latest`, `refresh`; `@solidjs/web`: `applyRef`, `clientOnly`, `httpHeader`, `httpStatus`, `hydrate`, `render`) [M]. **It cannot fail on rc.13 bytes as long as the contracts are not regenerated, and it was already blind to `until`, `lazy`, `omit` and every rc.4-rc.9 addition.** The independent declaration scan below is the real answer. | X: B4 (conditional), N |

**Where-exported tables against rc.13 declarations** [M]
(`work/tablecmp.txt`, `tablecmp.json`):

- **`solid-js` table.** Claims `(name, module, kind)` that rc.13 declarations
  lack: 18, against 16 on rc.9. The two new misses are the types
  `ESMRuntimeType` and `StandardRuntimeType` in `solid-js/refresh`; the five
  values (`$DEVCOMP`, `sharedConfig`, `createErrorBoundary`,
  `createLoadingBoundary`, `createRevealOrder` at `solid-js`) were already
  misses on rc.9 (unresolved) and are now plain absences. The remaining
  baseline misses are `$REFRESH`, `NoHydrateContext`, `storePath`,
  `creationStamp`, `getProjectionTrace`, `inServerComponentScope`,
  `materializeContainerTrace`, `runInServerComponentScope`, `ssrHandleError`,
  `ssrScope` and the type `SolidStore` (the rc.9 review § 3.1 list, unchanged).
  Subpaths with no table rows: `solid-js/attribution`, `solid-js/internal`.
- **`solid-js` declared but absent from the table** (value `isStatic`,
  `isHydrating`, `isHydratable`, `TimeoutError`, `configureClientErrors`,
  `OBSERVE`, `resetErrorHalt`, and the diagnostic/record types): 46 names on
  rc.13 against 52 on rc.9. rc.13 *adds* `isHydrating`, `isHydratable`,
  `RecordSubscribeOptions`, `RecoveryEvent`, `RecoveryLive`,
  `RecoveryListener`; it *drops* 12 attribution types from the root.
  `until` is in the table (`exports/solid_v2_solid_js.rs:95`) and is declared
  on the root, unchanged.
- **`@solidjs/web` table.** Claims not declared: 41 on both versions, the same list (no new
  miss; `work/tablecmp.txt`). The list was not investigated: it is mostly
  server-function and frame names claimed for subpaths whose bundler-resolved
  declarations lack them, plus `JSX`/`ClassValue`, which are a namespace and
  a type that my symbol-flag check does not count as a type.
  Declared but absent from the table: 136 on rc.13 against 89 on rc.9; the
  47-name growth is the new additions in § 2.1 (`getHydrationWriter`,
  `takeHydrationValue`, `ssrElementAttribute`, the render/call event types, the
  server-functions event-stream names, `armLiveBody`, `FRAME_HAVE_*`,
  `AttributeSlot`, ...). One removal: the type `TraceSlot`. The new subpath
  `@solidjs/web/performance-tracks` has no table rows and is not in
  `modules()`.
- **"Moved" names [M].** The five `solid-js` root names above moved to
  `solid-js/internal`. `ChunkReader`, `createChunk`, `createEventChunk`,
  `frameAddress` are now also exported from `@solidjs/web/server-functions/server`
  (they were client-only on rc.9). No other table name changed module.

**Callback-taking exports** (a parameter whose type has call signatures,
resolved by the checker through unions and generic constraints;
`work/cbs.cjs` -> `cbs-rc{9,13}.json`, `cbsdiff.txt`, `cbs-direct.txt`) [M]:

- 119 exports across all subpaths have a directly callable parameter on rc.13,
  113 on rc.9. Of those at `solid-js`, `solid-js/internal` and the `@solidjs/web`
  root, every rc.9 name that is in `TABLE` or the exclusion list is unchanged.
- **New in rc.13 and not in `TABLE` or `UNMODELLED_CALLBACK_TAKERS`:**
  1. `ssrSanitizeError(value, subject?, site?, hook?)` in `solid-js/internal`,
     argument 3 `hook: ServerErrorHook` (`<r13js>/types/internal.d.ts`). A
     server error hook, `@internal`-style; `reportServerError` (rc.9) has the
     same shape and is also unmodelled.
  2. `armLiveBody(controller, teardown)` in `@solidjs/web/server-functions/server`,
     argument 1 `teardown: () => void`. Server only.
  3. `setConsoleFooter(footer)` in `@solidjs/signals`, a dev diagnostics hook
     (not an export of `solid-js` or `@solidjs/web`).
  4. `ssrElement` (`@solidjs/web` root, compiler-facing server helper) gains a
     callable `attrs?: string | (() => string)` at argument 5 (`<r13web>/types/server.d.ts:397`); the function already took `skip` on rc.9 and was already unmodelled.
- **New callback shape on a modelled name:** `dynamic`'s source may now return
  `AsyncIterable<T>` (`<r13web>/types/index.d.ts:112`), still argument 0; no new
  callback position.
- **Re-homed, modelled:** `createErrorBoundary`, `createLoadingBoundary`,
  `createRevealOrder` (`TABLE`, positions `[0, 1]`, `[0, 1]`, `[0]` at
  `solid_2.rs:4960` and the `[0]` list). The rc.13 declarations match those
  positions (above).
- **Pre-existing unmodelled direct-callback exports** (identical on rc.9; the
  contract-driven test does not see them): `createComponent`, `Dynamic`,
  `effect`, `insert`, `memo`, `ref`, `scope`, `spread`, `addEvent`, `ssr*`,
  `registerElementClaim`, `getNextElement` (`@solidjs/web`), `getNextChildId`,
  `isDisposed`, `isWrappable`, `enableExternalSource`, `configureClientErrors`
  (`solid-js`), `OmitView`, `ssrScope`, `runInServerComponentScope`,
  `reportServerError` (`solid-js/internal`). The completeness test's exclusion
  list names only three, so "modelled or excluded" is not true of these on any
  release; that is a test-scope fact, not an rc.13 change [M for the list,
  E for the reading of intent].
- **`isHydrating()` / `isHydratable()`** (new root exports) take no argument;
  `shareAsyncIterable(source)` takes an `AsyncIterable`, not a function; the new
  `performance-tracks` export takes an options bag with no callable members
  (`attribution`, `minMs`, `rich`) [M].

### 3.2 Declarations the dialect restates

Declaration text, comments stripped, `work/decls-rc{9,13}.json`,
`tablediff.json`. Of 147 (name, module) pairs checked (all `TABLE` names plus
the shape types listed below, in `solid-js`, `@solidjs/web` and
`@solidjs/signals`), **139 are textually identical, 8 differ** [M].

| assumption | source | evidence | class |
| --- | --- | --- | --- |
| `createSignal` -> `Signal<T> = [get: SourceAccessor<T>, set: Setter<T>]`; `createMemo` -> `SourceAccessor<T>` | `reactive_result_slot`, `solid_2.rs:5479` | `createSignal`, `createMemo`, `Signal`, `Accessor`, `SourceAccessor`, `Setter`, `Refreshable`, `SignalOptions`, `EffectOptions`, `ProjectionOptions`, `StoreOptions`, `StoreSetter`, `ProjectionStoreReturn`, `StoreReturn`, `ComputeFunction`, `EffectFunction`, `EffectBundle` declarations are textually identical [M]. `MemoOptions` gained `_plumbing?: boolean` (an underscore, observe-tier option; `computed` maps it to `CONFIG_PLUMBING`, `<r13sig>/dist/dev-shared.js`) and is otherwise identical, including `sync`, `lazy`, `equals`, `unobserved`, `loadingValue` [M]. | U; `MemoOptions` S, I |
| `For`/`Show`/`Match`/`Repeat`/`Switch` callback shapes per `keyed` form | `children_accessor_parameters`, `solid_2.rs:5418` | `flow.d.ts` sha256 differs (`81af6e73…148a` -> `963ee098…`, 11188 -> 12936 bytes) but the diff is **doc comments only** (the `Loading` section); stripping comment lines leaves the file identical, and every exported flow declaration (`For`, `Show`, `Match`, `Switch`, `Repeat`, `Loading`, `Errored`, `Reveal`, `MatchProps`, `KeyedMatchProps`, `RevealProps`) is textually identical [M]. For rc.13's keyed form `For` still passes `item` plain and `index` as `Accessor<number>` (`flow.d.ts:36`) [M]. | U (code); comment text records a behaviour change, § 5 N2 |
| `createStore` root is `Readonly` (root writes belong to `tsc`) | `store_root_properties_are_readonly`, `solid_2.rs:5204` | **`Store<T> = T`** on rc.13 (`<r13sig>/dist/types/store/store.d.ts:4`), as on rc.9 [M]. `tsc` is silent on `st.a = 2`, `proj.a = 5`, `opt.a = 3` (§ 2.3). The runtime drops the root write outside a setter: probe gives `a=1` after `s.a = 99` on rc.9 and rc.13, dev and prod [M]. | **U vs rc.9: B1 answer `Mutable` still correct** |
| `options_argument`: memo/signal/optimistic/trackedEffect -> 1; store/projection/optimisticStore/effect/renderEffect -> 2 | `solid_2.rs:5625` | `createSignal`, `createMemo`, `createOptimistic`, `createTrackedEffect`, `createStore`, `createProjection`, `createOptimisticStore`, `createEffect`, `createRenderEffect` declarations are textually identical [M]. | U |
| `supports_sync_option` for the signal family only | `solid_2.rs:5650` | `computed` still maps `options?.sync` -> `CONFIG_SYNC` (`<r13sig>/dist/dev-shared.js`, `computed` slice, diff adds only the `_plumbing` bit and `_name`); `ProjectionOptions` still has no `sync` (declaration identical) [M]. | U |
| `createEffect(compute, apply)`; one-argument form is a type error | `effect_api.rs:176-181` | One overload on rc.13 (`signals.d.ts:384`); `tsc` TS2554 on one argument (§ 2.3). Runtime: one-argument `createEffect` throws `MISSING_EFFECT_FN` in dev and crashes in prod on both versions (probe) [M]. | U |
| `omit(props, ...keys)`; predicate form (B3) | `splits_props` `solid_2.rs:6130`, `callback_runs_on_result_access` `:6171` | `omit` declarations identical, both overloads, including `omit(props, hidden: (key) => boolean)` (`<r13sig>/dist/types/store/utils.d.ts:261`) [M]. Runtime selects the predicate by `keys.length === 1 && typeof keys[0] === "function"` (`<r13sig>/dist/dev.js:4550`), as on rc.9 (`:4380`) [M]. Probe: `omit({a,b,c}, k => k === "a")` -> `b,c`, `omit(..., "a")` -> `b,c` on both versions, dev and prod [M]. The `omit` slice changed in two ways: `$VIEW`/`$OMIT` lookups became `recordOf(props)`/`$RECORD`, and the non-Proxy copy path now re-homes accessors with the source as receiver (`get: desc.get && desc.get.bind(props)`) [M]; rule effect [E]: none. | U (B3 answer `true` still correct) |
| `dynamic(source)` / `dynamic(source, { static: true })` (B2) | `callback_positions` `solid_2.rs:4950`, `dynamic_call_forms_*` `:6726`; releases `DynamicOptions` | `if (options?.static) return staticDynamic(untrack(source));` is still the first statement of the client builds: `<r13web>/dist/web.js:2062`, `web.dev.js:2297`, `web.observe.js:2110`; and the server builds have the same `options?.static` branch (`server.js:4281`, `server.dev.js:4696`, `server.observe.js:4436`) [M]. `staticDynamic` is byte-identical in dev and prod [M]. Declaration changes: source may return `AsyncIterable<T>`, `DynamicOptions` unchanged otherwise [M]. The non-static path was rewritten (below). | U for the static form (B2 answer `StaticForm` still correct); S, I for the rest |
| `refresh(target)` | SC2001 refresh arm | `refresh` declaration identical (`Promise<...>`, `signals.d.ts:551`) [M]. | U |
| `render`/`hydrate(fn, el, init?, options?)` | `callback_positions`/`callback_owners` for mount | `render` and `hydrate` declarations identical (`<r13web>/types/client.d.ts:144`, `:245`); `render` and `hydrate` slices byte-identical in `web.js` and `web.dev.js` and `web.observe.js` [M]. | U |
| `until(fn, options?)` (B4) | `Primitive::Until`, `RELEASE_GATED_NAMES` `solid_2.rs:193` | Declared `until<T>(fn: () => T, options?: UntilOptions): Promise<Truthy<T>>` (`<r13sig>/dist/types/signals.d.ts:642`), re-exported by the `solid-js` root (`types/index.d.ts:1`); `until` slice **byte-identical** in the dev bundles; probes: throws `Cannot call until inside a reactive scope` in a memo, and `fn` runs during the call (`ran=true`), both versions [M]. `UntilOptions.signal` is now `GlobalAbortSignal` (a conditional alias to the global `AbortSignal` when a lib declares it, else a structural stand-in, `signals.d.ts:559-577`) [M]; rule effect [E]: none. | U (B4 answer `true` still correct); S, I for `UntilOptions` |
| Boundary primitives' callback shapes | `callback_positions` `solid_2.rs:4960`; `callback_owners` `:5302` | New declarations in `solid-js/internal` (§ 2.3) agree with the positions the dialect restates from rc.9 runtime reading: boundaries `(fn, fallback)`, loading boundary's third argument `{ on?: () => any }`, reveal order `(fn, { order?, collapsed? })` [M]. The nested option thunks (`on`, `order`, `collapsed`) are not callback positions in the dialect on any release [M for the code, E for effect]. | U |

Slice evidence for the runtime shapes the rows assume, signals dev builds
(`work/slicecmp2.cjs`) [M]: **byte-identical** `devGuardStoreSetterWrite`
(`<r13sig>/dist/dev-shared.js:6650`), `until`, `runWithOwner`, `createRoot`,
`resolve`, `onSettled`, `createTrackedEffect`, `trackedEffect`, `effect`,
`cleanup`, `onCleanup`, `latest`, `createReaction`, `mapArray`, `repeat`,
`createSignal`, `createMemo`, `createOptimistic`, `refresh`, `snapshot`,
`getOwner`, `createRenderEffect`, `createEffect`, `enqueueSub`. **Differ**:
`flush` (adds an attribution `flushStart` hook), `omit`, `untrack` (adds
`asyncTailFlights === 0` to the fast path and an `untrackDepth` counter),
`computed`, `recompute` (33,292 -> 39,072 bytes, not read), `setSignal`,
`isPending` (the suppressed-probe enrolment now requires `tracking`),
`createOwner`/`setupComputedNode` (`linkChild`, `registerRoot`), `createStore`
(`nameStore`), `action` (adds `enterCallback`/`exitCallback`), `merge`
(refactor to `recordOf`), `schedule` (withholds the microtask for a projection
draft write), `runEffect`. Of these only `untrack`, `omit` and `isPending` were
read for semantics; the rest were read only as far as the diff headers [M for
the diffs, E for "no rule effect"].

Runtime probes on `@solidjs/signals` (`probe-rt.mjs`), **identical on rc.9 and
rc.13 in dev and in prod** [M]:

| case | dev | prod |
| --- | --- | --- |
| store setter directly in a `createRoot` body (N3) | throws `REACTIVE_WRITE_IN_OWNED_SCOPE` | no throw |
| signal setter directly in a `createRoot` body | throws | no throw |
| `s.a = 99` outside a setter (B1) | dropped (`a=1`) | dropped |
| `createStore` / `createOptimisticStore` setter in a memo compute (N5) | throws | no throw |
| `until` in a memo compute | throws | no throw |
| `flush()` inside an action body (N4) | throws `FLUSH_IN_ACTION` | `flush ok` |
| `createEffect(() => 1)` one argument | throws `MISSING_EFFECT_FN` | crashes on `.effect` |
| `createMemo` compute, `createRenderEffect` compute/apply, `createTrackedEffect`, `flush(fn)`, `until(fn)` timing | compute during call; `c,a,ret`; `ret`; inline; fn during call | same |

### 3.5 `@solidjs/web` entry points and the compiler helper runtime

Slices compared by name and sha256 (`work/slices.cjs`) [M]:

| helper | `web.js` (prod) | `web.dev.js` | `web.observe.js` | note |
| --- | --- | --- | --- | --- |
| `insert` | identical | **differs** | **differs** | dev/observe add `spreadName` auto-naming; dev adds `unscopedHoleSnapshot`/`checkUnscopedHole` (`web.dev.js:1028`); the `effect(...)` calls inside are unchanged |
| `memo` | identical | identical | identical | `createMemo(() => fn(), syncOptions)` |
| `template` | identical | identical | identical | |
| `ref` / `applyRef` | identical | identical | identical | `ref` = `untrack(fn)` then `runWithOwner(null, () => applyRef(...))` |
| `delegateEvents` | identical | identical | identical | |
| `effect` | identical | **differs** | **differs** | the diff adds only the `spreadName` name option; `createRenderEffect(fn, effectFn, nodeOptions)` with `sync: true` and `transparent: !options.scope` is unchanged (prod slice quoted in full) |
| `spread` | **differs** | differs | differs | gains a trailing `name` parameter (`spread(node, props, skipChildren, skip, name)`); prod body differs only in the signature, dev adds the `setSpreadName` wrapper and `unscopedByDesign` markers |
| `addEvent` | identical | identical | identical | returns the listener, as on rc.9 |
| `clientOnly`, `useHead`, `httpHeader`, `httpStatus` | identical | identical | n/a | also identical in web `dist/server.js` |
| `render`, `hydrate` | identical | identical | identical | |
| `dynamic` | differs | differs | differs | static path unchanged (§ 3.2); default path rewritten, below |
| `insertExpression`, `eventHandler` | differs | differs | n/a | `current != null` instead of truthiness for node replacement; `eventHandler` drops the `_bnd` seam fallback |
| `getNextElement` | identical | differs | n/a | dev adds a `hydrationKeyMisses++` counter |

- **`dynamic`, default path** [M, `<r13web>/dist/web.js` from line 2061, diff]: the source is
  still read in a lazy `createMemo` (`cached`), but the memo now has `equals:
  sameInstance`, carries `AsyncIterable` sources through a `[FLIGHT]` slot, and
  the returned component creates **two** memos per instance (`value`, then the
  render memo) plus a `ownedWrite` address signal; under hydration it first
  reads `untrack(cached)`. The source is still read tracked inside a memo owned
  by the instance [M]; the dialect's `(0, Tracked)` / `Creates` answer for
  `Dynamic` (`callback_executions`/`callback_owners`) is unchanged by this
  [E, from the slice diff, not executed on a DOM].
- **Compiler helper surface** [M]: of the 40 helper names the pinned compiler
  (`yumemi-thomas/solid` `9f9a84b`, `packages/compiler/src`) imports by name
  (extracted with a regular expression, so approximate), the set missing from
  `@solidjs/web`'s client export list and from its server export list is
  **identical on rc.9 and rc.13**. No helper was removed. New helper
  parameters are additive: `spread`'s `name`, `ssrElement`'s `attrs`.
  Whether the pinned `9f9a84b` compiler (rc.3-era) emits what rc.13 runtime
  expects, and the `compiler-rc13-rebase` checkout under
  `~/.cargo/git/checkouts/` (not read), are separate questions (§ 6).
- **Entry points** [M]: the `@solidjs/web` root client bundle exports 120 names
  (117 on rc.9): the three additions are `getHydrationWriter`,
  `ssrElementAttribute`, `takeHydrationValue`. None was removed from the
  client or server roots except the server's `CLAIM_PROP`.

### 3.6 `solid-js` `dist/server.js` bodies

rc.9 vs rc.13, `solid-js/dist/server.js` (and, in parentheses, `server.dev.js`
and `server.observe.js`, which agree unless noted) [M]:

- **Byte-identical:** `createEffect`, `createRenderEffect`, `createRoot`,
  `createTrackedEffect`, `onSettled`, `untrack`, `createSignal`,
  `runWithOwner`, `createComponent`, `getOwner`, `onCleanup`, `createOwner`,
  `flush`, `resolve`, `latest`, `isPending`, `createReaction`,
  `createRevealOrder`, `Show`, `For`.
- **Changed:**
  - `serverEffect`: `source === CLIENT_HOLE` / `!== CLIENT_HOLE` became
    `isClientHole(source)` / `!isClientHole(next)`. Nothing else in the slice
    moved, including the use of `effectFn` that SC7001 cites [M].
  - `createMemo`: the disposal `flag` now runs `comp.onDisposed` hooks [M].
  - `mapArray`, `repeat`: `rowOwner.id` -> `ownerId(rowOwner)` [M].
  - `createLoadingBoundary`: passes `options?.on !== undefined` to
    `ssrLoadingBoundary`. `createErrorBoundary`: `ownerId(...)` and passes the
    render's `errorPolicy` to the sanitiser. `Loading`, `Errored`, `Reveal`
    differ (`Errored`: see N2) [M].

Rule effect [E]: none for SC7001 ("`serverEffect` ignores the apply
argument"), whose premise slice moved only in its client-hole test.

## 4. Items that need a dialect change (B-items)

All six `vocabulary_for` answers were measured on rc.13 bytes and equal
rc.9's (§ 3.2); the B-items are the consequences of the dialect not knowing
that.

### B1. rc.13 is an unread release, so it gets the conservative vocabulary

**What the code does [M, reading].** `Release::of` accepts only
`2.0.0-rc.N` with `N <= NEWEST_READ` (`releases.rs:546-560`, `NEWEST_READ = 9`
at `:447`). rc.13 therefore parses as `Release::Unread`, and
`vocabulary_for` (`releases.rs:591-650`) returns every field's `#[default]`:

| answer | line | rc.13 gets | rc.13's bytes say | effect today [E] |
| --- | --- | --- | --- | --- |
| B1 `store_root` | `:594` | `Readonly` | `Store<T> = T`, `tsc` silent, runtime drops the write | SC2003 suppresses the root write for `tsc`, which does not report it: **neither reports it** |
| B4 `until` | `:605` | `false` | exported, declared, throws in a reactive scope | name is not a vocabulary name: `await until(...)` certifies (the rc.9 review's B4 failure) |
| B2 `dynamic_options` | `:612` | `Unread` -> `DynamicUnknownForm` for any option-bearing call | `options?.static` selects `staticDynamic(untrack(source))` | `dynamic(src, { static: true })` is "states nothing"; not wrong, but loses the finding |
| B3 `omit_predicate_form` | `:621` | `false` | predicate form present, invoked on read | a callable `omit` argument is dropped from callback analysis |
| N3 `store_setter_roots` | `:626` | `Exempt` | guard rejects a root (probe, dev) | store setter in a `createRoot` body not reported |
| N4 `flush_in_action` | `:638` | `false` | `FLUSH_IN_ACTION` present (`dev-shared.js:2656`) | the throw is not claimed |
| N5 `optimistic_store_setter` | `:643` | `Guarded` | guarded (probe) | none, same answer |

**Suggested change.** Treat rc.13 as a read release for each owner with the
rc.9 answers (it needs the same row in the table at the top of `releases.rs`
and in `Solid2::RC9`'s doc comment, and a test per answer like the existing
`("2.0.0-rc.9", ...)` cases at `releases.rs:1145-1213`).

**Trap, [M] for the code, [E] for the consequence.** B2, B3 and N3 are
written as an **equality** on the newest-read release
(`Release::Read(NEWEST_READ)` at `releases.rs:613`, `:621`, `:627`; the three matches), and
`Release::of` accepts every `N <= NEWEST_READ`. Raising `NEWEST_READ` to 13 on
its own would (a) make those three answers `false`/`Ignored`/`Exempt` for rc.9,
a regression on the audited release, and (b) make rc.10, rc.11 and rc.12 parse
as `Read`, i.e. "reviewed", although this review did not read them. The read
set has to be an explicit list (rc.9 and rc.13, plus the earlier ones), and the
three equalities have to become membership tests on it. Also
`AUDITED_INSTALLATION` (`releases.rs:120-124`) and `Solid2::AUDITED`
(`:269`) name rc.9 and are what `SC9014` tells a user to pin; whether the
audited triple should move is the owner's decision, not this review's.

### B2. No archive row, so no negative-claim authority for rc.13

`AUDITED_ARCHIVES` (`solid_2.rs:239-282`) has no rc.13 entries, and rows are
archive-scoped, so certification of an rc.13 tree closes fewer claim domains
than on rc.9 (the rc.9 review § 7 states the same mechanism) [M for the
code, E for the consequence]. Tuples a re-audit would need (the manifest digest
is the `package.json` sha256 of § 1; integrity is the lockfile's claim, not
re-derived):

| name | version | integrity | `manifest_sha256` |
| --- | --- | --- | --- |
| `@solidjs/signals` | `2.0.0-rc.13` | `sha512-4+pRdrAH…MYeQ==` (§ 1) | `6783c3c632cfebf3a0fc26e18925887462b5925c481c6535ae0186845efb95a6` |
| `@solidjs/web` | `2.0.0-rc.13` | `sha512-vI/7v/XM…OQ==` (§ 1) | `8b45ed71ed7a369883e7e00bb48b01fd7bda935bd5ba889cbecafc8eaa122bf5` |
| `solid-js` | `2.0.0-rc.13` | `sha512-62bYOI4J…QdoQ==` (§ 1) | `ce43a022f763e5995d1ac8a5f78f135804edd01b627a2bcbee8ff9530e79d284` |

What the bytes say about carrying the five rc.9 `creates` rows
(`getOwner`, `onCleanup`, `createRoot`, `untrack`, `runWithOwner`) [M, slice
comparison of `@solidjs/signals`]:

- **dev** (`dev.js`+`dev-shared.js`): `getOwner`, `onCleanup`/`cleanup`,
  `createRoot`, `runWithOwner` byte-identical; **`untrack` changed** (extra fast-
  path condition `asyncTailFlights === 0` and an `untrackDepth` counter) and
  `createOwner` changed (`linkChild`, `registerRoot`).
- **prod** (`dist/prod/**`, all files): `getOwner`, `onCleanup`, `createMemo`,
  `createTrackedEffect`, `latest`, `createStore`, `reconcile`, `snapshot` are
  byte-identical. `cleanup`, `createRoot`, `runWithOwner` differ **only by
  minifier alpha-renaming** (read in full: `ke` -> `qe`, `e,t,n` -> `e,n,t`).
  `untrack`, `createOwner`, `setupComputedNode`, `createSignal`, `flush` and
  others differ in content.
- **observe** (`dist/observe/**`): `getOwner`, `createRoot`, `runWithOwner`,
  `onCleanup`, `createSignal`, `createMemo`, `createTrackedEffect` identical;
  `cleanup`, `untrack`, `createOwner`, `flush`, `createStore`, `action` differ.
- **Not read:** every other negative row (the 20 rc.9 parity rows, the 26
  `solid-js`/`@solidjs/web` rows), and the content of the differing prod and
  observe slices beyond the three alpha-renames above.

So the cheap candidates are the same as in the rc.9 review § 7 minus
`untrack` and `createOwner`, which need a reading.

### B3. Three moved primitives: `solid-js/internal` is not an owned module

`export_modules` (`solid_2.rs:6373`) answers `["solid-js"]` for
`createErrorBoundary`, `createLoadingBoundary`, `createRevealOrder`
(`exports/solid_v2_solid_js.rs:42` and the two neighbouring rows), and
`modules()` (`solid_2.rs:4804-4821`) does not own `solid-js/internal`. On
rc.13:

- `import { createErrorBoundary } from "solid-js"` is TS2305 (both
  `skipLibCheck` modes), so that code is `tsc`'s, and any checker finding on it
  must be a different claim [M]. This is the absolute rule at work, not a gap.
- `import { createErrorBoundary } from "solid-js/internal"` type-checks and
  binds to **nothing** in the dialect, so a callback passed to it is not
  classified [M for the type-check, E for the binding: the import loop at
  `solid-facts-backend/src/lib.rs:1899` skips a module `owns_module` rejects].
- The rc.9 `KNOWN_GAPS` entry for these five (`releases.rs:491-509`, scoped to
  the root specifier) describes a defect rc.13 does not have; rc.13's
  situation is the different one above.

**Suggested change.** Own `solid-js/internal` for exactly these three names
(and `sharedConfig`/`$DEVCOMP` if they matter), keeping the root rows because
the runtime root still exports them and older code still imports them. Fail-
closed today: findings are lost, none are wrong.

### B4 (conditional). Completeness criterion for new callback takers

The test cannot fail on rc.13 as the repository stands (§ 3.1); if the bundled
contracts are regenerated against rc.13, these new exports would need a
decision (model or add to `UNMODELLED_CALLBACK_TAKERS` with a reason):
`armLiveBody` (`@solidjs/web/server-functions/server`, `teardown`),
`ssrSanitizeError` (`solid-js/internal`, `hook`), `setConsoleFooter`
(`@solidjs/signals`, `footer`), and `ssrElement`'s `attrs`. All four are
server-side, diagnostic or compiler-facing; none runs reactive user code
[E]. The three re-homed boundary primitives are already modelled.

## 5. Changes that need no dialect change but should be recorded (N-items)

- **N1. The N2 defect is fixed; the names moved and became `tsc`'s [M].**
  Zero errors over the whole published surface with `skipLibCheck: false`
  (§ 2.3). The five names are declared at `solid-js/internal` and are TS2305
  (named import) / TS2339 (namespace member) at the root. The rc.9 observation
  that `createErrorBoundary((err) => ...)` was untyped (TS7006) no longer
  applies. `NAMESPACE_SOLID_JS` (`solid_2.rs:6424`) still lists the three
  boundary primitives, so a namespace call to one on rc.13 is both a `tsc`
  error and a (different-claim) checker primitive; nothing to change.
- **N2. Boundary semantics changed in places the dialect does not state [M
  for the bytes, E for effect].**
  - `<Loading>`'s `on` prop: rc.9 docs said it "scopes the boundary so it
    ignores transitions caused by writes to other reactive sources" with
    `on={route}`; rc.13 docs say it is a **tracked dependency list**
    (`on={route()}` or `on={[query(), page()]}`), re-showing the fallback when
    anything it reads changes (`<r13js>/types/client/flow.d.ts`, `Loading`
    section). The declaration is textually unchanged (`on?: ...`), so `tsc`
    cannot tell the two uses apart. Code written for the rc.9 meaning can
    silently change behaviour, and the dialect states nothing about `on`
    (boundary kind only, `solid_2.rs:5277`).
  - `<Errored>`: a function fallback is now always called as
    `f(err, reset)`; rc.9 returned a function with zero declared parameters
    as the fallback value itself (`typeof f === "function" && f.length ? ... :
    f` -> `typeof f === "function" ? ...`, `solid-js/dist/solid.js` slice
    `Errored`).
  - `lazy` now renders through `createComponent(Comp, props)`
    (`untrack(() => Comp(props || {}))` in prod) instead of an inlined
    `untrack(() => Comp(props))`; same owner and tracking in prod, and the
    dev wrapper tags the component through `createComponent` [M for the
    slice, E for equivalence].
- **N3. `dynamic`'s default path is restructured [M]** (§ 3.5): `AsyncIterable`
  sources, `sameInstance` equality, two memos per instance. The static form,
  which is what B2 models, is unchanged.
- **N4. New runtime diagnostic codes [M].** `DiagnosticCode` goes from 52 to
  63 members: added `LOADING_ON_OUTSIDE_HOLD`, `UNTRACKED_READ_AFTER_AWAIT`,
  `GRAPH_GROWTH`, `WASTED_RECOMPUTE`, `UNTRACKED_ASYNC_HANDLER`,
  `ABANDONED_FLIGHTS`, `FALLBACK_FLASH`, `STACKED_HOLDS`, `OPTIMISTIC_REVERTED`,
  `SERVER_ERROR_SANITIZED`, `SSR_BOUNDARY_WATERFALL`,
  `SSR_UNDECLARED_LIVE_SOURCE`, `UNSCOPED_HOLE_ALLOCATED_IDS`,
  `ATTRIBUTE_SLOT_POSITION`, `DYNAMIC_ASYNC_COMPONENT`; removed `WIDE_WRITE`,
  `SERVER_FN_ERROR_SANITIZED`, `SSR_ERROR_SANITIZED`, `BEHAVIOR_CLAIM_DROPPED`.
  The 12 codes the checker's rule catalog and rule pages cite that a plain
  string search can find (`STRICT_READ_UNTRACKED`, `REACTIVE_WRITE_IN_OWNED_SCOPE`,
  `PENDING_ASYNC_UNTRACKED_READ`, `FLUSH_IN_ACTION`, `SETTLED_CLEANUP_UNOWNED`,
  `ASYNC_OUTSIDE_LOADING_BOUNDARY`, `ACTION_CALLED_IN_OWNED_SCOPE`,
  `SYNC_NODE_RECEIVED_ASYNC`, `PRIMITIVE_IN_FORBIDDEN_SCOPE`, `NO_OWNER_EFFECT`,
  `MISSING_EFFECT_FN`, `CLEANUP_IN_FORBIDDEN_SCOPE`) are each present in the rc.13
  dev bundles as in rc.9's [M]. The new ones are rule opportunities, not
  compatibility items.
- **N5. New surface with no dialect treatment, correctly [M].**
  `@solidjs/web/performance-tracks` (one export, prod build a no-op),
  server-component slots (`slotValue`, `SLOT_*`, `AttributeSlot`,
  `ATTRIBUTE_SLOT_POSITION`), hydration value channel (`getHydrationWriter`,
  `takeHydrationValue`), server-function event streams, frames budget flags,
  and `skills/server-components/SKILL.md`. None of the exports has a callable
  parameter except `armLiveBody` and the `ssrElement` `attrs` (B4).
- **N6. `isHydrating` / `isHydratable` are new root exports [M]** (nullary
  predicates), and `inLiveServerComponentScope`, `shareAsyncIterable` are root
  runtime exports declared only at `solid-js/internal`. The four names in the
  brief that are not exports are in § 2.1.
- **N7. Typing-only changes [M].** `MemoOptions._plumbing`;
  `UntilOptions.signal: GlobalAbortSignal` (now checks without a DOM lib);
  `dynamic` source accepts `AsyncIterable`; `ssrElement` `attrs`; `spread`
  `name`; the attribution/record diagnostic types reshaped. None is read by a rule [E].
- **N8. `omit` and `merge` refactors [M].** Both now go through
  `recordOf`/`$RECORD`; `omit`'s non-Proxy copy path re-homes accessors with
  the source as receiver. Probes on `omit` equal rc.9 [M]; rule effect [E]:
  none.
- **N9. Prod bundles are minified, so slice hashes overstate change [M].** Three
  of the five `creates` slices differ only by alpha-renaming (B2). A future
  re-audit should compare after normalising names, or read the diffs.
- **N10. Runtime probes agree on every timing, tracking, ownership and guard
  premise tested [M]** (§ 3.2 table, 15 cases, dev and prod).

## 6. Not done, and what could not be measured

- **rc.10, rc.11, rc.12** were not provided; nothing here covers them, and
  the `Release::Read` arithmetic in B1 must not let them read as reviewed.
- **No dialect code was executed.** B1's "effect today" column, B3's binding
  claim and all "effect" statements are [E]: they come from reading
  `releases.rs`, `solid_2.rs` and `lib.rs`, not from running the checker on
  rc.13 (cargo, make and rustc were off limits). No fixture, snapshot, test or
  gate was run, including the completeness test, which was *read* (it reads the
  bundled rc.3 contracts) rather than run.
- **Integrity and authenticity.** The rc.13 integrities are the lockfile's
  values; no tarball was downloaded or re-derived (no network), so tarball
  identity is not established. The rc.13 tree under `audit-rc13/node_modules`
  is not compared with `benchmarks/package-contract-v2/phase0/` records
  (none for rc.13 were looked for).
- **`seroval` caveat.** The rc.9 archive has no `seroval`; to import rc.9's
  server bundles for key listing I used rc.13's `seroval`/`seroval-plugins`
  1.6.8 in the `work/rc9` copy. Export key lists do not depend on it; the
  rc.9 dependency pin is `~1.6.7`.
- **DOM and server execution.** `dynamic`'s static form and `Loading`/
  `Errored` were compared by bytes only; no DOM or SSR render was run. Only
  `@solidjs/signals` was probed at runtime.
- **Not read in full:** `recompute` (33,292 -> 39,072 bytes), `setSignal`'s
  later hunks, `action`, `merge`, the observe and prod tiers beyond slice
  hashes and the three alpha-rename diffs, the `observe` builds of the web and
  `solid-js` packages, `frames/**`, `server-functions/**`, `serialization/**`
  beyond export lists, and the reactivity-diagnostics skills.
- **Regex-derived helper list.** The 40 compiler helper names were extracted
  from the pinned compiler source by pattern, so the list is approximate; its
  rc.9/rc.13 equality is [M], its completeness is [E]. The unpinned
  `compiler-rc13-rebase` checkout was not read.
- **Callback scan scope.** The declaration scan finds parameters whose type has
  call signatures (direct) and, one level down, object parameters with callable
  members; the nested list is noisy (DOM/Response members) and is in
  `work/cbs-rc13.json` only. Untyped or `any` callbacks are invisible to it.
- **Template sections not redone:** § 3.3 (timing/tracking/ownership: only the
  15 probes and slice hashes above), § 3.4 (guard line numbers: only the
  12-code presence check), § 6 (compiler), § 7 (negative rows: B2 only),
  § 8 (experimental).

## Summary: items that need a dialect change

| # | item | evidence | status |
| --- | --- | --- | --- |
| B1 | rc.13 parses as `Release::Unread` (`releases.rs:546-560`, `NEWEST_READ = 9`), so all six answers revert to the conservative vocabulary; all six were measured equal to rc.9's on rc.13 bytes. The three `Release::Read(NEWEST_READ)` equalities (`:613`, `:621`, `:627`) must become a read-set test, or rc.9 regresses and rc.10-12 read as reviewed. | § 3.2, § 4 | [M] bytes/probes, [E] effect |
| B2 | no rc.13 rows in `AUDITED_ARCHIVES` (`solid_2.rs:239`); tuples supplied. `untrack` and `createOwner` changed in content, so those rows need a re-reading; `getOwner`, `onCleanup`, `createRoot`, `runWithOwner` are identical (dev) or alpha-renames (prod). | § 4 | [M] |
| B3 | `createErrorBoundary`/`createLoadingBoundary`/`createRevealOrder` moved to `solid-js/internal`, which `modules()` (`solid_2.rs:4804`) does not own and the export tables do not list; root import is TS2305 on rc.13. Fail-closed loss. | § 2.3, § 4 | [M] |
| B4 | conditional: `every_callback_taking_export_is_modelled_or_excluded` reads rc.3 contracts and cannot fail on rc.13 today; if regenerated, `armLiveBody`, `ssrSanitizeError`, `setConsoleFooter`, `ssrElement.attrs` need a decision. | § 3.1 | [M] |

## Summary: items recorded without a change

| # | item |
| --- | --- |
| N1 | N2 of rc.9 fixed: 0 `tsc` errors on the whole surface with `skipLibCheck: false` (rc.9: 5); the five names are now `solid-js/internal` declarations and root TS2305/TS2339. |
| N2 | `<Loading on>` is a tracked dependency list (was a transition scope); `<Errored>` always calls a function fallback; `lazy` goes through `createComponent`. Declarations unchanged. |
| N3 | `dynamic` default path restructured (two memos per instance, `AsyncIterable`); static form unchanged. |
| N4 | `DiagnosticCode` 52 -> 63 (15 added, 4 removed); the 12 cited codes still present. |
| N5 | new surface without a dialect treatment: `performance-tracks`, server-component slots, hydration value channel, event streams. |
| N6 | `isHydrating`/`isHydratable` added; `$DEVCOMP` leaves the root *declarations* only (still a root runtime export); `callerRenderContext`, `devPeekNextChildId`, `installServerWithOrigin`, `ownerId` are not exports. |
| N7 | typing-only changes (`MemoOptions._plumbing`, `UntilOptions.signal`, `spread`/`ssrElement` parameters). |
| N8 | `omit`/`merge` refactors; probes equal rc.9. |
| N9 | prod bundles are minified; compare after alpha-normalising. |
| N10 | 15 runtime probes identical on rc.9 and rc.13, dev and prod. |
