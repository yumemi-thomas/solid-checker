## 6. Compiler (rc.9 to rc.13)

Tags: [M] measured (command or file read in this audit), [E] estimated or
inferred. Scratch and extraction scripts are in
`rust/target/audit-rc13/compiler/work/` (`exports.mjs`, `slices.mjs`,
`helpers-A.txt`, `helpers-B.txt`, `slices-summary.txt`, `tar/`).

### 6.0 Current pins [M]

- `rust/Cargo.toml:37`: `solidjs-compiler = { git =
  "https://github.com/yumemi-thomas/solid", rev =
  "9f9a84b2f08bdf7a67049f16bc56b05af6ca49d4" }`; `rust/Cargo.lock:1699` agrees.
- `rust/dialects/solid-v2/compiler/src/lib.rs:29-31`: upstream
  `a10cf1a147209d8da50697896742d2b1d4afad75`, implementation
  `7f4e1135943c1fb01231d1bda707b4a1856a5607`, distribution
  `9f9a84b2f08bdf7a67049f16bc56b05af6ca49d4`; line 37 binds trace 3.
- `docs/package-contract-v2/phase4/compiler-identity.json`: same three
  revisions, `semanticTraceVersion` 3, `compilerFactsProtocol` 2, branch
  `solid-checker/compiler-facts-v3`.
- The pinned checkout (`~/.cargo/git/checkouts/solid-6c05d7779c3c2780/9f9a84b`)
  has `packages/compiler/package.json` version `2.0.0-rc.3` and
  `packages/web` version `2.0.0-rc.3`. The pin is an **rc.3-era** compiler
  and has not moved for rc.9.

