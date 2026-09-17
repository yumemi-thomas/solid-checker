# Plan: retire Solid 1.x support without losing shared-code precision

- **Status:** proposed, not started. Needs an ADR (see step 0).
- **Date:** 2026-09-16.
- **Goal:** the checker analyzes Solid 2 only. Every regression pin that today
  runs under a 1.x stub but exercises *shared* engine code is ported to 2.x
  before the 1.x dialect is deleted. A project on solid-js 1.x gets an explicit
  refusal, never a silent v2 analysis.

Read `AGENTS.md`, `CLAUDE.md`, `.claude/skills/add-fixture/SKILL.md`,
`.claude/skills/green-commits/SKILL.md` and `.claude/skills/verify-handoff/SKILL.md`
first. Run one Cargo process at a time. Do not run coverage with `--update`
until the non-updating run has shown the exact intentional change.

## Why port first, delete second

The 1.x footprint is small where it is genuinely 1.x (the `solid-v1` crate,
`solid_1x.rs`, ~160 gated lines in `upstream_compat`) and large where it is
*coverage*: 46 fixture dirs, 27 of the 37 stub-bearing `package-contracts`
fixtures, 271 of 306 ownership cases, 9 backend test fixtures. Those pin
dialect-neutral mechanics (callback attribution, export identity, unresolved
dispatch, conditional returns) and happen to run under a 1.x stub. Deleting
them leaves shared paths unpinned. Porting them is also the measurement: a
snapshot that moves in *substance* rather than rule prefix reveals a shared
path that behaves differently under v2, which is exactly what must be found
while the 1.x control still exists.

## Inventory (measured 2026-09-16)

| Area | 1.x-only content |
| --- | --- |
| `rust/dialects/solid-v1/` | 1,090 lines, 18 `v1/` rules; only `v1/jsx-no-undef` (SC8005) and `v1/prefer-classlist` (SC8013) have no v2 counterpart |
| `rust/crates/solid-dialect/src/solid_1x.rs` | 2,666 lines vocabulary table |
| `rust/crates/solid-reactive-ir/src/upstream_compat/` | `solid1x_attributes.rs`, `solid1x_undef.rs` gated by `carries_eslint_era_rules()` (~158 lines); `solid1x_syntax/structure/options.rs` are shared but 1.x-named |
| `fixtures/` | 46 dirs with a `1.9.14` stub: `reactive-ir` 18, `package-contracts` 27, `engine` 1. Divergence pairs: `dialect-solid-1x`/`dialect-solid-2`, `jsx-census-gap-*`, `jsx-void-child-divergence-*`, `eslint-plugin-corpus-v1`/`eslint-plugin-corpus`, `engine/eslint-reactivity-v1`/`-v2`, `package-contracts/props-split-vocabulary-v1`/`props-split-vocabulary` |
| `fixtures/ownership-cases/cases.json` | 271 `solid-v1` cases, 35 `solid-v2` |
| `fixtures/findings-snapshots/` | 11 snapshots for 1.x-only `reactive-ir` fixtures |
| `rust/crates/solid-facts-backend/tests/` | `dialects_process.rs` (26 v1 hits), 9 fixtures: `merge-props-function-v1`, `preferences-v1-enabled`, `preferences-v1-disabled`, `solid-1x-adapter-with-declarations`, `solid-1x-cross-file-adapter`, `solid-1x-evidence-contracts`, `solid-1x-options-comparator`, `solid-1x-resource-overloads`, `solid-1x-upstream-regressions` |
| `packages/cli/` | `lib/rules-solid-v1.json`, `test/adapter.test.mjs` (16 v1 hits), `published-contract-graph.test.mjs` (2) |
| `pkg/contracts/bundled/solid-v1/` | 19 files, inert historical documents |
| `pkg/contracts/accepted/` | 54 bundles, 22 packages, all 1.x releases. **21 of the 54 belong to two packages with no `solid2` benchmark row at all** — `@solidjs/start` 2.0.3 (11) and `@kobalte/solidbase` 0.6.13 (10) |
| `benchmarks/ecosystem/` | 418 rows: 250 `solid2`, 168 `solid1`. 214 packages: 93 on both targets, 46 solid2-only, **75 solid1-only** — all of `@corvu`, every `@tanstack/solid-*`, `solid-devtools`, `@solidjs/start`, `@solidjs/testing-library`, `@solidjs/image`. Also `benchmarks/package-contract-v2/phase14/solid-v1-authority/` |
| `scripts/` | `audit-solid-1x.mjs`, `SOLID_TARGETS` in `ecosystem-benchmark/discover.mjs`, `lib/{dialect-authority,report,select,manifest,families}.mjs`, `package-contract-v2-phase0.mjs` (solid1 compiler pin), `phase20/21-ledger.mjs` (hard-coded `|solid1|only` probe ids), `dialect-audit-yield.mjs` |
| `docs/` | `rules/v1/` (18 pages), `solid-1x-api-surface.md`, `v1/` rule names in `precision-backlog.md` |

## Ordering constraints (from review, 2026-09-16)

- **Step 4 lands before or in the same slice as step 3, never after.**
  `detect` in `rust/crates/solid-facts-backend/src/dialect.rs` maps a
  resolved 1.x version through `by_version` and falls back to the v2 default
  when nothing matches. Removing the v1 dialect first would turn every 1.x
  project into a silent v2 analysis for the whole window between the two
  steps, which is the outcome this plan forbids.
- **Step 2 does not start until step 1 is complete.** Replacing the accepted
  bundle tier changes certification results for every consumer, and the proof
  policy pins that tier rather than the build. The step 1 ports are the only
  evidence that shared paths behave identically under v2; the before/after
  census numbers are the only evidence step 2 did not regress silently.
- ~~The accepted-contract tier needs a decision before step 2 starts.~~
  Resolved by ADR 0110 § 4: the tier is keyed to the package artifact and
  survives intact, so step 2.5 is withdrawn and step 2 is unblocked.
- **Dry-run step 3 with the existing Cargo features first.** The backend
  already has `dialect-v1` and `dialect-v2` features (default both) and
  `scripts/verify.sh` runs four single-dialect arms. Flip the default to
  `["dialect-v2"]`, run `make verify`, and fix what breaks before deleting any
  code.

## Resolved: 1.x-era evidence and contracts (ADR 0110, 2026-09-16)

Both halves of what this section used to leave open are decided in
`docs/adr/0110-the-checker-analyzes-solid-2-only.md` §§ 4-5, on checked facts:

- **The accepted-contract tier survives intact at 54 bundles.** No bundle index
  or receipt carries a dialect, a Solid version or a language version; nothing
  regenerates the tier during verification (it is `include_bytes!`d and written
  only by `make accepted-bundles`); and a contract describes runtime behaviour
  the checker's scope does not change. **Step 2.5 is withdrawn.** The one real
  limit is recorded: a bundle whose derivation needed 1.x vocabulary can no
  longer be *regenerated*, so the tier is frozen — valid and receipt-proven, not
  reproducible from source.
- **The demand denominator stays frozen** at 2,585 call sites / 1,958 in corpus.
  The 1.x rows measure what consumers call, which this retirement does not make
  false. `SOLID_TARGETS = ["solid2"]` would raise every coverage percentage
  without proving a new statement, so the reporting rule is: a percentage
  measured after the retirement is not comparable to one before unless the
  denominator is held. Baseline to compare against: `ownerRequirement: 31`, 599
  operations stated of 1,940 measured sites.

Step 2 therefore has no accepted-tier work, and steps 2 and 3 are unblocked.

