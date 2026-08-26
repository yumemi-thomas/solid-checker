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
- scripts/ owns fixture coverage, product-ownership gates, contract generation, and
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
- The replacement package-contract format uses `schemaVersion: 2` only while
  every producer, consumer, bundled contract, fixture, proof sidecar, receipt,
  and gate migrates together. It is then re-emitted atomically as the first
  stable public `schemaVersion: 1`; never ship both meanings of version 1, add
  `schemaStatus`, or treat the temporary number as compatibility. After that
  stable cut, keep version 1 backward-compatible and update validation/tests
  for every additive field.
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
ownership-gate comparison after fixture work, then one handoff verification pass. If a
check fails because a binary is stale, build the required target once and rerun
that same check; do not fan out into unrelated full-suite commands.

### Which check to run

| You changed | Narrow check while iterating | Also before handoff |
| --- | --- | --- |
| solid-facts (AST, normalized facts) | `facts-lib` | universal set |
| solid-reactive-ir (IR, indexes, contracts, interprocedural, rules engine) | `ir-lib` | coverage compare, universal set |
| solid-facts-backend process/diagnostics | `backend-process` | coverage compare, universal set |
| dialects, contracts at the process boundary | `contract-process` | contract conformance, universal set |
| fixtures or expected findings | coverage compare (fresh debug binary) | ownership gate, universal set |
| packages/cli or packages/wasm | `bun run --cwd packages/cli test` (or `packages/wasm`) | universal set |
| release or broad architectural work | — | `make verify` |

`make verify-delta` mechanizes this table: it reads the changed paths (the
merge-base diff against `origin/main` plus the working tree), prints the row it
matched for every one of them, and runs those checks followed by the universal
set. It fails closed — a path no row claims (`scripts/`, the `Makefile`,
`schema/`, `rust/Cargo.*`, documentation, anything new) escalates the whole plan
to the full `make verify` and says which path did it, because an unmapped path
can change any answer here. Note what that does *not* include:
`rust/crates/solid-dialect/` has no row, deliberately — the crate owns the
shared `Dialect` interface that the IR and both dialect crates consume, so a
change there escalates rather than being answered with a narrower check.
`bun scripts/verify-delta.mjs --dry-run` prints the plan without running it.
**`make verify` remains the handoff authority**; `verify-delta` is the fast
loop, and it is only ever as good as its mapping.

**Fails closed for paths git reports — and git does not report everything.**
The selection basis is a merge-base diff plus the working tree, so anything
`.gitignore` hides is invisible to it. Two ignored classes are real inputs, and
the plan prints both as caveats on every run:

- **The build products under `/bin/` and `rust/target/`.** Above all
  `bin/solid-typefacts`, the producer of every fact here: rebuilding it changes
  every answer while `git status` stays silent. So `build-typefacts` — a stamp
  check that no-ops when the binary is already at the pinned revision — is in
  *every* plan, and a `bin/solid-typefacts.buildinfo` whose revision differs
  from `rust/Cargo.toml`'s pin (or is absent) escalates the whole plan. A
  hand-replaced binary at the right revision is still not detected.
- **Ignored fixture inputs.** A `node_modules/solid-js` stub added to an
  *already-tracked* fixture without its `.gitignore` exception lines is invisible
  to `git status`, so no row selects coverage — and `checkDialectStubs`, which
  catches a silently substituted dialect, lives inside coverage. This one is not
  closed. After touching a fixture's `node_modules`, run coverage (or
  `make verify`) rather than trusting a `verify-delta` plan.

A row can also be narrower than its blast radius, which is a mapping bug rather
than a basis one; `pkg/contracts/` is the case that already bit — it now carries
`coverage` and `ownership-gate` because those contracts are compiled into the
binary (see "Known traps").

`make verify` prints each step's wall time as it completes and a summary table
at the end, so a slowdown can be attributed rather than guessed at. Two
environment variables tune the gates it runs: `SOLID_CHECKER_GATE_CONCURRENCY`
overrides the default fan-out of min(cores, 8) for coverage, the oracle gate,
and the contract corpus, and `SOLID_CHECKER_GATE_CACHE=0` disables the
content-addressed result caches (see "Known traps" below) for both reading and
writing.

Its non-performance Cargo steps use the dedicated `verify` profile under
`rust/target/verify`: no debugger symbols and no incremental object cache. A
full feature-matrix run therefore does not multiply the normal `debug` tree.
Focused development commands intentionally keep Cargo's ordinary dev/test
profiles; use those when a debugger or incremental recompilation matters.

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
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" bun scripts/coverage.mjs

# universal handoff set
cargo +1.97 fmt --manifest-path rust/Cargo.toml --all -- --check
git diff --check
jq empty schema/solid-reactivity.schema.json
bun scripts/dialect-manifests.mjs validate
cargo +1.97 clippy --manifest-path rust/Cargo.toml --workspace --all-targets -- -D warnings
~~~

Rust library tests need no native binary; process tests, coverage, ownership, and
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
  may lag rust/ source. After Rust changes, run coverage/ownership with
  `SOLID_CHECKER_BIN="$PWD/rust/target/debug/solid-checker-rust"`; never
  conclude “no finding moved” from a run that may have used a stale binary.
  Do not rebuild or overwrite bin/solid-checker-rust merely to test a source
  change. The Node CLI launcher override is `SOLID_CHECKER_NATIVE_BIN`, not
  `SOLID_CHECKER_BIN`. The checked-in bin/solid-checker-rust currently speaks
  compiler-facts protocol 1 and *refuses the pinned producer outright*, so a
  manual `bun scripts/contract-corpus.mjs` (whose default is that binary)
  fails on the handshake instead of testing anything. Point it at the fresh
  build with `SOLID_CHECKER_NATIVE_BIN="$PWD/rust/target/debug/solid-checker-rust"`;
  `make contract-corpus` is unaffected because it depends on `build-rust`,
  which rebuilds bin/ first.
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
  revision before “fixing” one; retained behavior and intentional divergences
  must be pinned in fixtures/ownership-cases/cases.json.
