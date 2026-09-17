# Plan: finish the Solid 1.x retirement and make the next major cheap to add

- **Status:** proposed, not started.
- **Date:** 2026-09-17.
- **Follows:** `docs/adr/0110-the-checker-analyzes-solid-2-only.md`,
  `docs/adr/0111-contract-words-belong-to-the-dialect.md`,
  `docs/2026-09-16-retire-solid-1x-plan.md` (steps 0, 1, 3, 4 done).
- **Goal:** no live code, gate, or governing document still describes or
  executes Solid 1.x behaviour, and an installed Solid major this build does
  not carry is refused with `SC9013` instead of analyzed as 2.0.

Read `AGENTS.md`, `.claude/skills/green-commits/SKILL.md` and
`.claude/skills/verify-handoff/SKILL.md` first. One Cargo process at a time.
No coverage `--update` before the non-updating run has shown the exact change.

## Where things stand (measured 2026-09-17)

The retirement is substantively complete: the `solid-v1` crates, the
`solid_1x.rs` vocabulary table, the 254 upstream ownership cases, every 1.x
fixture stub and snapshot are gone, and `SC9013` is emitted from all three
process entry points (`main.rs`, `daemon.rs`, `solid-checker-session-bench.rs`).
What remains is residue in three layers, plus one forward-compatibility hole.

### Update (2026-09-17, later the same day)

**The two tables below are the original measurement and are kept as one.**
Every row of "residue that is live code or breaks CI" has since closed, and so
has the forward-compatibility hole: `Version::for_solid_js` now answers
`Classification::UnmodelledMajor` for any major this build carries no dialect
for, detection refuses it, and `checkDialectStubs` reads the carried majors
from the assembly manifests instead of checking presence alone. Steps 1-4 are
done, step 5 is most of the way, and step 7's smoke gate exists as
`scripts/second-dialect.test.mjs`.

The last four items closed in one slice, none of them behavioural:

- `LeafOwnerOperationKind::primitive()` in `solid-reactive-ir` spelled
  `onCleanup` and `flush` in shared IR. It had no callers -- the catalogs word
  these operations themselves in `leaf_operation_wording` -- so it is deleted
  rather than routed through `Dialect::name_of`.
- `contract_schema_exemptions`' `Version::V1` arm still named `createResource`
  and `on`. No vocabulary stands behind `V1`, so the arm cannot be reached; it
  now answers `false` and stays exhaustive, like `dialect_names` beside it.
- `bundled_first_party_contract_index`'s `"solid-v2"` match reads as an
  oversight and is a decision: the census it selects is an `include_bytes!` of
  one dialect's checked closure, so a second dialect adds its own arm and the
  refusal is the fail-closed answer meanwhile. Stated in a comment.
- `.gitignore`'s exception block named `solid-1x-sources`, a fixture deleted
  with the dialect, and explained itself in terms of catalog selection.

**What is still open.** Three sites, none of them 1.x residue -- they are the
tail of steps 5 and 6, and each needs its own slice because moving wording
moves findings:

- `execution_role.rs:1314` spells `"createEffect apply callback"` in shared IR
  read-analysis context. `Dialect::name_of` exists; the work is threading a
  dialect to the call and deciding whether the phrase is the catalog's.
- `server_rules.rs:572,658,711` build `SC7007`
  `server-function-rich-argument` violations, id and message, in shared IR
  rather than in `solid-v2-rules`.
- `scripts/check-compiler-facts-identity.mjs` reads
  `rust/dialects/solid-v2/compiler/src/lib.rs` by name (lines 43, 93, 190). A
  second dialect's compiler identity would simply go unchecked -- silent
  non-coverage, not a wrong answer -- and `second-dialect.test.mjs` does not
  cover this script.

Checked and already done, against the step 5 list: the `first_party_bundles`
and `diagnostics` package lists ask `primitive_defining_packages()`;
`local_access`, `source_discovery` and the `@solidjs/web` module identities go
through the seam; `findings.rs` takes its wording as a parameter, so its
`onCleanup` literals are test data; and `solid-facts-backend`'s direct
`solid_v2_compiler` re-export is gone.