## Step 0 — ADR (done, 2026-09-16)

`docs/adr/0110-the-checker-analyzes-solid-2-only.md` is written and accepted. No existing ADR
argues for keeping both dialects, so this is a new decision, not a reversal.
It must state: the refusal behaviour for 1.x projects (step 4), the two rules
dropped without replacement, the resolution of the open decision above with
its measured numbers (**not** the claim that the 1.x rows have `solid2`
successors — 75 of them do not), and that the port is the measurement
instrument (step 1). Slice: docs only.

## Step 1 — port shared-code pins to 2.x while the 1.x control still exists

For each of the 46 stub-bearing fixtures and the 9 backend test fixtures:

1. Decide its class from its README and expected findings:
   - **shared mechanic** (most `package-contracts/*`, `no-owner-v1`,
     `v1-reactivity`, `v1-write-scope`, `array-shape-v1`, `solid-1x-sources`):
     port.
   - **divergence pair** (the six pairs above): the 2.x half stays, the 1.x
     half is deleted in step 3; record the divergence in the ADR.
   - **1.x semantics only** (`solid-1x-leftovers`, `eslint-plugin-corpus-v1`,
     `solid-1x-resource-overloads`, `solid-1x-upstream-regressions`,
     `preferences-v1-*`): delete in step 3.
2. To port: change `node_modules/solid-js/package.json` to `2.0.0-rc.3`,
   add the `@solidjs/web` stub if the fixture imports DOM APIs, translate 1.x
   API names in the fixture source to the 2.0 vocabulary (`createResource` →
   `createAsync`, `mergeProps` → `merge`, `splitProps` → `omit`/`pick`,
   `onMount` → `onSettled`, `Context.Provider` → the 2.0 form, etc., per
   `fixtures/reactive-ir/dialect-solid-2/README.md`). Keep the fixture's
   claim identical.
3. Run coverage **without** `--update` against the fresh debug binary:

   ~~~sh
   make build-checker-debug
   SOLID_CHECKER_BIN="$PWD/rust/target/debug/solid-checker-rust" \
   SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" bun scripts/coverage.mjs
   ~~~

   A moved snapshot has three possible causes, and the classification is the
   work of this step — not a formality:
   - **prefix only** (rule `v1/` gone, same codes, spans and finding kind):
     the port is clean, `--update` that fixture's snapshot.
   - **deliberate v2 divergence**: the fixture crossed vocabulary that the
     two dialects genuinely disagree about. `solid_1x` overrides 59 of the
     `Dialect` trait's 105 methods, so this is common, not exceptional —
     expect it wherever the port renamed an API rather than only a version
     string. Write the divergence into the fixture README as its pin, the
     way `fixtures/reactive-ir/dialect-solid-2/README.md` already does,
     then update the snapshot. A divergence with no README sentence is not
     classified, it is assumed.
   - **a finding about shared code**: anything else. Stop, diagnose, record
     it in `docs/precision-backlog.md`, and fix or pin it before proceeding.

   Reaching for `--update` before naming which of the three applies is how
   this step silently becomes a rubber stamp.
4. Ownership cases: the 271 `solid-v1` cases in
   `fixtures/ownership-cases/cases.json` are ported the same way. Ids are not
   dialect-scoped and a colliding id is dropped silently while the gate stays
   green, so rename ids on port and confirm the case count after
   `bun scripts/ownership-gate.mjs` (see `.claude/skills/upstream-parity/SKILL.md`).
   Cases whose upstream heuristic is `carries_eslint_era_rules()`-gated are
   1.x-only and are deleted in step 3 with the two rules.
5. Commit per fixture family as individually green slices
   (`green-commits`). Snapshot updates travel with the fixture edit.

Exit criterion: no fixture, ownership case, or backend test fixture that
pins shared behaviour still carries a 1.x stub.

### Progress (2026-09-16)

**Gate correction:** `fixtures/package-contracts/*` are **not** in coverage.
`scripts/coverage.mjs` discovers directories holding a `tsconfig.json` under
`fixtures/reactive-ir/` and `fixtures/engine/` only; the 27 package-contract
fixtures are registered by name in `scripts/contract-corpus.mjs` and compare
`expected.json` plus `expected-proposal.json`. Their check is
`bun scripts/contract-corpus.mjs` with
`SOLID_CHECKER_NATIVE_BIN="$PWD/rust/target/debug/solid-checker-rust"`. The
step-1 loop above runs coverage for the `reactive-ir`/`engine` fixtures and the
corpus for the package-contract ones.

**Slice 1 — the ten package-contract fixtures with no Solid API in their own
source. Complete (uncommitted).** These are the cheapest possible probe: the
only inputs that change are `node_modules/solid-js/package.json` and the
`dependencies` pin, so any movement is unambiguously shared code. Seven ported
clean on the first run — contract and proposal plan identical once the
manifest-derived hashes are stripped, corpus totals byte-identical to baseline.

The other three found the defect this step exists to find:
`callback-execution-boundary`, `destructured-parameter-callback` and
`unresolved-dispatch-attribution` each gained a `callbacks` closure under v2,
because `direct_own_call` was recorded only on the last-resort derivation arm
(`interproc.rs:1288`) and a capitalized export takes `UntrackedRendering` ->
`inline` two arms earlier. **Fixed in the same session** by reading
`direct_own_call` off the call site, which is what the field's own
documentation always claimed it meant; four contract fixtures moved, every
negative held, and the three then ported hash-only. Full account in
`docs/precision-backlog.md`, entry of the same date.

The lesson for the remaining slices: the defect was visible in a *checked-in*
expectation — two byte-identical callback sites in one file disagreeing on
closure because one export was capitalized — and no gate was ever going to
report it, because both dialects were internally consistent. Porting is what
made the two answers meet. Do not treat a v1/v2 disagreement as v1's problem
by default.

**Slice 2 — the 17 package-contract fixtures that do import Solid primitives.
Eleven ported, four reclassified, two confirmed not-ports.**

Ported and verified claim-for-claim identical (only the manifest-derived hashes
moved): `callback-reactive-arguments`, `composed-operation-provenance`,
`composed-operation-shadowed-target`, `conditional-returns-divergence`,
`conditional-returns-divergence-both`, `unresolved-dispatch-domains-control`,
`uncaptured-source-return`, `solid-reexport` (whose refusal text and census
sidecar are byte-identical under v2 — it needed no snapshot change at all),
`destructured-return-slot`, `multi-role-callback-parameter`,
`shorthand-block-scope`.

Three things that slice taught, none of them mechanical:

- **`createEffect(fn)` is a type error in 2.0.** The real signature is
  `createEffect(compute, effectFn, options?)`; the single-argument form is a
  deprecated overload returning `never`. Every 1.x `createEffect(() => f())`
  becomes `createEffect(() => f(), () => {})` — the tracked site stays in the
  compute arm, the effect arm references nothing.
- **An unconstrained `<T>` cannot call 2.0's plain `createSignal`.** Its
  overload takes `value: Exclude<T, Function>`, so a generic helper forwarding
  a caller's value must carry the exclusion in its own signature.
  `destructured-return-slot` needed `initial: Exclude<T, Function>` on all
  nineteen of its functions. This is a property of real 2.0 code, and writing
  the fixture without it would have pinned a shape no project can produce.
- **Type-check ports against the audited install, not the stub.** The real
  `solid-js@2.0.0-rc.3` is already on disk at
  `rust/target/tsc-oracle/v2/node_modules` (the `tsc-oracle-provision` target
  puts it there), and `packages/cli/node_modules/.bin/tsc` will check a source
  against it in seconds. Both errors above were found that way and by nothing
  else — the hand-written stub was happy.

