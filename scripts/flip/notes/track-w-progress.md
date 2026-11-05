# Track W progress

Branch `claude/flip-writes`, from `claude/flip-base` (895fbf806). ADRs 0700-0749 are W's. Plan order: SC9013, SC7001, SC2004/5,
SC3002, SC7006, SC7005, SC7007, DOM dispatch list, SC2006, SC2008, SC6002, SC6001, SC3001, SC2009, SC4002, SC4001, SC2002, SC2003,
SC2001.

## Done

- **SC9013** (ADR 0700): a resolved `solid-js` whose version names no release (`nightly`, `workspace:*`) gets the SC9014 notice,
  blocker `solid-version-unclassified` as its `analysisContext`. Process test `an_unclassified_release_is_analyzed_with_a_named_blocker`,
  fixture `rust/crates/solid-facts-backend/tests/fixtures/unclassified-release-nightly`. No witnesses (0 of 0). The blocker string is
  defined in the backend (`dialect::SOLID_VERSION_UNCLASSIFIED`): `solid-reactive-ir`'s `blockers` module is private and `lib.rs` is R's.

## In progress: SC7001 (ADR 0701), uncommitted in the tree

Built, not yet committed: Dialect question `effect_function_slot` (`solid-dialect` `semantics.rs` `EffectFunctionSlot`,
`dialect.rs`, `solid_2.rs`), `effect_api.rs` `classify_effect_call` reads the slot, `static_api.rs:96` asks every primitive with a
slot. createRenderEffect: index 1, no bundle (an `{ effect }` literal is a violation), no `undefined` guard. createReaction: index 0,
fails only when its tracker runs, so a proven non-callable is uncertain, blocker `effect-returned-function-run-unproven` (new
`EffectFunctionUncertainty::ReturnedFunctionRun`). Owner pass unchanged (unguarded slots keep `Proven`). `check.mjs SC7001`: 4 of 4.
Fixture `fixtures/reactive-ir/rc21-effect-function-slots` (type-checks against the real rc.21 package, `.gitignore` lines added).
Chrome measured: render bundle throws `node._effectFn is not a function`; reaction bundle and render callable quiet.
Left: rule page `docs/rules/missing-effect-function.md`, the message in `rules/src/lib.rs` (`missing_effect_message` names only
createEffect), ADR 0701, rediscovery entry (outside: the judge keys on `MISSING_EFFECT_FN`, which only `createEffect` emits; the
others fail with a TypeError), `make coverage` (static-api fixture may move), coverage-update, ownership-gate, verify.

## Machine state (this Mac)

- rc.21 corpus rebuilt at `../solid-checker-rc21-frozen/apps` (2026-11-05) from the local rc.17 corpus, which matches
  `~/EDF/tmp/corpus-freeze` exactly (commits, frozen file hashes, nothing else changed). Only `2.0.0-rc.17` values of the seven
  packages moved (163 in 73 files); the repos' own rc.3/rc.4/rc.6 metadata pins were left (kui's comment block, app-game's
  peer allowance, error-menu-web's compiler hold). Installs: rc.17 build's original commands, all 23 OK; all 34 projects resolve
  the exact rc.21 triple. Script `~/EDF/tmp/flipw/install-rc21.mjs`.
- **Baseline sweep differs, corpus not in use**: flip-base build gives 16,237 (15,854 U, 383 V) against the other machine's
  16,419 (16,036 U, 383 V). Violations match rule by rule; 182 uncertifiable short. Per-project table:
  `~/EDF/tmp/flipw/sweep-base-per-project.md`, report `~/EDF/tmp/flipw/sweep-base.json`. Waiting on the owner (needs the other
  machine's per-project report to explain).
- Base build for comparisons: worktree `~/EDF/tmp/flipw/base-wt` (895fbf806, own target), binaries `~/EDF/tmp/flipw/bins/base/`.

## Surprises

- Pre-existing at the base (fail at 895fbf806 too, not W's): `dispatch_veto_reads_process::changing_a_receivers_type_in_another_file_moves_the_veto`
  and `rule_quality_process::eslint_plugin_solid_two_corpus_matches_native_rule_semantics` (SC9012 count 5 vs 3). So `make verify`
  fails at the base. `invocation_flow_process::a_relinked_pnpm_store_entry_is_resolved_again` and `scripts/fuzz/exec.test.mjs`
  failed under load and pass alone.
- `make verify-delta` escalates to full `make verify` whenever `docs/rules/` or `.gitignore` changes (no row maps them).
- `scripts/flip/tsc.mjs` needs an `App` export; to type-check a fixture, copy its source to a temp dir and append one.
