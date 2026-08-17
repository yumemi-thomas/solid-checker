# Agent instructions for solid-checker

This file is the repository-wide operating guide for coding agents. Read it
before changing code. CONTRIBUTING.md, rust/ARCHITECTURE.md, and the relevant
documents under docs/ remain the detailed sources of truth.

Task-specific procedure lives in `.claude/skills/` as plain markdown any agent
can read. Consult the matching one before starting that kind of work:

- `.claude/skills/add-fixture/SKILL.md` — authoring semantic fixtures, dialect
  stubs, and snapshot updates.
- `.claude/skills/verify-handoff/SKILL.md` — choosing checks proportional to a
  change and writing the final report.
- `.claude/skills/upstream-parity/SKILL.md` — investigating divergences from
  eslint-plugin-solid.
- `.claude/skills/green-commits/SKILL.md` — slicing a large worktree into
  individually green commits.

## Repository shape

solid-checker is a Rust analysis engine with TypeScript/Node adapters. The
analysis pipeline deliberately separates these semantic owners:

- rust/crates/solid-facts owns syntax and normalized fact data.
- rust/crates/solid-facts-backend owns orchestration, diagnostics, and the
  process/session boundary.
- rust/crates/solid-reactive-ir owns project indexes, interprocedural analysis,
  contracts, reachability, and proof obligations.
- rust/crates/solid-dialect owns the shared dialect interface.
- rust/dialects/solid-v1 owns Solid 1.x vocabulary, compiler integration, and
  rules.
- rust/dialects/solid-v2 owns Solid 2.0 vocabulary, compiler integration, and
  rules.
- packages/cli owns the Node CLI, ESLint adapter, package metadata, and tests.
- packages/wasm owns the WASM adapter.
- scripts/ owns fixture coverage, upstream parity, contract generation, and
  packaging workflows.
- fixtures/ contains focused semantic fixtures and expected findings.
- schema/ and pkg/contracts/bundled/ contain versioned public contract artifacts.

Keep these seams explicit. Do not move TypeScript-Go or Oxc nodes across fact
interfaces, do not put dialect-specific behavior in shared code when the
dialect seam can express it, and do not turn the analyzer into one monolithic
reactivity rule.

Use the canonical vocabulary defined in CONTEXT.md — fact domain, finding kind
(violation vs uncertifiable), failure class, Type Facts session, and the rest —
in code, diagnostics, documentation, and reports. Each entry there lists the
spellings to avoid.

## Safety and dirty-worktree rules

The worktree may already contain substantial user changes. Before editing:

1. Run git status --short and inspect relevant diffs.
2. Preserve unrelated modifications and untracked files.
3. Use apply_patch for source, fixture, documentation, and configuration edits.
4. Never use git reset --hard, git checkout --, git clean, or broad
   deletion/overwrite commands unless the user explicitly requests that exact
   operation.
5. Keep generated changes scoped to the fixture or artifact being tested. Do
   not rewrite all snapshots merely to make a check pass.

If an existing change overlaps the requested edit, understand it first and make
the smallest compatible patch. Do not “clean up” unrelated code while working
on a semantic issue.

## Absolute rule: never report what TypeScript already reports

**If `tsc` reports a diagnostic for the same code against the library's real
published typings, this checker must not report it.** This is not a preference
to balance against others; it is a hard boundary on what the project is for.

The checker exists to prove defects the type system *cannot* express —
reactivity, ownership, execution phase and timing, compiler lowering, and
package runtime behavior. A rule that fires where `tsc` already errors adds no
information, doubles the noise on broken code, and quietly makes the checker a
worse type checker instead of a better reactivity checker.

Applying the rule:

- **Test against the real package typings, not a fixture stub.** This is the
  trap that hides violations of this rule. A stub that types a callback return
  as `unknown` where the real package types it `(() => void) | void` manufactures
  a defect that cannot exist in a real project. Before adding or keeping a rule,
  write the case against the *published* types and run `tsc --noEmit` on it.
- **Fixture stubs must never be looser than the real package** in any way that
  creates a finding. If a stub must be reduced for other reasons, keep every
  signature that a rule's proof depends on byte-faithful to the real one, and
  say so in the fixture README.
- **Different claim about the same code is allowed; duplicate claim is not.**
  Reporting that a returned cleanup is never disposed (an ownership fact) is
  legitimate even where `tsc` also complains about that expression's type,
  provided the finding asserts something the type error does not.
- **These are not exceptions**: better wording than `tsc`, offering an autofix,
  "the user may not run `tsc`", "the project may not be `strict`", or the rule
  being cheap to keep. If the type system covers it, it is the type system's.