**Four are not ports, and the plan's step-1 classification should say so.**
Each pins behaviour that 2.0 genuinely changes or removes, so a v2 counterpart
has to be *authored*:

- `callback-slot-props-forwarding` — its negatives state that 1.x's
  `createSignal(fn)` stores the function without invoking it and that 1.x's
  `createStore` has no compute form at any arity. **Both are false in 2.0**:
  `createSignal(fn)` is the writable-memo form and `createStore(fn, store)`
  exists. This is a divergence pair to create, like `props-split-vocabulary`.
- `callback-deferred-untracked-chain` — built on `onMount`, which 2.0 removes,
  and its claims cite `solid-js@1.9.14`'s `dist/solid.js` line numbers.
- `callback-untracked-wrapper` — closest to portable (`untrack`, `createRoot`,
  `runWithOwner` all survive, and its README already reasons about 2.0's
  `flush`), but its claims are measured against 1.x runtime bytes.
- `escaping-private-helper` — 2.0 moves `namespace JSX` out of `solid-js` into
  `@solidjs/web`, and `For` gains a second `keyed: false` overload whose
  children callback inverts to `(item: Accessor<T>, index: number)`. Porting it
  means an `@solidjs/web` stub and a `jsxImportSource` arrangement across
  thirty files whose JSX arms are the fixture's subject.

**Confirmed not-ports, as classified:** `dialect-detection` (1.x vocabulary
reaching contract generation is its whole claim) and `props-split-vocabulary-v1`
(the 1.x control of an existing divergence pair).

**Slice 3 — the 18 `reactive-ir` + 1 `engine` fixtures (coverage-gated).
Two ported, sixteen classified as not-ports, one port reverted.**

Ported: `package-structured-unresolved` (four rule names lose `v1/`; every code,
kind, severity, path, span and fix count identical — the textbook prefix-only
port) and `eslint-compat`, the first **deliberate v2 divergence** the plan's
third bucket was added for. Seven findings before and after; five identical
modulo the prefix; two `jsx-no-duplicate-props` lost and two
`reactive-handler-frozen` gained, both traced to named dialect behaviour
(`carries_eslint_era_rules()`'s DOM slot folding and
`static_event_values_are_attributes()`). Written into a new
`fixtures/reactive-ir/eslint-compat/README.md` as its pin; the fixture is now
the only regression pin either mechanic has under 2.0.

**Reverted: `array-shape-v1`.** It ported green and that meant nothing — its
snapshot is `{"status": "certified", "findings": []}`, and an empty expectation
is satisfied by any dialect, including one that never reaches the rule the
fixture is about. Its README had warned that the stub pins 1.x deliberately.
Full account in `docs/precision-backlog.md`, entry of the same date. Two rules
for the rest of the retirement come out of it:

- **A fixture whose snapshot has no findings is not portable by measurement.**
  Green before and green after proves nothing. Give it a non-empty assertion
  first, or classify it by reading.
- **Read the README for a deliberate dialect pin before touching a stub.**

**Two review habits this slice cost.** Coverage prints only the *first* moved
line per project (`eslint-compat` reported one and had four), so `git diff` on
`fixtures/findings-snapshots/` is the real review surface. And **API
availability does not make a fixture portable** — all three fixtures this slice
started with as "portable, only needs `onMount` → `onSettled`" turned out to be
1.x-only once read:

- `import-location` — its own README: "on Solid 1.x — the dialect with four
  subpaths, and so the **only one** where 'which module exports this?' has an
  interesting answer." 2.0 ships `.` and `./refresh`.
- `summary-callback-extent` — pins that a **1.x** effect's callback is `Tracked`
  and so leaked its read into the enclosing helper's summary, where 2.0's apply
  slot is `Deferred`. 2.0's `createEffect(compute, effect)` cannot produce the
  shape.
- `v1-reactivity` — a 1.x behaviour catalogue: the seed-argument form
  (`createEffect(fn, value)`, which 2.0 replaces with the effect arm) and 1.x
  compiler treatment of hyphenated JSX attribute names.

**Not ports (16).** Slice 1's premise — that these fixtures pin dialect-neutral
mechanics and merely happen to run under a 1.x stub — holds for the
package-contract group and does **not** hold here:

- *Existing divergence pair:* `v1-write-scope`. The v2 half `write-scope`
  already exists with 16 findings across `reactive-write-in-owned-scope` and
  `action-called-in-owned-scope`, against this half's 4.
- *Divergence-pair halves already in the inventory:* `dialect-solid-1x`,
  `jsx-census-gap-solid-1x`, `jsx-void-child-divergence-solid-1x`,
  `eslint-plugin-corpus-v1`, `engine/eslint-reactivity-v1`.
- *Blocked on the accepted-tier open decision, not on porting:*
  `bundled-contract-solid1-consumer`, `bundled-rootless-consumer`,
  `bundled-scheduled-consumer` — artifact-identity controls that each control
  against a specific 1.x bundle (`@solid-primitives/rootless@1.5.4`,
  `scheduled@1.5.3`, `debounce`, `solid-js@1.9.14`). **The open decision blocks
  more than step 2.**
- *1.x vocabulary or 1.x semantics:* `solid-1x-sources` (`createComputed`,
  `createDeferred`, `createSelector`, `createMutable`, `createResource`,
  `Index` — 2.0 removes all six), `store-subpath` (its claim *is* the
  `solid-js/store` subpath; 2.0 exports only `.`, `./refresh`, `./types/*`,
  `./package.json`), `upstream-divergences` (`createResource`), `no-owner-v1`
  (`Suspense`, `observable`), plus `array-shape-v1`, `import-location`,
  `summary-callback-extent` and `v1-reactivity` above.

Counterpart check worth repeating: of the ten candidates, only `v1-write-scope`
already had a v2 twin. Where a port *is* possible it preserves coverage that
deletion would lose — but here that was true for only two of nineteen.

**Slice 4 — the 271 ownership cases and the 9 backend test fixtures. Neither
is portable; step 1.4 is wrong and step 3 is incomplete.**

*The ownership cases.* **254 of the 271 are `upstream/*`** — eslint-plugin-solid
0.14.5's own `__valid__NN`/`__invalid__NN` cases, transcribed. That plugin
targets Solid 1.x, so re-pointing them at the 2.0 catalog asserts that
upstream's 1.x-defined expectations hold for a version upstream does not
support: not parity with upstream, a new claim wearing upstream's name. **179 of
the 271 are negatives** (169 upstream), so the `array-shape-v1` lesson applies
at scale — porting them and watching the gate stay green proves nothing. The
repository already says as much: the 35 `solid-v2` cases contain **no**
`upstream/*` id.

Step 1.4's renamed-ids procedure is therefore moot, and the real consequence
needs stating in the ADR: **retiring 1.x deletes the eslint-plugin-solid parity
corpus.** AGENTS.md's instruction that "retained behavior and intentional
divergences must be pinned in `fixtures/ownership-cases/cases.json`", and its
`upstream_compat` "Known traps" entry pinning commit 6d3bc311, both lose their
subject — **add AGENTS.md to step 3's edit list.**

Of the 17 product-owned v1 cases, four already have a v2 case in the same rule
family. Two deserve authored v2 counterparts rather than deletion because they
encode the absolute rule rather than a dialect's behaviour:
`react-prop/typescript-owned/001` and `innerhtml/typescript-owned/001`, both
pinning that TypeScript owns the diagnostic and the checker stays silent. Both
are negatives, so both must be authored, not ported.