### Residue that is live code or breaks CI

| Site | Problem |
| --- | --- |
| `.github/workflows/ci.yml:48,50` | `--features dialect-v1` no longer exists in either Cargo.toml; both steps fail |
| `packages/wasm/prototypes/self-contained.mjs:73` | same feature list |
| `rust/crates/solid-reactive-ir/src/effect_api.rs:126,185` | full `Version::V1` analysis arms; ADR 0110 keeps `V1` for classification only |
| `rust/crates/solid-facts-backend/src/dialect.rs:434` | `detect()` collapses `Unsupported` onto the default dialect, the behaviour ADR 0110 § 1 forbids; only test callers remain |
| `rust/crates/solid-facts-backend/src/diagnostics.rs:773,1595` | dead `"solid-v1"` arms (first-party package list, `v1/prefer-for`) |
| `rust/crates/solid-facts-backend/src/package_requirements.rs:35` | `dialect_id == "solid-v1"` branch |
| `rust/crates/solid-reactive-ir/src/upstream_compat/rule_options.rs:172-174` | `v1/`-prefixed option keys honoured as live; pinned by `tests/fixtures/preferences-v1-*` |
| `rust/Cargo.toml:79` | unused `solid1-dom-expressions-compiler` git dependency; comments at 16, 62, 65 cite a deleted crate |
| `Makefile:72-73` | `SOLID_CHECKER_SOLID1_ARCHIVE_ROOT` has no reader |
| `scripts/tsc-oracle-gate.mjs:111,235,318,378`, `scripts/tsc-oracle.mjs:274,285` | accept a `v1` dialect and synthesize `v1/` rule ids |
| `fixtures/tsc-oracle/packages.json:5-8` | live `v1` oracle project installing `solid-js@1.9.14` |
| `scripts/ownership-gate.mjs:40,191` | maps `solid-v1`; rejects any id other than v1/v2 |
| `packages/cli/eslint.d.ts:35`, `packages/wasm/index.d.ts:121,152`, `solid-reactive-ir/src/findings.rs:47` | type surfaces still advertise `"solid-v1"` |

### Residue in governing prose (present tense, contradicting the ADR)

- `AGENTS.md:318-326` ("A project runs the v1 catalog only when ...") and
  `:346` (cites the deleted `fixtures/reactive-ir/dialect-solid-1x`).
- `.claude/skills/add-fixture/SKILL.md:41,59-60` — tells authors to write 1.x
  stubs and read the deleted pair.
- `.claude/skills/green-commits/SKILL.md:49` — "both rules catalogs".
- `CONTEXT.md:160` — rule options "the 1.x rules honour".
- `packages/wasm/README.md:24`, `docs/monorepo.md:25,77-81`,
  `docs/rules/README.md:21,28`, `docs/rule-catalog-migration.md:3,106-107`,
  `rust/ARCHITECTURE.md:130,340-344`,
  `rust/crates/solid-dialect/audited-slices/README.md:47-53`,
  `rust/crates/solid-dialect/contracts/README.md:15`.
- `.gitignore:63-65,106-107` — comments claim 1.x-catalog coverage for
  fixtures that may now pin nothing; `:23-24,45-50` globs match nothing.
- `.github/workflows/ci.yml:219` — cites a deleted script.
- `docs/design-review-remediation.md` — reads as a live backlog but cites
  `rust/dialects/solid-v1/rules/src/*` line numbers.

### Deliberate archives (keep, per ADR 0110 §§ 4-5)

`pkg/contracts/bundled/solid-v1/` (three files; two are `include_bytes!`
inputs to `policy2_receipt/tests.rs`), the frozen accepted-contract tier,
`benchmarks/**` rows and `|solid1|` probe ids, the retired-rule ledger in
`dialect.rs`, `docs/precision-backlog.md`, the retirement plan and ADRs.
`rust/crates/solid-facts-backend/tests/fixtures/unsupported-runtime-v1/` is
the `SC9013` refusal fixture and is legitimate.

### The forward-compatibility hole