**A local rc.13 rebase candidate already exists [M].**
`~/.cargo/git/checkouts/compiler-rc13-rebase-a366210d61261a48/3ad4bbe`
(`@solidjs/compiler` version `2.0.0-rc.13`). It is the distribution commit
`3ad4bbec37ae30f325a803cdb4271a71c86a2a2d` on upstream published RC.13
`5efaf260becb32293f2bcb4d32f8be72be6de674`, implementation merge
`c04c4877...`. It is documented in
`docs/package-contract-v2/phase22/2026-10-03-rc13-compiler-facts-rebase.md`
and `benchmarks/compiler-facts/rc13/` (patch, bundle, evidence). It is not
referenced by `rust/Cargo.toml` and was never pushed (report: "The branch is
local; no remote push occurred"). The report claims 3,285/3,285 upstream/fork
output matches and 3,285/3,285 against the published darwin-arm64 native
compiler; those figures are the report's, not re-measured here.

### 6.1 Runtime helper interface (Q1) [M]

Method: helper names taken from every `import_named(...)` and
`import_wrapper_helper(...)` literal under `packages/compiler/src` in the
pinned fork (`helpers-A.txt`, 41 literals) and in the rc.13 candidate
(`helpers-B.txt`, 42 literals), plus the variable-named wrappers `memo`,
`effect` (default names; `src/dom/template.rs:166-210`) and `patchDriver`,
plus the 10 default built-ins (`src/compiler.rs:49-51`: For, Show, Switch,
Match, Loading, Reveal, Portal, Repeat, Dynamic, Errored). Exports were
extracted from all six `@solidjs/web` bundles (client dev, prod, observe;
server dev, prod, observe) for rc.9 and rc.13 with `exports.mjs`.

Note: the rc.9 audit said "40 helpers"; counting literals plus the three
wrapper names gives 44 distinct names in the pinned fork (plus 10 built-ins).
The difference is counting, not content.

Result for every helper the **pinned fork** can emit:

- Exported by rc.13 wherever rc.9 exported it, in all six bundles. No export
  was removed or renamed among them. Per-bundle table: run
  `node exports.mjs helpers`.
- Not exported by `@solidjs/web` in **either** rc.9 or rc.13 (so unchanged):
  `createElement`, `createTextNode`, `insertNode`, `setProp`, `patchDriver`
  (universal renderer or opt-in patch driver; the adapter emits DOM or SSR
  only, `rust/dialects/solid-v2/compiler/src/lib.rs:137-145`), and `rowProof`
  (dormant opt-in, also absent in rc.9).
- `getFirstChild` and `getNextSibling` are client-only in both versions.
- `sharedConfig`, `ssrClaim`, `ssrSelectValues` are server-only in both.

Export-set deltas rc.9 to rc.13 (complete):

| Bundle | Added | Removed |
| --- | --- | --- |
| web.js / web.dev.js / web.observe.js | `getHydrationWriter`, `ssrElementAttribute`, `takeHydrationValue` | none |
| server.js / server.dev.js / server.observe.js | `SLOT_FACE_DATA`, `SLOT_FACE_MARKUP`, `SLOT_FACE_STREAM`, `SLOT_MARKER`, `SLOT_VALUE`, `getHydrationWriter`, `isSlotValue`, `slotValue`, `ssrElementAttribute`, `takeHydrationValue` | `CLAIM_PROP` |

`CLAIM_PROP` is the only removal. The pinned fork does not reference it
(`grep -rn CLAIM_PROP packages/compiler/src` is empty in both checkouts), so
no pinned emission is affected. **Missing or renamed helpers: none.**

### 6.2 Semantics of the helpers the checker models (Q2) [M]

Function slices (from `function NAME(` to the next column-0 closing brace)
compared byte for byte, `slices.mjs summary`. Signatures in rc.13:
`insert(parent, accessor, marker, initial, options)`,
`effect(fn, effectFn, options)`, `memo(fn)`, `ref(fn, element)`,
`applyRef(r, element)`, `template(html, flag)`, `delegateEvents(eventNames)`,
`addEvent(node, name, handler, delegate)`,
`spread(node, props, skipChildren, skip, name)`.

| Helper | web.js (prod) | web.dev.js | web.observe.js | server.* |
| --- | --- | --- | --- | --- |
| `memo`, `template`, `ref`, `applyRef`, `delegateEvents`, `addEvent` | identical | identical | identical | `memo`, `applyRef` identical |
| `insert` | identical | CHANGED | CHANGED | n/a (not a server function) |
| `effect` | identical | CHANGED | CHANGED | identical |
| `spread` | CHANGED (signature only) | CHANGED | CHANGED | n/a |
| `createComponent`, `mergeProps` (re-exports) | export line identical | identical | identical | exported |

Description of each change:

- `spread`, all web tiers: new trailing parameter `name`. In prod the body is
  otherwise identical (the diff is the single signature line). In dev it
  additionally sets `unscopedByDesign = node` around the props effect and
  wraps in `setSpreadName(name)`.
- `effect`, `insert` (dev and observe only): when a `spreadName` is set and no
  `options.name` is given, they set `options.name` to `<name>.spread` or
  `<name>.children`. Naming only; prod is untouched. This is the runtime half
  of rc.10 `sourceNames.bindings` (CHANGELOG rc.10, fd36d37).
- `insert` (dev only): adds `unscopedHoleSnapshot` / `checkUnscopedHole`
  diagnostics around the accessor read. Diagnostic only.
- `getNextElement` (dev only): increments `hydrationKeyMisses`. Diagnostic.
- The prod semantics of `insert`, `effect` (including its
  `createRenderEffect` shape), `memo`, `template`, `ref`/`applyRef`,
  `delegateEvents`, `addEvent` are byte-identical. `delegateEvents` is
  identical in every tier, including the `_$$<type>` event key (introduced in
  rc.9; see 6.5).
- Server `ssrElement` gains optional trailing parameters `attrs`, `claims`
  (`server.js`: `ssrElement(tag, props, children, needsId, skip, attrs,
  claims)`); the old 5-argument call shape is still accepted
  (`attrs !== undefined` guard). Also changed on the server: `escape`,
  `ssrAttribute`, `ssrClassName`, `ssrStyle`, `ssrStyleProperty` (slot-value
  handling `isSlotValue`/`slotValueInline`), and `ssrClaim`, whose output
  changed from ` _bnd="..."` to slot markers. `ssrClaim` is emitted only when
  `server_components` is on; the adapter never sets it
  (`compile_options`, `lib.rs:146-161`, leaves it at the default false), so
  this does not touch the checker's compile path [M for the adapter, E that
  it has no other consumer].