*The backend test fixtures.* The nine carry **no `node_modules/solid-js` stub**.
`dialects_process.rs` passes `--dialect solid-v1` explicitly at 26 sites, so no
stub swap reaches them and removing the dialect makes every site fail loudly —
the good case. But three assertions are **differential by construction**:
`dialect_pair_findings` (3 uses) compares byte-identical sources across the
pair, and `component_ref_callbacks_are_setup_time_outputs_in_both_dialects` and
`returned_event_handler_factories_preserve_deferred_execution` each loop
`for dialect in ["solid-v1", "solid-v2"]`. Deleting v1 does not leave a narrower
test; it removes the comparison that *is* the test. Each needs an explicit
decision — rewrite as a single-dialect assertion and accept the weaker claim,
or delete and record the loss. `preferences-v1-*` already have `preferences-v2`
/ `preferences-v2-disabled` beside them and are a clean step-3 deletion.

Full accounts in `docs/precision-backlog.md`, entries of the same date.

**Step 1 is now complete as a porting exercise.** What could be ported has
been: 21 of 27 package-contract fixtures and 2 of 19 `reactive-ir`/`engine`
fixtures. Everything else is authoring work, a step-3 deletion, or blocked on
the accepted-tier decision.

**Slice 5 — step 4's plumbing, and a hole in the handoff authority.**

`detect` could not express a refusal: it returns `&'static Dialect`, and
`resolved_solid_version` collapsed three outcomes into one `Option<Version>`
while the manifest path a refusal must name was a local in the loop.
`Detection` (`Installed` / `Unsupported` / `Defaulted`, each resolution case
carrying the path it read) now sits behind `detect_detailed`. **No behaviour
changed** — `detect` collapses it exactly as before, every gate unmoved — but
the hole is now asserted instead of described:
`a_one_x_install_is_supported_exactly_while_its_dialect_is_compiled_in` proves
that without the 1.x dialect a 1.x install is `Unsupported` *while `detect`
still answers `solid-v2`*.

Still to do for step 4: the emission at the three call sites (`daemon.rs`,
`main.rs`, `solid-checker-session-bench.rs`), the `SC9013` registration, and
the fixtures and CLI test. Unchanged from the ordering constraint: it lands
with step 3, never after.

**And `make verify` does not test what this retirement produces.** All four
single-dialect arms in `scripts/verify.sh` are `cargo check`, so the
`--features dialect-v2` configuration — the one step 3 makes permanent — has
never had its tests run. Five were broken there; all five are now fixed (three
asserting dialect ids for claims that were about resolution, one demanding
every `RULE_ALIASES` target load, two differential tests now gated on both
features). Both arms pass in full: 541 v1-only, 545 v2-only.

Two consequences for the plan. **Add to step 3:** those two newly gated tests
join the three differential assertions in `dialects_process.rs` — **five** tests
lose their subject when the 1.x dialect goes, not three. **Done:** `test-backend-v1` and
`test-backend-v2` now run in `scripts/verify.sh` after the oracle archive
exports, `--lib`, and `make verify` passes end to end — but they cost 130.5 s
and 135.3 s of a **516 s** total (it was roughly 250 s), which is 51.5% of the
run. `test-backend-v1` verifies a configuration step 3 deletes; dropping it is
a one-step, −130 s trade if the number is unacceptable. Both kept for now
because the transition window is exactly when a v1-only regression could land.
See `docs/precision-backlog.md`.
Full accounts in `docs/precision-backlog.md`, entries of the same date.

**Slice 6 — eleven ownership cases were asserting nothing.**

The ownership gate required every negative to "name at least one absent rule or
family", and an `absent` clause is a filter over actual findings — so a clause
naming a rule **no catalog declares** cannot fail, and is satisfied by any
implementation. An author with no real rule to name reached for the upstream
name the checker deliberately does not implement, and the requirement defeated
itself. 23 clauses across the corpus are ineffective that way, and for **11
cases the clause was the only assertion** — including both `typescript-owned`
cases, which carry the absolute rule and were asserting it vacuously.

Fixed on both sides: the gate now rejects an `absent` clause whose rule the
case's dialect catalog does not declare (verified with a negative control), and
`expect.silent: true` is the falsifiable shape for "this source emits nothing".
All 23 repaired — clause dropped, intent moved to a `note`, and the 11 given
`silent: true`, which all pass. Gate green at 306 cases, ledger 465 rows.

**Slice 7 — the absolute rule has a Solid 2 pin.** The two `typescript-owned`
cases existed only for 1.x. Authoring the v2 counterparts needed a gate
extension first: the per-finding `typescript-owned` shape asserts both halves
correctly but needs a rule and code to name the finding that must not be
emitted, and these are exactly the cases where the checker carries *no rule at
all* — naming one under a fabricated `SC0000` would put a fiction in an
exactness corpus. `expect.typescript` is now a case-level `{code, span}` array,
and paired with `silent: true` it asserts that TypeScript really does report
`TS2322` at those spans (confirmed against the audited oracle, identical in both
dialects) and that the checker emits nothing at all.

Four cases now: both v1 halves strengthened from `silent`-only, and
`…/typescript-owned/002` authored for `solid-v2`. Corpus at 308 (271 v1, 37 v2).
Three negative controls — wrong code, wrong span, reintroduced undeclared
`absent` clause — each fail and each clear. **Step 3 can now delete the 1.x
halves without the invariant losing its pin**, which is why this was authored
before the deletion rather than after.

## Step 2 — retarget the contract data to Solid 2

**Mostly superseded by ADR 0110, and the parts that survive are not what this
step proposed. Resolved 2026-09-16.**

- **Sub-steps 3 and 5 are withdrawn by ADR 0110 § 4.** No bundle binds a
  dialect, nothing regenerates the accepted tier during verification, and a
  contract describes runtime behaviour the checker's scope does not change. The
  tier stays at 54 bundles.
- **Sub-step 1 is refused by ADR 0110 § 5**, which the plan predates.
  `SOLID_TARGETS = ["solid2"]` would drop 168 of 307 benchmark rows — 75
  packages are solid1-only, including SolidStart itself, every
  `@tanstack/solid-*` and all of `@corvu`. Those rows are evidence about *what
  consumers call*, which is a fact about the ecosystem and not about which
  language this checker analyzes. Narrowing the corpus would raise every
  coverage percentage without proving one new statement, so the benchmark
  manifest keeps its 1.x rows.
- **What did have to change:** `make contract-coverage-census` ran
  `--solid 1`, which after the retirement refuses all 18 packages with SC9013
  and reports nothing. It is `--solid 2` now — and because that measures a
  *narrower* corpus, the census pin records which corpus produced it and
  **refuses to compare across the boundary**. The first `solid2` run therefore
  fails until it is re-pinned deliberately, rather than silently reporting
  improved coverage. The 1.x-era numbers (`ownerRequirement: 31`, 599 of 1,940
  sites) stay as the frozen baseline ADR 0110 § 5 names.