`Version::for_solid_js` (`rust/crates/solid-dialect/src/lib.rs:96-108`) maps
major 1 and 2 and answers `None` for everything else. `detect_detailed`
treats `None` as unclassifiable and answers `Detection::Defaulted`, so an
installed `solid-js@3.0.0` is analyzed as Solid 2 in silence. This is the hole
ADR 0110 § 1 closed for 1.x, reopened for every future major.
`scripts/coverage.mjs` `checkDialectStubs()` has the same blind spot: it checks
presence, parseability and tracked-ness, never whether the major is carried.

### Seam readiness, by area

| Area | Verdict | Notes |
| --- | --- | --- |
| `Dialect` trait (`solid-dialect/src/lib.rs:1063`) | partially ready | 26 required, ~30 defaulted conservatively; `Version` and `Primitive` are closed shared enums a new dialect must edit (by design) |
| Selection / registry (`solid-facts-backend/src/dialect.rs`) | ready for registration, hole in detection | `ALL` + `by_id`/`by_version` iterate; `default_dialect()` is a literal; major ≥ 3 defaults silently |
| Shared-code vocabulary leaks | partially ready | see step 5 |
| Compiler / Type Facts | ready | per-dialect compiler crate with per-dialect `COMPILER_FACTS_IDENTITY`; two-compiler precedent in `rust/Cargo.toml`; Type Facts producer is correctly dialect-free; `scripts/check-compiler-facts-identity.mjs` hard-codes v2 |
| Contracts / manifests | ready in Rust, CLI, `dialect-manifests.mjs` | ~8 phase/gate scripts pin `solid-v2` literally |
| Fixtures | ready | stub mechanism is generic; no major validation |
| Rule ids | partially ready | `namespace` plumbed end to end; SC-code allocation across catalogs is convention only; `RETIRED_RULES` is dialect-global |
| Docs | ready | `docs/adding-a-dialect.md` is a real checklist; needs the ADR 0111 audit list |

## Plan

Each step is one individually green commit slice (`green-commits`). Steps 1-3
finish the retirement; steps 4-7 make the next major cheap and safe. Step 4 is
the one forward item worth doing before anything else.

### Step 1 — fix CI and dead build inputs

- Drop the two `dialect-v1` arms from `ci.yml:48,50` and the wasm prototype.
- Remove the unused `solid1-dom-expressions-compiler` workspace dependency and
  the comments at `rust/Cargo.toml:16,62,65`.
- Remove `SOLID_CHECKER_SOLID1_ARCHIVE_ROOT` from `Makefile:72-73`.
- Fix the `ci.yml:219` comment.

Check: both remaining single-feature `cargo check` arms, `make verify-delta`
(Makefile and `rust/Cargo.toml` escalate to `make verify`; run it once).

### Step 2 — delete the remaining 1.x analysis code

- Remove the `Version::V1` arms and `v1_effect_function_status` in
  `effect_api.rs`.
- Remove the three `"solid-v1"` branches in `diagnostics.rs` and
  `package_requirements.rs`.
- Delete `detect()`; retarget its two tests at `detect_detailed`.
- Move the `v1/` option keys in `rule_options.rs` to `RETIRED_RULES` so an old
  `rule-options.json` keeps loading; retarget or delete
  `tests/fixtures/preferences-v1-*`.
- Retire the `v1` oracle project in `fixtures/tsc-oracle/packages.json` and the
  `v1` branches in `tsc-oracle-gate.mjs`, `tsc-oracle.mjs`,
  `ownership-gate.mjs`.
- Fix the `.d.ts` and `findings.rs` comments.
- Record each removal in `docs/precision-backlog.md`.

Check: `ir-lib`, `backend-process`, `contract-process`, coverage against the
fresh debug binary (expect zero movement), ownership gate, tsc oracle gate.

### Step 3 — fix governing prose

Edit every site in the prose table above. AGENTS.md `:318-347` and the
`add-fixture` skill first, because agents act on them. Mark
`docs/design-review-remediation.md` as historical or refresh its citations.
Docs-only slice; `verify-delta` escalates to `make verify`, run it once.

