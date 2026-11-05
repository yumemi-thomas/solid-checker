# Track W progress

Branch `claude/flip-writes`, from `claude/flip-base` (895fbf806). ADRs 0700-0749 are W's. Plan order: SC9013, SC7001, SC2004/5,
SC3002, SC7006, SC7005, SC7007, DOM dispatch list, SC2006, SC2008, SC6002, SC6001, SC3001, SC2009, SC4002, SC4001, SC2002, SC2003,
SC2001.

## Done

- **SC9013** (ADR 0700): a resolved `solid-js` whose version names no release (`nightly`, `workspace:*`) gets the SC9014 notice,
  blocker `solid-version-unclassified` as its `analysisContext`. Process test `an_unclassified_release_is_analyzed_with_a_named_blocker`,
  fixture `rust/crates/solid-facts-backend/tests/fixtures/unclassified-release-nightly`. No witnesses (0 of 0). The blocker string is
  defined in the backend (`dialect::SOLID_VERSION_UNCLASSIFIED`): `solid-reactive-ir`'s `blockers` module is private and `lib.rs` is R's.

- **SC7001** (ADR 0701): Dialect question `effect_function_slot` (`solid-dialect` `semantics.rs` `EffectFunctionSlot`,
  `dialect.rs`, `solid_2.rs`); `effect_api.rs` `classify_effect_call` reads the slot; `static_api.rs` asks every primitive with a
  slot. createRenderEffect: index 1, no bundle, no `undefined` guard: a proven non-callable (an object literal included) is a
  violation. createReaction: index 0, fails only when its tracker runs: uncertain, blocker `effect-returned-function-run-unproven`.
  Owner pass unchanged. Witnesses f01-f03 now hold (all: 101 of 311, only these moved). Coverage: dialect-solid-2's message, and
  createReaction call-valued or `as`/`!` arguments in four identity fixtures became SC7001 uncertain; that is the same limit
  `createEffect(count, makeApply(count))` already had (no Type Facts domain for a call result or an asserted value). Corpus: 0
  moved (SC9013 + SC7001, sweep `~/EDF/tmp/flipw/sweep-w7001.json`). fuzz-patterns unchanged. Fuzzer: outside (the judge keys on
  `MISSING_EFFECT_FN`, which only createEffect emits).

- **SC2004/5, SC3002, SC6001**: no flip needed (plan). check.mjs on the SC7001 build: 6 of 6, 9 of 9, 8 of 8. No commit.

- **SC7006** (ADR 0702): **no code change.** The plan's edit was wrong: rc.21's `transformDirectives` (server mode) registers each
  export's initializer at its declaration (`registerServerReference(id, <initializer>)`, specifier form too), so a reassignment
  never reaches registration; today's `export_shape` answers are right (rebound function clean, rebound literal a violation). I
  built the plan's edit first, then measured the compiler (research agent's finding, confirmed) and reverted it. Fixture
  `valid-use-server-rebound` pins both. Witnesses 8 of 8 keep.
- **SC7005** (ADR 0703): `server_rules.rs` `http_response_in_file`: the role gate skips only EventCallback, DeferredCallback,
  EffectApply, DirectiveApply, DiscardedRendering, ModuleInitialization; an undominated call is uncertain, blocker
  `response-head-position-unproven`. Witnesses helper/method/memo/wrapper hold (105 of 311). SSR host: 4 templates added to
  `ssr.mjs` (`m-helper`, `m-method`, `m-memo`, `m-wrapper`); old build 8 false clean, new 18 agree, 0 false clean, 12
  uncertifiable. Cost: shell content (`l-*`, fixture negatives `ShellStatus`, `FallbackNote`, `ShellDeep`, `FallbackDeep`) is
  now an obligation; the shell-root proof (render-entry identity, every caller, cross-file) is open work. Corpus 0 moved (CSR).
- **SC7007** (ADR 0704): `server_rules.rs` `handed_server_function_values`, per client file inside the existing loop: every
  identifier reference that is not a callee, type, declaration/module surface, in server code, or the object of a member other
  than `.call`/`.apply`/`.bind`, resolved through the binder's declaration then `value_symbol` (a shorthand `{ save }` gets the
  property's symbol from the compiler) and `function_for_symbol`; a server function there is uncertain, blocker
  `server-function-value-handed`, unless the serializer is proven. First cut walked symbol references project-wide: the IR read
  ratchet (`make ir-read-lint`) refused the raw `facts.files`/`file_by_path` reads, so it was rewritten. Witnesses b02-b05 hold
  (109 of 311). Corpus 0 moved (a `loadUserData.keyFor` member read moved and was excluded). Runtime not measured.
- **DOM dispatch list** (ADR 0705): rows in both `cleanup.rs` `synchronously_dispatching_dom_member` and `runtime_semantics.rs`
  `delivers_synchronously`: `HTMLFormElement.submit`, global and `WindowOrWorkerGlobalScope.reportError`, `ChildNode.replaceWith`,
  `Node.replaceChild`, `ParentNode.replaceChildren`, `History.go`, `Location.assign`/`reload`/`replace`. Supersedes ADR 0642's
  "form.submit is no hazard". Chrome: submit/reportError throw from onSettled (were certified); navigation measured in an iframe
  (`beforeunload` sync for go(0), reload, assign, replace). Fuzzer: `flip-write.mjs` contexts `flw-settled-form-submit`,
  `flw-settled-report-error` (PRIMITIVE_IN_FORBIDDEN_SCOPE): old 1998 false clean of 2304, new 0. Corpus +25 U (router's
  `history.go` withholds program-wide proofs), 0 removed. Fixture `leaf-scope-dispatch-members`.