- **Sub-step 5's deletion is refused, on ADR 0110 § 4's own argument.**
  `pkg/contracts/bundled/solid-v1/` (19 documents) and
  `benchmarks/package-contract-v2/phase14/solid-v1-authority/` (19 documents
  plus `authority-index.json`) are audit records of *published package
  artifacts* — exact `solid-js@1.9.14`, `@solid-primitives/scheduled@1.5.3`,
  `@solid-primitives/debounce@1.3.0`, `@solid-primitives/rootless@1.5.4` — and
  § 4 already decided that class survives: a contract describes bytes that
  invoke what they invoke, which this checker's scope does not change.
  Three checked facts, not judgement:

  - **Neither directory is an analyzer input.** Both bundle indexes are
    `contracts: []`, `EMBEDDED_BUNDLES` and `EMBEDDED_SOLID1_BUNDLES` are
    empty, and `bundled_first_party_contract_index` — the seam whose
    `"solid-v1"` arm reads the authority — has no caller in the workspace at
    all. ADR 0027 retired it before this plan existed.
  - **They are live gate inputs.** `solid1_bundles_with_measurements` decodes,
    normalizes and closure-digest-checks all 19 authority documents against
    the index (returning no bundles: policy 2 retired them, it did not retire
    the census), `scripts/package-contract-v2-phase0.mjs` measures four of the
    bundled documents, and `runtime-lock.json` records the audited closure.
    That is conformance evidence about 1.x artifacts, which nothing else
    reproduces.
  - **Two of them are test fixtures for dialect-neutral code.**
    `debounce-root-default.json` and `solid-root-node.json` reach
    `solid-checker-wasm` and `policy2_receipt/tests.rs` through
    `include_bytes!` as real stable-v1 documents. Deleting them would cost
    those tests their input and buy nothing, since neither test asserts
    anything about Solid 1.x.

  What *is* dead here and was left alone deliberately: the `"solid-v1"` arm of
  that uncalled seam. Removing it is ADR 0027 cleanup, not retirement work —
  it was already unreachable before this plan, and AGENTS.md forbids widening
  a semantic change into one.

- **Sub-step 4's "blocker" is not one, and calling it a resolution defect was
  wrong (corrected 2026-09-16).** The census does refuse callees resolved into
  `solid-js/types/client/hydration.d.ts` — `createSignal` on
  `@solid-primitives/scheduled@2.0.0-next.2` is the observed case — but
  resolution is working, and its answer is correct. Traced end to end:

  - `solid-js@2.0.0-rc.3`'s `types/index.d.ts:8` **re-declares**
    `createSignal`, `createMemo`, `createEffect` and eight siblings from
    `./client/hydration.js`. Line 1 re-exports a different set from
    `@solidjs/signals`, and `createSignal` is **not** among them — checked
    against the audited install, not inferred.
  - So the callee's declaration really does land in `hydration.d.ts`, and the
    archive it binds is `solid-js`, not `@solidjs/signals`.
  - `NEGATIVE_ROWS` carries `createSignal`/`creates` for
    **`package: "@solidjs/signals"`** only. `solid-js`' re-declaration is
    withheld on purpose (`solid_2.rs` § 7.4): the browser bodies create
    nothing, but the `node`/`worker`/`deno` body reaches
    `ctx.serialize(id, deferred.promise, deferStream)`, which
    `semantic-model.md` § creates **[Decision 2026-09-04]** settles *is* a
    create.
  - `census_dialect_axiom` is archive-bound — it takes
    `audited_archive_for_snapshot(snapshot)` for the snapshot the declaration
    resolves into and asks `primitive_performs_no_operation`. With the row
    withheld that is `false`, no terminator is issued, and the claim stays
    open. That is the refusal.

  A `(package, export, domain)` row carries no condition, so closing this one
  would state something false for every SSR consumer. **The fix is a
  condition-aware negative table — an open item in ADR 0007 — not a resolver
  change**, and it is a design change well outside this plan. The census
  numbers are readable now, with this hole named: no `creates` statement is
  available for the eleven primitives `solid-js` re-declares from
  `./client/hydration.js`, however they are imported.

  **One thing to look at while there, deliberately not changed here:**
  `some_audit_denies_primitive` matches `row.export` and `row.domain` and
  never `row.package`, so the *proposal* generator reads `@solidjs/signals`'
  row as covering `solid-js`' withheld re-declaration. Its doc comment argues
  this is safe because a proposal is only ever proven later against
  authenticated bytes — and the trace above is that argument working, since
  the archive-bound census is what refuses. It is the same package-blind
  name match whose cross-*dialect* form was a real bug before the retirement,
  so it wants a deliberate look rather than an inherited one.

### Original text, kept for the record


1. `scripts/ecosystem-benchmark/discover.mjs`: `SOLID_TARGETS = ["solid2"]`;
   remove the `solid1` rank branch and the `dialect-authority`/`select`/
   `families`/`manifest`/`report` handling for it, including tests.
2. `scripts/package-contract-v2-phase0.mjs` (solid1 compiler pin) and the
   phase 20/21 ledgers (hard-coded `|solid1|only` probe ids): move to a
   `solid2` pin or retire the ledger with a note; do not leave them failing.
3. Re-run the census on the Solid 2 rows and regenerate the compiled-in tier:

   ~~~sh
   make contract-coverage-census
   make accepted-bundles
   make ecosystem-regression
   ~~~

   Read the run through `--run`, never from `rust/target/coverage-census/`
   leftovers. `ecosystem-regression` is the only gate that sees a receipt
   lost elsewhere in the corpus.
4. Known blocker to expect: the Solid 2 `creates` census refuses callees
   resolved into `solid-js/types/client/hydration.d.ts` (seen on
   `@solid-primitives/scheduled@2.0.0-next.2`, `createSignal`). Fix that
   resolution before reading the new census numbers; the 1.x-era pin
   (`ownerRequirement: 31`, 29/1,890 actionable sites) is the comparison
   baseline and must be recorded beside the new one.
   **Corrected above: resolution is right and the refusal is a deliberately
   withheld row. Only the baseline sentence still stands.**
5. Apply the accepted-tier resolution from the open decision above — this
   sub-step has no default. If the tier is regenerated, it covers only the
   20 packages with a `solid2` row; `@solidjs/start` and `@kobalte/solidbase`
   (21 of 54 bundles) are decided explicitly either way, and
   `make ecosystem-regression` is the gate that shows the receipt loss.
   Then delete `pkg/contracts/bundled/solid-v1/` and
   `benchmarks/package-contract-v2/phase14/solid-v1-authority/`, updating
   `pkg/contracts/bundled/README.md` and the bundle/conformance gates that
   retain those inventories (`scripts/check-bundled-contracts.mjs`,
   `scripts/runtime-lock.test.mjs`).

## Step 3 — delete the 1.x dialect

Only after steps 1 and 2 are green and step 4 is in the same slice or
already landed.

0. Dry run: set `default = ["dialect-v2"]` in
   `rust/crates/solid-facts-backend/Cargo.toml`. **Done 2026-09-16; this is the
   measured blast radius.** The workspace still *builds* — nothing outside the
   backend references the v1 crate directly — and
   `solid-facts-backend --lib` passes at 545. Four integration targets fail,
   ~40 assertions, and they sort into three groups:

   - **~33 in `dialects_process`**, almost all named `solid_one_*`: the v1
     vocabulary and compiler-integration tests. They go with the dialect.
   - **Three that are already prepared.**
     `component_ref_callbacks_are_setup_time_outputs_in_both_dialects` and
     `returned_event_handler_factories_preserve_deferred_execution` fail only
     because `DIALECT_INDEPENDENT` still lists `"solid-v1"` — a one-line edit —
     and `the_dialect_pair_reports_different_findings_from_identical_sources`
     is the deletion whose loss is already verified as contrast-only.
   - **Six that need reading, not deleting**, because their names do not say
     v1: `component_identity_combines_type_facts_with_dialect_compatibility`,
     `preferences_are_default_on_with_explicit_disables_winning`,
     `project_rule_options_disable_one_exact_catalog_rule`,
     `run_with_owner_distinguishes_null_definite_and_nullable_owners_in_both_dialects`,
     `an_incompatible_core_package_requires_a_dialect_change_not_a_receipt`, and
     three `cross_file_*_process` tests. Each is either a pair loop to narrow or
     a v1 fixture to retire, and each needs the same read-the-claim treatment
     slices 3 and 8 needed.

   Nothing in the Bun gates failed in the dry run, because they select the
   dialect from fixture stubs rather than from the build.