- **Snapshot updates travel with the code that moved the findings** — the same
  commit, not the thematically nearest one. Never run coverage with
  `--update` until the non-updating run has shown the exact intentional
  change; snapshot updates record a deliberate semantic change, they do not
  discover what the implementation does.
- **Differential dialect intent is pinned by fixture pairs.** The
  fixtures/reactive-ir/dialect-solid-1x and dialect-solid-2 pair pins where
  1.x and 2.0 deliberately differ; read fixture comments before mirroring
  behavior across dialects.
- **Bundled contracts are compiled into the binary.** pkg/contracts/bundled/**
  reaches the analyzer through `include_bytes!` in
  rust/crates/solid-facts-backend/src/diagnostics.rs, so editing one changes
  nothing until the binary is rebuilt. The stale-binary trap above applies to
  a contract-only change exactly as it does to a Rust one: build
  rust/target/debug first, then run coverage/ownership with
  `SOLID_CHECKER_BIN="$PWD/rust/target/debug/solid-checker-rust"`. “No finding
  moved” from a run that used the previous binary is meaningless there too.
- **The gate result caches key on inputs, not on verdicts.** coverage, the
  contract corpus, the ownership gate, and the TypeScript/checker halves of the
  tsc oracle store each unit’s *computed result* under
  rust/target/gate-cache/, keyed by a digest over the fixture tree as it sits on
  disk (untracked files included), the dialect-selection chain *above* that tree
  (every ancestor’s `node_modules/solid-js/package.json`, to the filesystem
  root, because `dialect.rs` walks unbounded and roughly half the fixtures rely
  on there being no stub above them), the checker and TypeFacts binaries plus
  the producer’s `.buildinfo`, the gate script and every local module it can
  reach plus all of scripts/lib/**, any tree the gate *executes* but does not own
  (`packages/cli`, for the contract corpus’s generator), every `SOLID_*`
  variable, the Bun/Node runtime identity, and a format constant — see
  scripts/lib/gate-cache.mjs, whose header is the authoritative list. Snapshots
  and `expected.json` are deliberately *not* in the key: comparison always runs
  fresh, so editing an expectation needs no cache awareness and a mismatch still
  fails on a warm cache. A unit whose tree moves *while the gate is running* is
  computed but not stored: the key parts are a thunk, re-evaluated after the
  unit runs, and a fixed array carrying a filesystem digest is refused outright
  rather than trusted. What this means in practice: a stale green gate is the one
  failure mode that matters, so widen the key rather than narrowing it, bump
  `CACHE_FORMAT_VERSION` when an entry’s meaning changes, and reach for
  `SOLID_CHECKER_GATE_CACHE=0` whenever a result looks impossible. `make clean`
  wipes the cache with the rest of rust/target; `cargo clean --profile verify`
  reclaims the large verification binaries without removing these entries or
  the audited TypeScript installs. Oracle TypeScript observations are separate
  from checker observations so a Rust rebuild does not invalidate diagnostics
  that depend only on the same snippet and published typings. Entries are never
  evicted — one file per (shared digest × unit), and checker rebuilds write a
  fresh checker-side set — so a full clean remains the complete reclamation path.
- **The registry memo stores the falsifier, so it is bound to its inputs.**
  scripts/check-contract-pins.mjs memoizes registry integrity by `name@version`
  under the same `SOLID_CHECKER_GATE_CACHE` switch, in
  rust/target/registry-integrity.json. Entries carry a format version and a
  digest of everything that determines the answer — the effective npm registry
  (resolved the way npm resolves it, so a mirror’s answers never serve an npmjs
  run), this script’s own closure, and the Node version — and anything that is
  not exactly that envelope discards the file whole. A memoized answer that
  *disagrees* with the pin it is compared against is never the verdict: it
  misses, the registry is asked live, and the verdict follows the registry. It is
  still a file in a user-writable build root, so an entry hand-edited to *agree*
  with a pin is indistinguishable from a live answer that agrees — the memo is
  not tamper-proof and does not claim to be. CI’s contracts job has no
  rust/target, so every push still performs the live lookup for every pin.

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

Current builds still consume the Solid 2 compiler and Type Facts as revision-
pinned dependencies. Until the approved source bootstrap lands, moving either
pin must update the corresponding notice and follow docs/monorepo.md; do not
vendor or float a branch. Reuse the checked-in bin/solid-typefacts when present;
rebuild it with scripts/build-typefacts.sh only when the Type Facts revision,
protocol, build id, or producer-dependent code changed.

The approved target moves the Type Facts producer/client back into this
repository and follows the Solid 2 compiler at
`solidjs/solid/packages/compiler`. Follow
docs/package-contract-v2/compiler-and-typefacts-bootstrap.md and do not leave a
mixed local/external producer-client state. The Solid fork is semantic-facts-
only: trace models, output-neutral recording hooks, validation/serialization,
and fact tests are allowed; lowering, generated output, diagnostics, runtime
behavior, compiler features, performance changes, and unrelated refactors are
forbidden. Unrelated dependency changes are forbidden;
semantic-fact-specific dependencies are allowed only when the fact interface
requires them and normal compiler behavior remains unchanged. A compiler defect
goes upstream separately,
and the corresponding checker fact stays open until the semantic branch rebases
onto the upstream fix.

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