- Picked from claude/flip-reads: `ce6a831b2`, `ae2327986` (stale tests; R's progress-file hunk dropped), `ef46d0ac6`
  (verify-delta maps rule pages and `.gitignore`), `aaaf31cad` (maps `scripts/flip/`).

## Owner notes (2026-11-05)

- From Track R's research: a setter in a helper reached only from a getter, `toString`, tag, iterator, field or `Map` slot is a
  W false clean (probe u11). Handle it before SC2001 (it is SC2001's `unreached_write` territory; may land with SC2001's
  prerequisites). Full notes arrive in `scripts/flip/notes/` on claude/flip-reads.
- `reactive_analysis.rs`, `result_reads.rs`, `props_reactivity.rs` are not in the plan's file table; both tracks edit
  `reactive_analysis.rs`, so keep W's edits there small.
- j08 (SC2003) goes to Track R.
- Use `make verify-delta` per rule; full `make verify` only at merges.

- SSR audit (owner, 2026-11-05; report lands in `scripts/flip/notes/ssr/` on claude/flip-reads): fix `client_build` before
  SC2008 and SC6002 (a hydrated project gives a false clean; `lib.rs:289`'s comment is wrong: createRenderEffect runs its apply
  on the server). SC7007 follow-up: namespace calls (`import * as api`), runtime measured with the report's `h/sf.mjs`, add its
  D4, D5, D10, D11. SC7006 follow-up: the capture error D12 (reassigning `export let` is no boot failure: ADR 0702 agrees).
  Track R adds the shared facts `client_root_present` and a server-only arm: take them at the merge, do not build W's own.
- SC2001 research done (`~/EDF/tmp/flipw/research-sc2001/sc2001.md`): gate `unreached_write` on "site may run" (not
  `unplaced_run`, which misses u11), exclude module initialization, setters are not forced Unknown by `alias_withheld`;
  order SC2002 first, SC2001 last; no reachability edit; 1 corpus site newly U. SC4002/SC4001/SC2003 research running
  (`~/EDF/tmp/flipw/research-sc4002/`).

## Next

SC7007 and SC7006 SSR follow-ups and `client_build` once the SSR report lands; then SC2006 (after `client_build`), SC2008, SC6002. SC2001/unreached-write research: `~/EDF/tmp/flipw/research-sc2001/sc2001.md`. A research agent prepared these:
`~/EDF/tmp/flipw/research/track-w-next.md` (machine-local; re-derive if absent).

## Machine state (this Mac)

- rc.21 corpus rebuilt at `../solid-checker-rc21-frozen/apps` (2026-11-05) from the local rc.17 corpus, which matches
  `~/EDF/tmp/corpus-freeze` exactly (commits, frozen file hashes, nothing else changed). Only `2.0.0-rc.17` values of the seven
  packages moved (163 in 73 files); the repos' own rc.3/rc.4/rc.6 metadata pins were left (kui's comment block, app-game's
  peer allowance, error-menu-web's compiler hold). Installs: rc.17 build's original commands, all 23 OK; all 34 projects resolve
  the exact rc.21 triple. Script `~/EDF/tmp/flipw/install-rc21.mjs`.
- **Baseline**: flip-base build gives 16,237 (15,854 U, 383 V) here; the other machine 16,419 (16,036 U). The gap is
  jandibat-web 342 vs 509 (167) and app-game 5,172 vs 5,187 (15); the other 32 projects match exactly. Deterministic on rerun.
  jandibat's installed tree, yarn.lock (patch records included) and Solid copy layout match its rc.17 install except the
  expected rc.21 entries; seroval 1.7.0 is nested under `@solidjs/web` with 1.6.7 at the root, the one layout point a
  machine could hoist differently. Not fully explained; the owner said to use this machine's sweep as the baseline and never
  rebuild or edit the corpus again. Baseline report `~/EDF/tmp/flipw/sweep-base.json` (also the `--order-from`).
- Base build for comparisons: worktree `~/EDF/tmp/flipw/base-wt` (895fbf806, own target), binaries `~/EDF/tmp/flipw/bins/base/`.

## Surprises

- Pre-existing at the base (fail at 895fbf806 too, not W's): `dispatch_veto_reads_process::changing_a_receivers_type_in_another_file_moves_the_veto`
  and `rule_quality_process::eslint_plugin_solid_two_corpus_matches_native_rule_semantics` (SC9012 count 5 vs 3). So `make verify`
  fails at the base. `invocation_flow_process::a_relinked_pnpm_store_entry_is_resolved_again` and `scripts/fuzz/exec.test.mjs`
  failed under load and pass alone.
- `make verify-delta` escalates to full `make verify` whenever `docs/rules/` or `.gitignore` changes (no row maps them).
- `scripts/flip/tsc.mjs` needs an `App` export; to type-check a fixture, copy its source to a temp dir and append one.