**Step 4's emission must land before or with this step (ADR 0110 § 1), and it
is not done.** The catalog identity is mechanical — a `Rule` variant, a
metadata row `("SC9013", "unsupported-solid-runtime", "error", true)`, an
`evidence` arm, `Rule::ALL` 26 -> 27, the counts in `docs/rules/README.md` and
`rust/ARCHITECTURE.md`, a rule page, and one row in
`packages/cli/lib/rules-solid-v2.json`. The part that needs design is the
emission seam: the refusal has to reach the reporting path without passing
through the rules engine.

**Correction (2026-09-16, same day): the landing site first recorded here was
wrong, and the way it was wrong is the design constraint.** It said
`main.rs` immediately before `analyze_project_accepted_measured_with_enablement`
(~line 3375). That line is unreachable for the most common released path.
`detect` has **three** call sites, not one:

- `main.rs:2689`, the one-shot process path;
- `daemon.rs:127` (`resolve_dialect`), the retained per-project daemon;
- `bin/solid-checker-session-bench.rs:77`, the benchmark harness.

`daemon::enabled()` defaults to **true whenever `debug_assertions` is off**, and
`daemon::eligible` is satisfied by exactly the ordinary project check
(no `--sources`, no contract emission, no probe plan, `default`/`json`/`text`
format). So a release CLI checking a 1.x project takes the branch at
`main.rs:2818`, `daemon::check` returns `Ok(code)`, and **the function returns
at 2820 without ever reaching 3375** — while the daemon's own `resolve_dialect`
called plain `detect` and silently collapsed the 1.x install to v2. A refusal
placed at 3375 would be correct in debug, absent in release, and no gate here
runs a release binary against a 1.x stub.

The refusal therefore belongs at the **selection** site, `main.rs:2689`, which
precedes the daemon branch — construct a `diagnostics::Snapshot` with one
`SnapshotFinding` whose `primary_location` is the deciding `package.json`, hand
it to `snapshot_emission::emit`, and return before the daemon is consulted at
all. `detect` becomes `detect_detailed` there. The other two sites need their
own guard rather than the same one, and neither is a user-facing report path:
`resolve_dialect` already returns `Result` and errors on an unknown dialect id,
so `Detection::Unsupported` fits that shape for a direct `--serve`, and the
bench can fail the same way. Pin the release path specifically — a test that
only ever runs a debug binary cannot see this.

A rule identity with no producer is not a shippable unit, so the catalog entry
and the emission land together.
1. Remove `rust/dialects/solid-v1/` from `rust/Cargo.toml` workspace members
   and the backend's dialect registry; remove `solid_1x.rs` from
   `solid-dialect`; remove the two modules `solid1x_attributes.rs` and
   `solid1x_undef.rs` that `carries_eslint_era_rules()` gates wholesale at
   `upstream_compat/mod.rs:703`. **The predicate has six call sites, not
   that one**, and the other four live in modules that stay: it selects
   `folds_dom_slots` in `solid1x_syntax.rs:55`, an async-argument branch in
   `solid1x_structure.rs:88`, and the `v1/prefer-for` / `v1/prefer-show`
   rule names in `mod.rs:736`, and it is the trait method at
   `solid-dialect/src/lib.rs:1699`. Deleting the predicate means resolving
   each of those to its v2 branch — a semantic edit that can move findings, so
   give it its own slice and its own coverage run rather than folding it into
   a deletion commit. The rename of `solid1x_syntax/structure/options.rs` to
   dialect-neutral names travels with that slice, since those modules stay. **Keep `Version::V1` in the `solid-dialect`
   enum**: the step 4 refusal needs to recognize 1.x to refuse it. Remove the
   `dialect-v1` feature, its two arms in `scripts/verify.sh` (lines 173 and
   183; the two `dialect-v2` arms at 178 and 188 then just duplicate the
   default build — collapse them), and every
   `#[cfg(feature = "dialect-v1")]` branch in `dialect.rs`, including the
   `default_dialect` fallback.
2. Drop `packages/cli/lib/rules-solid-v1.json`, the v1 arms in
   `adapter.test.mjs` and `published-contract-graph.test.mjs`,
   `scripts/audit-solid-1x.mjs`, `scripts/dialect-audit-yield.mjs` if it has
   no v2 use, `docs/solid-1x-api-surface.md`, `docs/rules/v1/`.
3. Delete the 1.x halves of the divergence pairs and the 1.x-only fixtures
   classed in step 1, their `.gitignore` exception lines, their
   `findings-snapshots`, and `stableMainDocuments` entries for removed
   `package-contracts` fixtures (a removed fixture moves that ledger).
4. Record `v1/jsx-no-undef` and `v1/prefer-classlist` in the removed-rule
   ledger in `dialect.rs` (the `("<rule>", "removed <date>: <reason>")`
   table near line 60) and in `docs/precision-backlog.md`; rename every
   `v1/` reference there. The catalog-count test in `dialect.rs` (near line
   837) reads `docs/rules/README.md` and `rust/ARCHITECTURE.md`; update both
   or it fails.
5. `bun scripts/dialect-manifests.mjs validate` must still pass with one
   manifest; update its test that expects two.
6. Run `cargo +1.97 fmt` and workspace `clippy --all-targets` immediately
   after the reshaping, before `make verify`; test-only literals break only
   there.

## Step 4 — refuse 1.x projects explicitly

`rust/crates/solid-facts-backend/src/dialect.rs` today resolves the nearest
`node_modules/solid-js/package.json` and falls back to the v2 default when
nothing resolves (`default_dialect`, ~line 347-410). Change the resolved-1.x
branch to an explicit refusal.

This is a signature change, not a branch edit. `detect` returns
`&'static Dialect`, which has nowhere to put a refusal, and
`resolved_solid_version` collapses the three outcomes this step must
separate into one `Option<Version>`: a resolved 1.x version, a resolved but
unclassifiable one (`workspace:*`, `3.0.0` — the walk stops and answers
`None` deliberately), and nothing resolving at all. The refusal also has to
carry *which* manifest was read, which today is a local in the loop. Expect
a new return type through `detect` and its callers before any of the
behaviour below is expressible:

- a resolved version `<2.0.0` produces one project-level `uncertifiable`
  result with a stable code (`SC9013 unsupported-solid-runtime` is free; the
  SC90xx space already holds SC9002, SC9003, SC9005, SC9011 and SC9012; rules live under
  `rust/dialects/solid-v2/rules/`) and no other findings — fail closed, not
  a silent v2 analysis;
- absent or unparsable stub keeps today's v2 default; the message should
  say which `package.json` was read;
- pin both with fixtures: a 1.x-stub project expecting exactly the refusal,
  and a 2.x project unchanged. Add the process test to `dialects_process.rs`
  and a CLI test for the exit status and JSON shape.

Update `docs/adding-a-dialect.md` so it no longer implies two shipped
dialects, and the CLI README's supported-versions statement.

## Step 3 progress (2026-09-16)

**The dry run's failure list, as recorded, was incomplete and partly stale.**
Measured again after step 4 landed, with every integration target and
`--no-fail-fast`: **38 failing assertions across four targets**, not ~40 across
"four targets" enumerated loosely. The classification that matters:

