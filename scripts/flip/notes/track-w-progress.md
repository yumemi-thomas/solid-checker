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

- **First merge** (5ddaadd4b, claude/flip-reads cee52c939): conflicts in decisions.md (both kept), rediscovery.json (union, 173),
  one snapshot (R's side, regenerated: +W's two SC7005 obligations). Then 90c6e5e73: two SC7005 process tests updated to ADR 0703
  (they escaped SC7005's verify-delta); 059beb67a: fixture README budget 480,000 -> 484,000. make verify after: all lanes pass
  but load-timing tests (development-feedback 1 s timeout, fuzz exec), each passing alone. Witnesses after merge: 178 of 335
  (incl. 24 HOST); 49 more hold (all R's rules), 0 of W's regressed; 4 HOST fail as R marked `open` (h10, h11, h12 NoHydration,
  co-read): h11 is an SC2001 violation the hydration host contradicts, R's to close.
- **D13 / client_build**: R's merge already did it (`static_api.rs:40`: `!selects_server_build() || client_root_present`, the
  `lib.rs` comment fixed). No W edit.
- **SC7006 D12** (ADR 0706): `valid_use_server.rs` `captured_bindings`: a value reference in an extracted, nested server
  function whose declaration is in an enclosing function (or is its own name) is a violation, proven from the binder; rc.21
  `transformDirectives` measured on 12 shapes. Fixture `valid-use-server-capture`. Corpus 0 moved.
- **SC7007 SSR follow-up** (ADR 0707): D9 namespace member calls (`namespace_member_function` fallback), D10 the opt-in is the
  `enableRichArguments()` call (proven only unconditional at module scope in the client entry before its render, matched by
  import binding: a zero-arg call has no resolved-call fact; an import otherwise Unresolved, never Disabled), D11 `-0` (a zero
  constant or numeric literal domain proves nothing; an unsigned decimal literal as argument or graph leaf does), D4 a leading
  `undefined` before another argument is uncertain. Not built: D5, dynamic `import()`, a client-reached non-entry file
  (open-work). Fixtures `server-function-transport-audit`, `server-function-rich-args-entry`. Corpus 0 moved.
  **Trap:** `verify-delta` does not select `diagnostics_process` for `server_rules.rs`; run
  `cargo test -p solid-facts-backend --test diagnostics_process` by hand after any server rule change (it caught 3 stale
  tests here, 2 at the merge).
- **Post-merge baseline sweep**: `~/EDF/tmp/flipw/sweep-merge.json` (059beb67a, binaries `bins/wmerge`): 16,835 (16,452 U,
  383 V). Diff later rules against it.

- **SC2006** (ADR 0708): `static_api.rs`, an `else` after the step proof: `kind` throws in a step, the project names a stepping
  primitive (`indexes.rs` `project_steps_action_callbacks`, by name: `project_primitives` holds every named symbol, so real
  typings always pass), and `runs_unplaced_through_callers` (`unplaced_run` at the call, or at an exact call of the named helper
  around it, through 4 helpers and the in-place climb's call): uncertain, blocker `function-run-unplaced`. First cut used
  `unplaced_run` alone: the fuzzer's `wrapper` binding (`wrapUse` at module scope, called from the slot) stayed 37 false clean;
  the caller walk closes them. Witnesses a02-a10 hold (185 of 335; b05 moved from certified to an SC2006 obligation, still
  failing for SC2008). Fuzzer `flw-action-slot` (FLUSH_IN_ACTION): old 209 false clean of 379, new 0. Fixtures
  `rc21-flush-in-action-unplaced`, `rc21-flush-unplaced-no-action`; 4 snapshots gained obligations (SC2008 fixtures whose
  handlers and members now ask). Corpus +21 U SC2006 (`sweep-w2006.json`), 0 removed. fuzz-patterns 747 fine (was 744).

- **SC2008** (ADR 0709): ADR 0639's net in `static_api.rs` also asks when `runs_unplaced_through_callers` holds and the project
  names a primitive that defers a callback (`indexes.rs` `project_defers_callbacks`; `action` counts, so SC2006's fixture gets
  SC2008 obligations too); blocker `function-run-unplaced`, the net's filters kept. Covers the bundle `effect` and named apply
  the plan listed. Witness b05 holds (186 of 335). Fuzzer `flw-apply-slot-module` (holder hoisted, apply calls it through raw
  text: the validator refuses a component reference to a hoisted name): the old build is withheld in the grid by a remote
  `reactive-dispatch-unresolved`, so rediscovery says `outside`. Fixture `rc21-flush-in-effect-unplaced` (stubs in
  `node_modules`: an ambient `declare module` gives no `callee_package_root`, so the proven SC2008 violation vanished).
  rc13-flush-in-effect-callback negatives never placed (`void onClick`, a `let` helper, a factory member) are obligations now.
  Corpus 0 moved (`sweep-w2008b.json`). Doc budget: trimmed READMEs rather than raise it.

- **SC6002** (ADR 0710): `static_api.rs`, a second arm after the proven walk: creating call, client build, the project defers
  a callback, `runs_unplaced_through_callers(.., ownerless: true)`, and `runs_off_every_apply` false (role gate: handler with
  no sync dispatch, deferred, directive, rendering, module, discarded). With `ownerless`, each level stops at a site directly in
  a synchronous primitive callback other than `untrack`'s (`owner_set_around`) or in a may-be-component body. First cut asked
  component bodies: 657 corpus sites, nearly all components; then the role gate. Witnesses e05 f04 f07 hold (189 of 335).
  fuzz-patterns FIXED `census/h11-custom-element-connected` (moved to rediscovery). Fuzzer: `flw-apply-slot-module` for
  PRIMITIVE_IN_EFFECT_CALLBACK, outside like SC2008 (old build withheld in the grid). Fixture `rc21-create-in-effect-unplaced`.
  11 fixture snapshots gain obligations (handlers held by value, named applies, static initializers). Corpus +271 U
  (`sweep-w6002c.json`), mostly exported hooks.

- **SC3001** (ADR 0711): `cleanup.rs` `primitive_identity_proven`: a leaf operation is a violation only if the callee is a
  named import whose imported name the dialect maps to the attributed primitive, or a namespace member of a dialect-owned
  module; else uncertain (lexical) or possible (helper). The two corpus `createOwner` violations (solid-three `storePath`,
  inferay `recordUiTimings`) appear only with `--runtime-target browser` (the producer puts `createOwner`'s symbol there; not
  reproduced in small programs, producer is R's). Now uncertifiable; violations 383 -> 381 (`sweep-w3001.json`). Fixture move:
  engine/eslint-reactivity-v2 `leaf-reexport-cleanup` (renamed barrel re-export) violation -> uncertain. **Owner / merge:** the
  named blocker `leaf-operation-identity-unproven` needs a field on `LeafOwnerOperation` (lib.rs, R's) and its wording
  (rules lib.rs); today it reuses the uncertain/possible wording. Witnesses 13 of 13 keep.

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
- Research notes: `~/EDF/tmp/flipw/research-sc4002/{sc4002,sc4001,sc2003}.md` (SC4001: clean half already done by ADR 0534, a
  measured false positive at owner_context.rs:242, harness artifact in run.mjs first; SC2003: no new for_targets fact, for-head
  writes need a non-empty iterable, V-D1 patch applies, W closes 8 of 12; SC4002: Open state + lexical + wrapper proof).
- SC2001 research done (`~/EDF/tmp/flipw/research-sc2001/sc2001.md`): gate `unreached_write` on "site may run" (not
  `unplaced_run`, which misses u11), exclude module initialization, setters are not forced Unknown by `alias_withheld`;
  order SC2002 first, SC2001 last; no reachability edit; 1 corpus site newly U. SC4002/SC4001/SC2003 research running
  (`~/EDF/tmp/flipw/research-sc4002/`).

## Next (a fresh agent starts here)

Order left, per PLAN.md "Track W" and the research (SC2006 done, ADR 0708; `runs_unplaced_through_callers` in
`static_api.rs` is the helper-aware form of `unplaced_run`, reuse it for SC2008/SC6002):
2. SC2006, SC2008, SC6002 done (ADRs 0708-0710). SC6001 needs no flip (plan).
3. SC3001 done (ADR 0711).
4. **SC2009**, then **SC4002** (`~/EDF/tmp/flipw/research-sc4002/sc4002.md`), **SC4001** (`.../sc4001.md`: fix the run.mjs /
   patterns.mjs import-before-subscribe artifact first; the edit is the measured dead-branch false positive at
   `owner_context.rs:242`), **SC2002** then **SC2003** (`.../sc2003.md`, 5 commits) then **SC2001** last
   (`~/EDF/tmp/flipw/research-sc2001/sc2001.md`: gate `unreached_write` on "site may run", exclude module initialization;
   u11 closes there).
Per rule: `node scripts/flip/check.mjs <code>`, fixture both directions, `make coverage` (read) / `coverage-update`,
`make ownership-gate`, `check.mjs all` diff, corpus sweep against `~/EDF/tmp/flipw/sweep-merge.json` (post-merge baseline;
latest `sweep-w3001.json`), `make fuzz-patterns`, `make verify-delta BASE=HEAD`, and by hand
`cargo +1.99.1 test -p solid-facts-backend --test diagnostics_process` after any server or backend-visible change.
Push after every 2-3 rules: `git push origin claude/flip-writes:claude/flip-writes` (upstream is now origin/claude/flip-writes).

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
- Comparison builds: worktree `~/EDF/tmp/flipw/base-wt` (now at 059beb67a, own target; `git -C` it to another commit and build
  there), binaries under `~/EDF/tmp/flipw/bins/` (`base` = 895fbf806, `wmerge` = 059beb67a, `w7007ssr` = 70bc53c26).

## Surprises

- Load-timing tests fail under a full verify and pass alone: `scripts/fuzz/exec.test.mjs` (deadlines), packages/cli
  `development-feedback.test.mjs` (1 s timeout), `invocation_flow_process` file-stamp tests. Rerun alone, never change limits.
- `verify-delta` stops at the first failing check: run the ones after it by hand (usually only `wasm-compile`).
- rustfmt reflows edited code: a later scripted edit that matches the old text silently fails; match by function name.
- `scripts/flip/tsc.mjs` needs an `App` export; to type-check a fixture, copy its source to a temp dir and append one.
