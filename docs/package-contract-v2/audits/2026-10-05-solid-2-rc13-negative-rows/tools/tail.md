## Other rc.9-scoped answers in `solid_2.rs`

These answers live outside `NEGATIVE_ROWS`. Each is either gated in code on an
rc.9 archive or sourced in its doc comment from rc.9 bytes. The ADR 0168
argument-scoped rows (§§ 10, 26, 29, 45) and the two `browser` host-target rows
(§§ 44, 46) are rows, and are decided above. `other-answers.json` carries the
same list.

| # | answer (`solid_2.rs`) | what it asserts | rc.13 verdict |
| --- | --- | --- | --- |
| A1 | `computed_accessor_read_archive` (ADR 0162, ADR 0175; gated on `audited.version == "2.0.0-rc.9"` for `@solidjs/signals` and `solid-js`) | Invoking `createMemo`'s whole result, or `createSignal`'s slot 0, reads the node the creating call made. When that node is stale it re-runs the computation the call registered, and it may throw. It invokes no other callable. | **Holds on rc.13** [M diffs, E reading]. In signals, `createMemo`, `accessor` (`read.bind(null, node)`, `$REFRESH`) and `signal` are byte-identical in dev. `read` adds only the dev `checkPostAwaitRead` diagnostic and a guard term; `prepareComputed` relinks an auto-disposed computed under its live parent before `recompute(comp, true)`; `updateIfNecessary` widens a flag mask. In `solid-js`, every exit of rc.13's client `hydrateSignalLike` still returns `coreFn(…)`'s result (the new `transparent \|\| noHydrationId()` exit, `withHydrationGate` (identical), the rebuilt hybrid branch, `hydrateSignalFromAsyncIterable` (identical), and the final `coreFn`). On the server, `createMemo`'s `read` is unchanged (only the disposal flag gained `onDisposed` hooks), and `createSyncMemo`, `clientHoleRead` and `createSignal` are identical. What changed is the registered computation, which is the creating call's. **The gate must be extended to the rc.13 tuples for the answer to bind**: a code change, not made here. |
| A2 | `inert_accessor_read` (ADR 0146) | With primitive arguments, `createSignal`'s slot 0 is a plain signal's `read`, which runs no code. | **Holds** [M]: `createSignal`'s plain path, `signal` (dev) and `accessor` are identical. `read`'s rc.13 additions are dev diagnostics (`checkPostAwaitRead`), of the same class as rc.9's `warnStrictReadUntracked`, so no caller code runs. The `solid-js` server `createSignal` non-function path (`() => first`) is identical. |
| A3 | `inert_read_ignores_options` (ADR 0180) | `read` consults neither `options.equals` nor `options.unobserved`. | **Holds** [M]: dev `signal` is byte-identical, prod differs in mangled names only, and rc.13 `read` does not reference `_equals` or the `unobserved` slot. The one `unobserved` mention is a comment above the `dormantNodes` deferral, which applies only to auto-disposed computeds and already existed in rc.9. Not re-probed. |
| A4 | `eager_owned_computation_slot` (ADR 0183) | A one-argument `createMemo` runs its compute during the call under the new node. A two-argument `createEffect` runs its compute during the call. | **Holds** [M]: `setupComputedNode` keeps `!options?.lazy && recompute(self, true)`. `effect` and `createEffect` are identical (runtime.md § 3.3). runtime.md probes J/K/N show `ranDuringCall=true`, and `REACTIVE_WRITE_IN_OWNED_SCOPE` still fires in dev on rc.13. |
| A5 | `callback_handles_pending_accessor_read` | `isPending`'s callback is exempt from the strict pending-read safeguard. | **Holds** [M]: rc.13 `read` still guards with `strictRead && !pendingCheckActive && owner._statusFlags & STATUS_PENDING`, and the new `checkPostAwaitRead` gate also excludes `pendingCheckActive`. |
| A6 | `tracking_runtime` (ADR 0163) | `@solidjs/signals` exports `createRoot`, `createMemo`, `createSignal` and `getObserver`, and a read inside a created memo links a dependency (`link`). | **Holds** [M]: all four are exported by `dist/prod/index.js`, `dist/observe/index.js` and `dist/dev.js`. `link` is identical in dev (prod and observe are mangled). The probe's run-time calibration of the dependency fields succeeded on all three rc.13 builds (§ 0.6). |
| A7 | `AUDITED_ARCHIVES` rc.9 tuples | The archives a row may name. | rc.13 tuples, all [M] from § 0.1: `@solidjs/signals` / `2.0.0-rc.13` / `sha512-4+pRdrAH…MYeQ==` / `6783c3c6…95a6`; `solid-js` / `2.0.0-rc.13` / `sha512-62bYOI4J…BdoQ==` / `ce43a022…d284`; `@solidjs/web` / `2.0.0-rc.13` / `sha512-vI/7v/XM…gOQ==` / `8b45ed71…2bf5`. The full strings are in § 0.1 and `other-answers.json`. |
| A8 | Withheld rc.9 entries (the dialect test's withheld list: `Show`, `Loading`, flat `createSignal`/`createMemo`, `merge`, `affects`/`isPending`/`latest`/`refresh`, `hydrate`/`render` `reads`; and `createOptimisticStore` `reads`) | No row. | **Not re-read.** They stay rowless for rc.13. `reconcile` `reads` joins them (§ 24). |
| A9 | `DEFINITION_ALIASES` (`createOptimisticStore` → `createOptimisticStoreNext`, `createProjection` → `createProjectionNext`, version-keyed) | The local name a citation's slice begins with. | Same aliases on rc.13 [M]: `dist/prod/index.js:41`, `:43` and `dist/observe/index.js:43`, `:45`. The table needs rc.13 entries. |
| A10 | The host-target scopes of §§ 44, 46: condition `browser`, runtime `dist/solid{,.dev,.observe}.js`, delegates | Which `.` resolutions the scoped rows answer for. | **Unchanged** [M]: `solid-js`' `exports["."]` is identical to rc.9's. The delegates are rc.13 signals rows granted here (§§ 4, 11, 18). |
| A11 | `releases.rs` release answers (B1–B4, N3–N5) | Per-release vocabulary. | Not re-measured here. runtime.md § 3.5 measured all seven equal to `Solid2::RC9` on rc.13. |

## Citations, and how they were computed

- **Bytes and offsets.** Byte offsets are of the whole file, read in binary from
  `rust/target/audit-rc13/node_modules`. Each cited file's sha256 and length
  equal its rc.13 `files.json` entry. Each slice is the rc.13 definition of the
  same name, cut by the convention the rc.9 citation of the same row and file
  used. There are two:
  - From the start of the definition's line (including a leading ` */ `) to
    the start of the next line.
  - From the `function`/`const` token to the closing `}` or `;`.
- **Conventions were reproduced first** [M]. The convention was inferred from
  the rc.9 slice, and re-cutting rc.9 by it reproduced every rc.9 citation's
  `start_byte`/`end_byte` exactly, all 192 citations of the 48 rows.
- **Uniqueness** [M]. Each rc.13 definition is the only `function <name>(` or
  `const <name> = ` in its file.
- **Slice files.** The slices are written verbatim to
  `audited-slices/solid-v2/rc13/{solidjs-signals,solid-js,solidjs-web}/<archive_path>.<start>-<end>.slice`.
  That is `audited_slice_path`'s scheme with `phase0_archive_directory` =
  `rc13/<archive>`.
- **Withheld row.** § 24's table records the subject of the reading only; the
  row carries no citation.
- **Self-check** [M]. For every citation in `rows.json`, the slice file and the
  cited range of the rc.13 file were re-hashed. Both equal `slice_sha256`, and
  `file_sha256` equals `files.json` (`tools/selfcheck.mjs`, output in
  `selfcheck.txt`).

## What the lead must change for these rows to bind (not done here)

1. `AUDITED_ARCHIVES`: add the three rc.13 tuples (A7), and add
   `audited-archives.json` if it mirrors them.
2. Add `const RC13: &str = "2.0.0-rc.13";` and an audit constant naming this
   document once it is moved under `docs/package-contract-v2/audits/`. The
   `section` strings in `rows.json` are this document's headings verbatim.
3. Add the 47 granted rows. Each keeps its rc.9 row's `scope` and argument and
   host-target premises verbatim; `rows.json` `scope` is the rc.9 Rust
   expression. Copy the slice files into
   `rust/crates/solid-dialect/audited-slices/solid-v2/rc13/`.
4. `DEFINITION_ALIASES`: add the two rc.13 entries (A9). In the dialect test's
   implementation-audit list, add the 47 rows and record `reconcile` `reads` as
   withheld for rc.13.
5. `computed_accessor_read_archive`: admit the rc.13 `@solidjs/signals` and
   `solid-js` tuples (A1), with its own test.
6. Update the counts in the `NEGATIVE_ROWS` and `AUDITED_ARCHIVES` doc comments
   and in `the_negative_table_is_derived_from_the_audited_documents`, and
   record the `reconcile` `reads` withholding in `docs/precision-backlog.md`.
7. `releases.rs:39` currently says "No negative row is carried to its archives
   (ADR 0194)". That sentence moves with the rows.

## Residual approximations

- The call graphs are name-based over-approximations (§ 0.4). Dispatch through
  node fields (`_fn`, `_equals`, `_run`), caller values and hook slots is
  dispositioned by hand. The hook-slot follow-up was run on the dev build only;
  prod and observe rely on the name-based graph and slice diffs.
- The spurious `write` cut (owner item 4) was verified by showing that no
  `reads` root reaches `runProjectionComputedNext`, the arrow's only legitimate
  caller. A root that did reach it would need the edge back.
- Owner flag O1 is a judgement, not a measurement.
- `reconcile` `reads` is withheld on reachability. Whether the
  `readerOverride` arm actually runs under a caller's optimistic-store setter
  was read, not executed [E].
- The `solid-js` pairing covers `solid-js@2.0.0-rc.13`'s server builds. No other
  `solid-js` release is covered beside rc.13 signals.
- The flat `For`, `Repeat`, `clientOnly` rows rest on rc.9's flags F1–F3 as
  before.

## Tools (all under `rust/target/audit-rc13/rows/tools/`)

| file | does |
| --- | --- |
| `parse-rows.mjs` → `rc9-rows.json` | extracts the 48 rc.9 rows and their citations from `solid_2.rs` |
| `locate.mjs` → `located.json` | verifies each rc.9 citation, infers its convention, and locates and cuts the rc.13 definition |
| `closure.mjs`, `measure.mjs` → `measurements.json` | read-site call graph, host reach, rc.9/rc.13 closure comparison |
| `slots.mjs` | hook-slot follow-up (dev) |
| `readsites.mjs` | archive-wide read-site census |
| `hostcensus.mjs` → `host-*.txt` | archive-wide host-boundary census |
| `topwalk.mjs`, `defcmp.mjs`, `defdiff.mjs`, `show.mjs`, `slicediff.mjs`, `newcalls.mjs` | per-file walks and diffs |
| `texts.mjs`, `head.md`, `tail.md`, `build.mjs` | generate `rows.json`, the slices and this document |
| `selfcheck.mjs` | the citation self-check |
| `../probes/` | the rc.9 reads probes run on rc.9 and rc.13 |