- **24 `solid_one_*`** in `dialects_process` — the v1 vocabulary and
  compiler-integration tests. They go with the dialect.
- **`the_dialect_pair_reports_different_findings_from_identical_sources`** —
  deleted outright, contrast-only, already verified.
- **Six whose scope, not whose subject, was 1.x** — narrowed in
  `aacddf12`, and they now pass in *both* arms.
- **Three `cross_file_*`** — ported in `bc51f4b5`; they were dialect-neutral
  mechanics that merely ran under 1.x fixtures.
- **`an_incompatible_core_package_requires_a_dialect_change_not_a_receipt`** —
  see the product consequence below.
- **`contract_closure_process::the_catalog_bearing_fixtures_mint_a_policy_2_corpus`**
  — a corpus-composition pin the deletion itself moves; re-pin it there, not
  before.

Two of these (`core_runtime_model_needs_no_contract_and_is_not_reported_as_certified`
and the policy-2 corpus pin) were **not** in the original enumeration, and one
of them fails only *because of step 4*: `--check-contracts` on an unsupported
runtime now returns a findings snapshot rather than a contract report. That is
the intended fail-closed behaviour — reporting package status under a language
the project does not run is the silent wrong-language answer ADR 0110 forbids —
but it is a **shape change for that mode**, and it was accidental rather than
designed.

**Pinned deliberately 2026-09-16, and it was hiding a defect.** The shape is
kept — an absent report, not an empty one — because
`packages/cli/scripts/generate-missing-contracts.mjs` read `report.packages`
through `Array.isArray(…) ? … : []`, which turned the refusal snapshot into
"no package needs a contract" and reported nothing to generate for a project
that was never analyzed. Exit code could not catch it either: the report exits
1 when a package needs action, so the sweep accepts 0 and 1, and the refusal
exits 0. Teaching the native side to emit `{ packages: [] }` with a flag beside
it would have moved the same false negative one layer down, for every consumer
that does not read the flag. So the consumer refuses a document that is not a
report, quoting the refusal's own message and hint, and both sides are pinned:
`a_refused_runtime_answers_check_contracts_with_the_refusal_not_a_report`
(`dialects_process`) and "the contract sweep refuses a document that is not a
contract report" (`packages/cli/test/contract-workflow.test.mjs`).

### A product consequence the plan did not anticipate