### Step 4 — close the future-major hole

- Change `Version::for_solid_js` to distinguish "not a version"
  (`workspace:*`, `0.5.0`) from "a released major this build carries no
  dialect for" (`3.0.0`), so the latter reaches `Detection::Unsupported` and
  `SC9013`. Option: return an `Unclassified { major }` marker rather than
  adding a `V3` variant nobody can implement yet.
- Pin it with a `3.0.0` stub fixture in `dialects_process.rs` beside
  `unsupported-runtime-v1`.
- Make `checkDialectStubs()` in `scripts/coverage.mjs` refuse a stub whose
  major no carried dialect maps to.
- Update the `Version` and `Detection` doc comments, which still narrate the
  1.x plan steps as pending.

Check: `contract-process`, coverage, and the ESLint adapter test for the
refusal.

### Step 5 — move remaining Solid 2 vocabulary behind the seam

Coverage and the contract corpus must be byte-identical after every item.

- `diagnostics.rs:774-780` and `first_party_bundles.rs:246-265`: replace
  `match dialect.id` package lists with
  `vocabulary.primitive_defining_packages()`. The `_ => &[]` arm is a silent
  wrong answer for a third dialect.
- `first_party_bundles.rs:151,403`, `diagnostics.rs:1547`: replace the
  `solid-js` / `@solidjs/` family tests with a dialect question.
- `source_discovery.rs:440-445,518` and `server_rules.rs:963,979-980`: route
  `@solidjs/web` module identity through `Dialect::modules()` /
  `owns_module()`.
- `local_access.rs:540,882,989` and `source_discovery.rs:1859`: replace string
  matches on `createOptimistic*` / `createStore` with `Primitive` and
  `returns_store` / `creates_reactive_source`.
- `findings.rs:618,636`, `execution_role.rs:1314`, `server_rules.rs:572,658,711`,
  `lib.rs:457`: move catalog wording and the `onCleanup` name into
  `solid-v2-rules`; `Dialect::name_of` already exists.
- Work through the ADR 0111 audit list (`owners.rs`, `static_api.rs`,
  `cleanup.rs`, `indexes.rs`): decide per site whether it is dispatch on
  identity (fine) or role knowledge a future dialect could answer differently
  (add a trait question).
- `solid-facts-backend/src/lib.rs:70` re-exports `solid_v2_compiler` directly;
  route through the registry.

### Step 6 — generalize the hard-coded gate scripts

Have these enumerate `rust/dialects/*/dialect.json` the way
`scripts/dialect-manifests.mjs:24-27` does: `ownership-gate.mjs`,
`check-compiler-facts-identity.mjs:43,93,190`,
`solid-facts-backend/src/bin/solid-contract-bundles.rs:56`,
`package-contract-phase18.mjs`, `phase19.mjs`, `v2-phase0.mjs`,
`lib/tsc-oracle-case.mjs:42`, `ecosystem-benchmark/lib/dialect-authority.mjs:36`.
Make `default_dialect()` an explicit registry or manifest decision rather than
a literal.

### Step 7 — add a second-dialect smoke gate

- Extend `docs/adding-a-dialect.md` with the ADR 0111 audit list and a
  cross-catalog SC-code uniqueness check.
- Add a test that a stub `solid-v3` manifest plus registry entry flows
  through detection, `by_id`, catalog discovery in `eslint.cjs`, and the gate
  scripts from step 6, then is removed. This is the only durable proof that
  steps 4-6 stay true after the next refactor.

## Deferred, deliberately

- The CLI-level exit-status pin for `SC9013` waits on a release that rebuilds
  `bin/solid-checker-rust`; AGENTS.md forbids rebuilding it for a source
  change. Pinned at every other layer already.
- The negative-authority table is not condition-aware (ADR 0007 open item);
  unrelated to dialect retirement.

## Verification and handoff

Per slice: the narrow check from the AGENTS.md table. At the end of the
series: `make verify`, confirmed by grepping for `FAILED during step` and a
present `TOTAL` line. Report which slices moved findings (expected: none).