- `createComponent` and `mergeProps` are re-exported from `solid-js`
  (`web.js:2`, identical in both). Their bodies belong to the solid-js bytes
  review, not this section.

### 6.3 What compiler does an rc.13 consumer run? (Q3) [M]

`npm view` results, 2026-10-05:

| Package | dist-tags | Relevant versions (time) |
| --- | --- | --- |
| `solid-js` | latest 1.9.15, next 2.0.0-rc.13 | rc.9 2026-09-18, rc.10 09-27, rc.11 09-28, rc.13 09-30 (no rc.12 published) |
| `@solidjs/web` | latest 2.0.0-rc.0, next 2.0.0-rc.13 | peer `solid-js ^2.0.0-rc.13` |
| `@solidjs/compiler` | latest 2.0.0-rc.2, next 2.0.0-rc.13 | rc.9 09-18, rc.10 09-27, rc.11 09-28, rc.13 09-30 (no rc.12); gitHead `5efaf260` |
| `@solidjs/babel-plugin` | latest 2.0.0-rc.2, next 2.0.0-rc.13 | same cadence; gitHead `5efaf260` |
| `@solidjs/vite-plugin` | **latest 3.0.0-next.47, next 3.0.0-next.35** (inverted) | next.44 09-18, next.45/46 09-27, next.47 09-30 |
| `vite-plugin-solid` | latest 2.11.14, next 3.0.0-next.27 | frozen since 2026-08-12 |
| `babel-preset-solid` | latest 1.9.15, next 2.0.0-rc.2 | no 2.0 release since rc.2 |
| `@dom-expressions/babel-plugin-jsx` | latest 0.50.0-next.44, next 0.50.0-next.42 | frozen |
| `@dom-expressions/compiler` | next 0.50.0-next.44, latest next.25 | frozen |

Pairings with `solid-js@2.0.0-rc.13`:

- `@solidjs/vite-plugin@3.0.0-next.47` (09-30): dependencies
  `@solidjs/compiler ^2.0.0-rc.13`, `@solidjs/babel-plugin ^2.0.0-rc.13`;
  peers `solid-js ^2.0.0-rc.13`, `@solidjs/web ^2.0.0-rc.13`.
- `solid-js@2.0.0-rc.13` itself declares no compiler dependency
  (`dependencies`: csstype, seroval, seroval-plugins, `@solidjs/signals
  ^2.0.0-rc.13`).
- `@solidjs/compiler@2.0.0-rc.13` is a thin loader (`index.js`, 13 kB) over
  per-platform native packages (`optionalDependencies`, e.g.
  `@solidjs/compiler-darwin-arm64@2.0.0-rc.13`).

Did the compiler a project runs move between rc.9 and rc.13? **Yes, if the
project follows the vite plugin; the rc.9 audit's statement was narrower than
it read.**

- The audit said the rc.9 consumer compiles with
  `@dom-expressions/babel-plugin-jsx@0.50.0-next.44` through
  `babel-preset-solid@2.0.0-rc.2`, pulled by `@solidjs/vite-plugin@3.0.0-next.31`.
  That is accurate for **that vite-plugin version**: next.31/32 depend on
  `babel-preset-solid ^2.0.0-rc.0` + `@dom-expressions/compiler ^0.50.0-next.43`,
  next.33 on `@dom-expressions/compiler ^0.50.0-next.44` [M].
- From `@solidjs/vite-plugin@3.0.0-next.34` (2026-08-26, before rc.9) the
  plugin depends on `@solidjs/compiler` and `@solidjs/babel-plugin` instead,
  and keeps doing so through next.47 [M]. The rc.9-era plugin next.44
  (2026-09-18) pins `@solidjs/compiler ^2.0.0-rc.9`; next.45/46 `^rc.10`;
  next.47 `^rc.13` [M]. So a consumer on a current plugin ran the native
  `@solidjs/compiler` at rc.9 and runs rc.13 now. `@dom-expressions/*`
  and `babel-preset-solid` have not moved (still next.44 and rc.2) [M].