- When a rule turns out to be TypeScript's job, delete it and record the
  removal in docs/precision-backlog.md with the `tsc` output that proves it.
  Do not demote it to a warning or hide it behind an option.

## Precision contract

This project certifies behavior; it is not a syntax-pattern collection.

- Never duplicate a TypeScript diagnostic — see the absolute rule above.

- Report a violation only when semantic facts and the execution model prove it.
- When a required fact, symbol, call target, package contract, or compiler
  behavior is missing, fail closed or produce an explicit uncertifiable result.
- Never treat “unresolved” or “not found” as proof of undefined, unsafe, or
  non-reactive behavior.
- Resolve exact symbols and declarations. Do not use the smallest contained
  symbol, name-only matching, regex trust, wildcard trust, or guessed member
  dispatch as a substitute for semantic resolution.
- Preserve legitimate transparent TypeScript wrappers when classifying spans or
  callees, while rejecting ambiguous computed/member targets conservatively.
- External behavior is contract-driven. Unknown packages and callback helpers
  remain uncertifiable unless semantic, type, or exact package-contract facts
  establish the behavior.
- Keep contract schemaVersion at 1; add only backward-compatible fields and
  update validation/tests when doing so.
- Preserve exact Solid 1.x and Solid 2.0 behavior. Do not infer an API from its
  name alone or share vocabulary between dialects without an explicit dialect
  owner.
- Do not implement legacy SolidStart routeData, JSX sorting, .at() preference,
  negative-index style rules, or another unrelated lint rule.

Every semantic branch should have focused positive and negative regression
fixtures. If a branch changes expected findings, update only its snapshot and
record the precision/backlog status in docs/precision-backlog.md.

## Fast implementation loop

Do not start with make verify for every edit; it intentionally includes the
slowest repository-wide work. Prefer one inspection pass, one focused patch,
and one focused check per semantic slice. Do not repeat a command while the
source, fixture, binary, and environment are unchanged. Before launching a
command, identify which fact it will establish; if it establishes nothing new,
skip it.

- Combine the initial status and search into one command:

  ~~~sh
  git status --short && rg -n "relevant-symbol-or-rule" rust packages scripts fixtures docs
  ~~~

  Read only the owning implementation, its nearest test, and the relevant
  fixture before editing.
- Run only one Cargo build/test/clippy process at a time. Parallel Cargo
  commands contend for the same build lock and make progress slower.
- Do not add temporary debug prints as a first diagnostic. Use a focused
  fixture or existing test output first. If instrumentation is truly
  necessary, make it one isolated patch, run one reproducer, remove it with
  apply_patch immediately, and rerun the focused check.
- Do not use network/package installation to investigate a local semantic
  failure; record external-artifact blockers instead of retrying.
- Use SOLID_CHECKER_TIMINGS=1 when diagnosing a genuinely slow analysis rather
  than guessing which phase is slow.

The normal cadence is: focused test after a semantic slice, one coverage or
parity comparison after fixture work, then one handoff verification pass. If a
check fails because a binary is stale, build the required target once and rerun
that same check; do not fan out into unrelated full-suite commands.

### Which check to run

| You changed | Narrow check while iterating | Also before handoff |
| --- | --- | --- |
| solid-facts (AST, normalized facts) | `facts-lib` | universal set |
| solid-reactive-ir (IR, indexes, contracts, interprocedural, rules engine) | `ir-lib` | coverage compare, universal set |
| solid-facts-backend process/diagnostics | `backend-process` | coverage compare, universal set |
| dialects, contracts at the process boundary | `contract-process` | contract conformance, universal set |
| fixtures or expected findings | coverage compare (fresh debug binary) | parity, universal set |
| packages/cli or packages/wasm | `npm test --prefix packages/cli` (or `packages/wasm`) | universal set |
| release or broad architectural work | — | `make verify` |

The named checks:

~~~sh
# facts-lib
cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-facts --lib

# ir-lib
cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-reactive-ir --lib

# backend-process
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-facts-backend --test diagnostics_process

# contract-process
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-facts-backend --test contracts_process --test dialects_process

# coverage compare (after Rust source changes, point at the fresh debug binary)
SOLID_CHECKER_BIN="$PWD/rust/target/debug/solid-checker-rust" \
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" node scripts/coverage.mjs

# universal handoff set
cargo +1.97 fmt --manifest-path rust/Cargo.toml --all -- --check
git diff --check
jq empty schema/solid-reactivity.schema.json
node scripts/dialect-manifests.mjs validate
cargo +1.97 clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings
~~~