`PackageContractStatus.status == "unsupported-runtime"` becomes **unreachable**.
It is produced when an imported core package is outside the selected dialect's
`primitive_defining_packages()`. v1 models `["solid-js"]`; v2 models
`["solid-js", "@solidjs/signals", "@solidjs/web"]` — which is the whole union
across dialects, so under a v2-only build every core package is modelled. The
status, its `remedy` ("a core package contract cannot extend the built-in
model"), and its membership in the certification-blocking set all survive as
code with no producer, and
`an_incompatible_core_package_requires_a_dialect_change_not_a_receipt` loses its
subject entirely.

This is a decision, not a cleanup: keeping it matches `Version::V1`, which step 4
deliberately retains so the refusal can recognise 1.x to refuse it, and the next
dialect re-reaches it. Deleting it touches `--check-contracts` output, the
blocking-status set, and any schema that names it. **Recommend keeping it** with
a comment naming the unreachability, and recording it here rather than leaving a
reader to discover an untested status.

### `DIALECTS`: the deletion got much smaller (2026-09-16)

The surface below was measured *before* `bf24c947`, and that commit changed the
number by an order of magnitude. Twenty sites in `solid-dialect/src/lib.rs`
enumerated `[Version::V1, Version::V2]` and immediately took `.dialect()` --
thirteen cross-dialect helpers and seven test loops. None was a claim about
which Solid versions exist; all were asking what the vocabularies on hand say.
They now iterate a `DIALECTS` constant.

Enumerating `Version` there was also subtly wrong, not merely verbose.
`Version::V1` is retained for **classification** long after the build stops
carrying its vocabulary, because detection must recognise an installed `1.9.14`
in order to refuse it. "Which versions exist" and "which vocabularies I have"
had been the same list by coincidence; step 3 is where they part.

**Measured both ways.** Deleting `solid_1x.rs` outright was 178 compile-error
sites across 28 tests in that one file. With `DIALECTS` in place, flipping it to
`&[&Solid2]` leaves **six failing tests**, and all six are genuinely about 1.x:

- `solid_1x::tests::the_authority_denies_creates_for_the_sixteen_and_nothing_else`
- `audited_archives_are_looked_up_by_name_and_carry_all_four_fields` (2 -> 1)
- `implementation_roles_require_canonical_cross_dialect_agreement`
- `one_dialect_answers_for_a_shared_archive_name_only_while_the_other_is_absent`
- `solid_1x_audits_solid_js_1_9_14_and_denies_only_its_sixteen_creates_rows`
- `value_export_modules_answer_only_for_audited_dialect_modules`

So the vocabulary layer is now a one-line edit plus six deletions. The surface
below still stands for everything *outside* `solid-dialect`.

### The deletion's measured surface (outside the vocabulary layer)

Larger than the inventory suggested, and not confined to the dialect crates:

- **`Version::V1`: 63 references**, 50 of them in `solid-dialect/src/lib.rs`
  alone. It stays (step 4 needs it), so each one is a read, not a delete.
- **`Version::dialect()` returns `&'static dyn Dialect` and maps `V1 =>
  &Solid1x`.** Removing `solid_1x.rs` therefore changes that signature or that
  arm — a retained `Version::V1` with no vocabulary behind it is the first thing
  the deletion has to decide.
- **`Solid1xRuleOptions` is a shared IR type**, not a v1 artifact:
  `solid-reactive-ir` re-exports it from `lib.rs`, `pipeline.rs` carries it as
  `solid1x_rule_options`, and six call sites outside its own module use it. It
  is 1.x-*named*, not 1.x-*scoped*, and belongs with the
  `solid1x_syntax/structure/options.rs` rename slice.
- **The IR itself names the 1.x vocabulary**: `PrimitiveName::new("flush",
  &solid_dialect::Solid1x)` and the same for `batch` in
  `solid-reactive-ir/src/lib.rs`.
- 14 `#[cfg(feature = "dialect-v1")]` sites in the backend.

Nothing here changes the plan's conclusion — it changes the estimate. The
deletion is one atomic commit (no intermediate state compiles) over a surface
that reaches into the shared IR, and it wants its own uninterrupted pass rather
than being started at the end of one.

## Verification and handoff

Per slice: the row from the AGENTS.md check table. Before handoff:
`make verify` (grep for `FAILED during step`; a missing `TOTAL` means it
aborted), `make ecosystem-regression`, and `make contract-coverage-census`.
The report must list: every snapshot that moved in substance during step 1
and where it was recorded, split by the three step-1 causes; the
before/after census numbers from step 2, including the change in corpus
size, not only in coverage percentage; how the accepted tier resolved and
how many bundles it holds; the two dropped rules; the exact refusal
behaviour from step 4.

## Out of scope

Closing more contract claim domains, changing the conjunctive open-claims
gate, or adding a `cleanups`/`disposals` consumer rule. Those are the Solid 2
contract-precision work and are tracked separately.

**Slice 8 — the three differential backend tests are one, and it needs no
decision.** `component_ref_callbacks_are_setup_time_outputs_in_both_dialects`
and `returned_event_handler_factories_preserve_deferred_execution` loop both
dialects asserting the *same* property, which is a dialect-independence claim:
the list is the scope, not the property. Both now iterate a named
`DIALECT_INDEPENDENT` constant documenting that step 3 narrows the scope and
keeps the claim. Only
`the_dialect_pair_reports_different_findings_from_identical_sources` is truly
differential, and it is deleted outright — **verified**, not assumed: every
v2-side assertion in it is already pinned span for span by
`fixtures/findings-snapshots/reactive-ir__dialect-solid-2.json` (its six
`SC7001` rows and the absence of `SC3001`). Step 3 loses the contrast and
nothing else.

## Remaining (refreshed 2026-09-17)

**Steps 0, 1, 3 and 4 are done, and step 2 is resolved.** Twenty-four commits
carry the retirement, `08360f56` (the ADR) to `0854fdd7`; `make verify` is green
at the tip. The previous version of this section is superseded -- it still
described step 3 as in progress and step 2 as undecided, and listed three loose
ends that have since closed. What actually closed them:

- **Step 2** was mostly withdrawn by ADR 0110 §§ 4-5 (see the step 2 section).
  What it really needed landed as `3fc4302b`: the coverage census runs
  `--solid 2` and refuses to compare across the retirement boundary.
- **Step 4's two documentation loose ends are done.** `docs/adding-a-dialect.md`
  no longer mentions 1.x at all, and the CLI README documents the `SC9013`
  refusal and says the `v1/`-prefixed rule names went with the 1.x catalog.
- **The `test-backend-v1` question answered itself.** Step 3 deleted the
  configuration, so the arm is gone from `scripts/verify.sh`.

What is left, in recommended order:

1. ~~**`some_audit_denies_primitive` matches on export and domain, never on
   `row.package`**~~ — **fixed 2026-09-17.** The denial lookup is now keyed on
   the package that *declares* the callee.

   - **The defect was real**, demonstrated on a probe package built against the
     audited rc.3 install. `solid-js` re-*declares* `createSignal`,
     `createMemo`, `createStore`, `createProjection`, `createOptimistic` and
     `createOptimisticStore` from `./client/hydration.js`, and the audit
     withholds rows for those six implementations on purpose. They were
     answered out of `@solidjs/signals`' rows anyway: the generator proposed a
     closed `creates` the audit refuses to make, and emitted no decline record
     to say so.
   - **The obvious fix was wrong, and measuring caught it.**
     `ResolvedDeclaration::origin_module` is the module the *import specifier*
     resolved to, so it reports `solid-js` for `untrack` and `createRoot` —
     genuine re-exports — as readily as for `createSignal`. Keying on it
     declined ten legitimate re-exports to fix six re-declarations, and
     `callback-reactive-arguments` caught it.
   - **The discriminator is `ResolvedDeclaration::source_file`**, the file the
     declaration is actually written in, reached through a new
     `callee_declaration_source_file` accessor and mapped to its installed
     package by the deepest `node_modules/` segment. Probed against the real
     install: a `solid-js` import of `untrack` answers
     `@solidjs/signals/.../core/core.d.ts`, of `createSignal`
     `solid-js/types/client/hydration.d.ts`. An unresolved declaration denies
     nothing, which is the safe polarity here.
   - **Measured effect:** declined closure proposals 227 → 312, across 15
     corpus fixtures. Every change is in one direction — **no export closed a
     domain it had not closed before**, and no export's summary changed outside
     the `closed`/`creates` domains. The decline record's own `package` field is
     untouched, so `creates-decline-records` is unaffected.
   - **It also settled a fixture that contradicted itself.**
     `callback-reactive-arguments`' README says "the `creates` domain is not
     closed, so the package makes no claim that it creates nothing", while its
     snapshot closed `creates: []` — granted on `@solidjs/signals`' row, for a
     two-line local stub that plainly does create. The snapshot now matches the
     README.

2. ~~**`callback-deferred-untracked-chain`**~~ — **authored 2026-09-17, with
   the defect it exposed fixed in the same slice.** Not a port: three 1.x
   exports lost their premise under 2.0 (`mountShape` answers `same-stack`
   because 2.0's `createEffect` compute is `DuringCall`;
   `unestablishedScheduleShape` resolves because 2.0 models `createSignal(fn)`
   as a compute slot; `mergePropsShape` has no counterpart at all). The 2.0
   fixture is a four-cell grid over schedule × tracking instead, plus the
   `solid-js/runtime.ts` forwarding seam, which is dialect-neutral and ported
   unchanged.

   Probing it found that the direct-invocation rung published `queued` for
   every tracked callback regardless of the dialect's audited timing — wrong,
   after the retirement, for every 2.0 tracked primitive but
   `createTrackedEffect`. Fixed by recomposing the schedule from the chain that
   produced the word. See `docs/precision-backlog.md` § "Remaining
   approximations". Corpus 95 → 96 fixtures.

3. ~~**`escaping-private-helper`**~~ — **restored 2026-09-17, and it was a port
   after all.** The estimate here was wrong in the safe direction: the fixture
   carries no `jsxImportSource` pragma and no tsconfig of its own, so the
   "arrangement across thirty JSX files" never existed — the generator supplies
   the analysis config, and the stub's `namespace JSX` only ever made `For`'s
   own declaration self-contained. The port needed a new manifest dependency
   and a 2.0 `solid-js` stub, nothing else.

   **All 24 entrypoint/export rows came back byte-identical to the 1.x
   original**, which is the strongest evidence available that its claim — the
   call graph's answer is fail-closed or exact — never depended on a dialect.
   The seven arms of `unresolved-dispatch-reachability`'s README are pinned
   again.

   Two 2.0 shapes did have to be met in the stub, and are: `For` has three
   overloads (only `keyed?: true` keeps 1.x's raw `(item, index)` children), and
   `For`/`Show` return `Element` from `solid-js`' own `types.js` rather than
   `JSX.Element`, 2.0 having moved `namespace JSX` to `@solidjs/web`. The
   absolute-rule check was re-run against the real rc.3 install: clean under
   `jsxImportSource: "@solidjs/web"`, and the generator-config `TS2769` is
   identical with the published package installed, so it is a property of that
   config and not of the stub.

4. **The CLI-level exit-status pin for `SC9013`** — the one item that cannot be
   closed here, and the blocker is now measured rather than inferred.
   `bin/solid-checker-rust` is dated before the retirement commits and **still
   carries the 1.x dialect**: run today it accepts `--dialect solid-v1` and
   analyzes with it. So a CLI-level pin asserting that an unsupported runtime
   exits the way `SC9013` should would not exercise the refusal at all — the
   shipped binary would simply analyze the project.

   Writing it against a stub native binary would pin the launcher's
   exit-code forwarding, which `packages/cli/test/launcher.test.mjs` already
   covers generically, and not the refusal. So this waits on a release that
   rebuilds the shipped binary; AGENTS.md forbids rebuilding it to test a
   source change, and that is the right rule here.

   The refusal itself is pinned at every layer that can be: the Rust process
   boundary (`dialects_process`'s
   `unsupported_runtime_refusal_replaces_the_analysis` and
   `a_refused_runtime_answers_check_contracts_with_the_refusal_not_a_report`),
   the ESLint adapter (`packages/cli/test/adapter.test.mjs`), and the contract
   sweep (`packages/cli/test/contract-workflow.test.mjs`).

Out of this plan, and tracked separately: the negative-authority table is not
condition-aware, which is why `solid-js`' `createSignal`, `createEffect` and
their nine `./client/hydration.js` siblings can carry no `creates` statement at
all. That is an open item in ADR 0007, recorded in `docs/precision-backlog.md`,
and it is the honest reason the first `solid2` census has the hole it has.