- Consequence [E]: whichever compiler a given rc.9 consumer ran (lockfile
  decides), an rc.13 consumer on the paired plugin runs
  `@solidjs/compiler@2.0.0-rc.13` (= upstream `5efaf260`, the same base as
  the local candidate `3ad4bbe`), whose lowering differs from the rc.3-era
  pinned fork (6.5). Consumers pinned to old dom-expressions keep the old
  compiler regardless of the runtime.

### 6.4 Helper references of the newer published compilers (Q4) [M]

Method: `npm pack` of the tarballs under `work/tar/`; for the native binary,
substring search of `compiler.darwin-arm64.node`; for the Babel plugins,
`registerImportMethod(path, "<name>")` literals.

| Compiler | Helpers it can reference beyond the pinned fork's set |
| --- | --- |
| `@solidjs/compiler@2.0.0-rc.13` (native, darwin-arm64) | `readShallow`, `ssrElementAttribute`. Binary contains both strings; contains no `rowProof` or `patchDriver`. Every other name in `helpers-B.txt` present. |
| `@solidjs/babel-plugin@2.0.0-rc.13` | `readShallow`, `ssrElementAttribute` (rc.9 plugin already referenced `readShallow`) |
| `@solidjs/babel-plugin@2.0.0-rc.9` | `readShallow` |
| `@dom-expressions/babel-plugin-jsx@0.50.0-next.44` | none beyond the pinned set (no `readShallow`) |

- All of these are exported by rc.13 `@solidjs/web` (client: `readShallow`,
  `ssrElementAttribute` in web.js/dev/observe; server: both in server.*).
  **No helper an rc.13 compiler references is missing from the rc.13 runtime.**
  (Universal-only names `setProp`, `createElement`, `createTextNode`,
  `insertNode` are not in `@solidjs/web` in either version; they resolve
  against the universal renderer module.)
- Helpers the rc.13 compiler references that the **pinned fork does not
  know**: `readShallow` (DOM `style`/`className` object reads, `dom/dynamics.rs:31,59`
  in the candidate; first appears with rc.8 ab4c40c) and `ssrElementAttribute`
  (rc.10 e10a4ba, rc.12 ad1ecc5). The pinned fork cannot emit them, so its
  facts for code that rc.13 would lower through them are modeled with the
  older lowering. This does not break anything (the runtime still exports the
  older entry points); it is a fidelity gap, not a resolution failure.
- The rc.13 candidate `3ad4bbe` knows both and drops `rowProof`
  (`helpers-A` vs `helpers-B` diff: +`readShallow`, +`ssrElementAttribute`,
  `-rowProof`).

### 6.5 Output skew between the pinned fork and rc.13 compilers [M, impact E]