Rust library tests need no native binary; process tests, coverage, parity, and
contract generation do. During iteration prefer targeted Clippy
(`-p <crate> --lib`); run the workspace-wide universal set once at handoff, not
after every patch. See `.claude/skills/verify-handoff/SKILL.md` for the full
proportionality rules and the report format.

## Known traps

- **A bare `cargo test` is meaningless for process tests.** Every
  fixture-driven test under rust/crates/solid-facts-backend/tests skips
  silently when SOLID_TYPEFACTS_BIN is unset; a canary test exists only to
  fail loudly. Always arm it:
  `SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts"`.
- **Stale binaries hide source changes.** The checked-in bin/solid-checker-rust
  may lag rust/ source. After Rust changes, run coverage/parity with
  `SOLID_CHECKER_BIN="$PWD/rust/target/debug/solid-checker-rust"`; never
  conclude “no finding moved” from a run that may have used a stale binary.
  Do not rebuild or overwrite bin/solid-checker-rust merely to test a source
  change. The Node CLI launcher override is `SOLID_CHECKER_NATIVE_BIN`, not
  `SOLID_CHECKER_BIN`.
- **Dialect selection follows the installed solid-js.** A project runs the v1
  catalog only when the nearest node_modules/solid-js/package.json above it
  resolves to a 1.x version (rust/crates/solid-facts-backend/src/dialect.rs).
  A missing or unparsable stub silently falls back to the v2 default and can
  make a v1 fixture a no-op.
- **.gitignore blocks node_modules with per-fixture exceptions.** A new
  fixture’s solid-js stub is silently excluded from git add unless its
  `!fixtures/.../node_modules/` exception lines exist — the fixture then
  un-dialects only in CI.
- **Odd upstream heuristics may be deliberate.** Code under
  rust/crates/solid-reactive-ir/src/upstream_compat/ ports eslint-plugin-solid
  0.14.5 (commit 6d3bc311) byte-faithfully. Check the upstream source at that
  revision before “fixing” one; intentional divergences must be declared in
  fixtures/upstream-parity/deviations.json.
- **Snapshot updates travel with the code that moved the findings** — the same
  commit, not the thematically nearest one. Never run coverage or parity with
  `--update` until the non-updating run has shown the exact intentional
  change; snapshot updates record a deliberate semantic change, they do not
  discover what the implementation does.
- **Differential dialect intent is pinned by fixture pairs.** The
  fixtures/reactive-ir/dialect-solid-1x and dialect-solid-2 pair pins where
  1.x and 2.0 deliberately differ; read fixture comments before mirroring
  behavior across dialects.

## Fixtures, diagnostics, and snapshots

Fixtures should isolate one semantic claim. For a new semantic path, include
at least:

- a positive case that must be diagnosed or certified;
- a negative case that must remain clean;
- an unresolved, shadowed, generic, namespace, member, or wrapper case when
  that distinction is part of the behavior;
- an assertion that the result is a proven violation versus an uncertifiable
  result, where applicable.

Prefer exact symbol identity and resolved declaration locations in tests. Add
cross-file cases whenever the implementation claims project-level resolution.
For Solid primitives, test both dialect vocabulary and namespace imports when
those paths differ. The mechanics — directory anatomy, tsconfig and package
stubs, dialect selection, snapshot review and update — are in
`.claude/skills/add-fixture/SKILL.md`.

## Contracts and dependency pins

Contracts describe exact package/version behavior at a trust boundary. Keep
generated contracts tied to the package artifact and the audited dialect
version. Test unknown external behavior as fail-closed; do not add blanket
trust to make a fixture green.

When testing a real external package, use an isolated temporary directory and
record the exact version. Do not modify checked-in node_modules or bundled
contracts accidentally. The repository audits Solid 1.x and a specific Solid
2.0 prerelease; a newer prerelease must be reviewed rather than silently
substituted.

Upstream compiler and TypeFacts dependencies are pinned by revision. If a pin
must move, update the corresponding notice and follow docs/monorepo.md; do not
vendor or float a branch. Reuse the checked-in bin/solid-typefacts when
present; rebuild it with scripts/build-typefacts.sh only when the TypeFacts
revision, protocol, build id, or producer-dependent code changed.

## Documentation and final report

Update the nearest rule page, architecture note, or docs/precision-backlog.md
when behavior or precision status changes. Do not claim an upstream issue is
fully resolved if only one path is covered; distinguish full and partial
coverage and name remaining approximations.

The final report should state:

- what changed and where;
- which focused fixtures and tests ran;
- which expensive checks ran or were intentionally deferred;
- any generated artifacts changed;
- exact remaining fail-closed or uncertifiable cases.

Do not claim perfection while known approximations or fail-closed paths remain.