The pinned fork is a `2.0.0-rc.3` compiler. Compiler changes released after
rc.3 that alter lowering (from `packages/compiler/CHANGELOG.md` in the
candidate; the pinned checkout's changelog ends at rc.3):

- rc.9 6d2bdeb: delegated handlers stamped as `_$$<type>` instead of
  `$$<type>`. [M] Pinned `src/dom/events.rs:52,59,71` emits `$$${event_name}`;
  the candidate emits `_$$${event_name}`. rc.9 and rc.13 runtimes
  (`delegateEvents` identical) read `_$$`. This skew already existed at rc.9
  and was not recorded in the rc.9 audit. The checker does not consume the
  key string (no `$$click` or `_$$` use in `rust/crates` or
  `rust/dialects`; only `$$component` etc. refresh names in
  `solid_v2_solid_js.rs:14-17`), so no finding depends on it [M for the
  search, E for the conclusion].
- rc.9 8d6de07 / 1643d2a: several spread sources lower to the array form
  `spread(el, [a, b])` / `ssrElement(..., [a, b])`, not `mergeProps`. [E]
  Pinned fork emits the pre-array form; the rc.13 runtime still accepts
  both (not re-verified for `mergeProps`-wrapped reactive spreads).
- rc.12 c71486d: an element's `ref` now runs after its attributes and spread,
  before children. [E] This is an ordering change for a lowering the
  checker's ref/effect timing model reads.
- rc.10 fd36d37 / 6717d35: `sourceNames` (default follows `dev`) adds
  trailing `{ name }` arguments to `effect`/`insert` and a name to `spread`;
  rc.10 1735074 / e10a4ba: SSR `hoistProps` and trailing-attribute thunks;
  rc.12 ad1ecc5: server-component slot holes. The adapter requests `dev`
  from its normalized options (`lib.rs:146-161`); a rebased fork must decide
  explicitly what `sourceNames`/`hoistProps` do to recorded facts. The
  candidate already binds `hoist_props`, `source_names_components` and
  `source_names_bindings` into the trace's effective configuration
  (SEMANTIC_FACTS.md, "2026-10-03 published RC.13 update").
- The rc.9 audit's own estimate stands: the unpinned rc.7-era checkouts
  (`16f0988`, `9d2ccb9`) differ from `9f9a84b` in 26 files. The pinned
  checkout `9f9a84b` is itself rc.3.

### 6.6 Conclusion (Q5)

1. **Helper interface: compatible [M].** Every helper the pinned fork
   (`9f9a84b`) can emit is still exported by `@solidjs/web@2.0.0-rc.13`
   wherever rc.9 exported it, in all six bundles. The only removal in the web
   packages is the server `CLAIM_PROP` export, which the fork never
   references. `rowProof` and `patchDriver` remain absent as in rc.9 and are
   dormant by default. Moving the audited triple to rc.13 therefore does not
   break the fork's helper imports, same as for rc.9.
2. **Runtime semantics the checker models: unchanged in prod [M].** `insert`,
   `memo`, `template`, `ref`, `applyRef`, `delegateEvents`, `addEvent`,
   `effect` are byte-identical in the prod web bundle. Differences are
   confined to dev/observe diagnostics and naming, plus the new optional
   `name` argument on `spread`.
3. **A rebase is not required for helper compatibility, but the pin is stale
   in lowering [M/E].** The fork is rc.3-based; an rc.13 consumer's real
   compiler (`@solidjs/compiler@2.0.0-rc.13` at `5efaf260`, selected by
   `@solidjs/vite-plugin@3.0.0-next.47`) emits `readShallow` and
   `ssrElementAttribute`, `_$$` event keys, array-form spreads, and
   post-attribute `ref` ordering, none of which the pinned fork produces.
   That gap predates rc.13 (it is present against rc.9 for `_$$` and the
   spread form), so rc.13 widens it rather than creating it. Whether any
   finding depends on those lowering differences is not measured here.
4. **A rebase candidate is ready locally.** `3ad4bbe` (rc.13 base
   `5efaf260`) is the intended adoption path; the report lists the open
   steps: publish the commit so Cargo can fetch it, move Cargo pins, adapter
   identity, source manifest, `compiler-identity.json`, notices, and
   conformance records atomically, then run the armed process tests,
   coverage/ownership, contract conformance/corpus and `make verify`
   (phase22 report, "Artifacts and remaining adoption work"). Those gates
   were not run by this audit.
5. **Registry hazards to record [M].** `@solidjs/vite-plugin`'s `next` tag
   (next.35) is older than its `latest` (next.47), so `@next` pairs with
   rc.3-era compilers; `babel-preset-solid@next` is still rc.2; no rc.12 of
   `solid-js`, `@solidjs/compiler` or `@solidjs/babel-plugin` was published.

Not done: no runtime execution of compiled output against the rc.13 bundles;
no diff of `solid-js` / `@solidjs/signals` bodies (other sections); no check
of the unreleased `next` head (`9e85a092`, per the phase22 report).
