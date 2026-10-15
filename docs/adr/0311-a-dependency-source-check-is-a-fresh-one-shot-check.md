# ADR 0311: A dependency-source check is a fresh one-shot check

- Status: implemented as a bounded assembly on 2026-10-15; uncommitted and
  awaiting Sol code review. It is **not** a B2, RS1 or RS3 clearance, and it
  does not clear any part of A, P4 inference, P5, P7, release or deletion.
- Owner: `daemon.rs` (`eligible`, `serve`). `main.rs`, the selector, the
  producer, the IR and the CLI adapter are read, not changed.
- Supersedes the retention decision of ADR 0296 for a check whose effective
  selection is nonempty. ADR 0296's text, timings and trust description stay
  as history of the retained design.
- Premise: `/tmp/sc-scratch.noindex/runtime-session-premise-r2.md` (cleared for
  this bounded assembly by `…-premise-review-r2.md`, with its corrections);
  preceded by `runtime-session-premise.md` and its blocked review R1.

## Decision

For an ordinary invocation of this native executable whose effective
`Request.dependency_sources` is nonempty after the existing default selection,
the executable does not consult or start its retained analysis daemon. It uses
the existing one-shot path, which opens a new original Type Facts
producer/program for preparation, drops it, and opens a new analyzed Type Facts
producer/program. No program, original inventory or fact table from an earlier
native invocation supplies that answer. Direct `--serve` with a nonempty
selection refuses before socket or state initialization.

### Entry points and ordering

- `main.rs` rejects the opt-out with an explicit selection, then applies the
  implicit `*` default (declared target **and** build, ordinary mode, no
  explicit selection, no opt-out, and a project that establishes the resolver)
  before the daemon branch. A target alone is not the default, and the project
  refusal can leave the effective selection empty.
- `daemon::eligible` gains one condition: the request's `dependency_sources`
  is empty. Named lists, `*`, duplicates, combinations, a selection that later
  selects nothing, and the implicit default all bypass the daemon, decided from
  the request alone, never from an earlier answer or a preparation output.
- `SOLID_CHECKER_DAEMON=1` cannot override eligibility; `0` already chooses
  one-shot. No `--no-daemon` flag exists and none is added.
- `--serve` is handled before `eligible`, and the implicit default excludes it,
  so `daemon::serve` refuses a nonempty selection as its first statement,
  before the live-socket early success, the bind, any state and any producer.
  A bare `--serve` stays a base server even with a declared host.
- `handle` keeps its dependency-selection and runtime comparisons. Every
  operationally opened server has an empty selection, so a nonempty wire request
  fails that existing check and never reaches a retained worker. The obsolete
  wire field and the unreachable dependency worker (`DependencyState`,
  `OriginalProgram`, `Sync::Reprepare` and their tests) are **not** removed here.
- Older dependency-source sockets and processes are neither consulted nor
  terminated: the implementation identity already separates running images.

### What it removes

The retained dependency daemon's reuse across invocations of its original
inventory and, when preparation compares equal, of the analyzed program's
facts and bindings. Both stay stale after an input a retained program does not
watch changes (ADR 0296 named declaration files, extended tsconfigs and
untracked `node_modules` files). A fresh invocation constructs a new client
session and producer child (`TypeFactsSession::open`, `Connection::spawn`) and
a new overlay and program (`OpenProjectWithConfig`, `buildProgram`); old
compiler caches and client tables do not cross invocations. Reusing an
identical generated configuration file is not reusing a program.

### Errors and fallbacks, unchanged

Propagated original-open, analyzed-open, preparation or analysis errors
return an error and print no stale or partial snapshot. Two existing paths are
not "always an error" and stay as they were: a generated-program write failure
under an **implicit** default is noted and analyzed freshly in contract mode,
whereas an explicit selection errors; and a selection refusal can deliberately
yield no selected program. TypeScript can also recover from a missing
declaration, so a deleted declaration is not asserted to fail.

## What this does not claim

- It is native-CLI routing. It does not cover the retained session library
  (`NativeIncrementalSession` used directly), WASM, adapter callbacks that do not
  invoke this executable, or a caller-supplied producer executable that is not
  honest.
- It promises no coherent filesystem snapshot and no unchanged inputs during one
  run: the original open, the selector walk and the analyzed open read at
  different instants. In-run coherence (RS3) is **open**.
- It is not the primitive-runtime question (RS1): root agreement is not an
  exact runtime edge, and no primitive exemption is made or implied.
- **Base mode (no effective selection) is unchanged**, including its trust
  limits. It performs no current dependency-source preparation and installs no
  selected-source redirects, but it does analyze project source, and its
  findings can depend on Type Facts declarations and host inference. Its
  retained freshness is neither improved nor certified here. Future P4 source
  selection must revisit this routing.
- Existing authorized premises (the Vite/Solid plugin, the standard runtime
  configuration of ADR 0266, package integrity of ADR 0300, the Window realm of
  ADR 0295) are neither reopened nor widened. They do not assert that every
  full subpath, copy or escaped entry of a primitive package is an audited
  implementation.

## Held

B2 (complete input freshness for positive source authority), RS1, RS3 and every
part of A stay held. A recorded filesystem-operation manifest from the producer
is a **held candidate**, not an established fact: the producer's compiler-host
file system (`overlayFS`) is a useful interception point, not an audited
universal input boundary. Non-VFS reads (`filepath.EvalSymlinks` for runtime
identity, the generated `-project-config` bytes read before the overlay exists),
overlay clones, virtual libraries, host and resolver caches and replayed old
resolutions each need provenance, and conflicting observations of one path,
inherited cached observations, negative probes and a stated coherence/ABA policy
need a premise of their own. It requires its own bounded premise before any
implementation.

## Consequences

- Every dependency-source check, including every lint pass of the ESLint
  adapter that reads package source, pays a full project check synchronously.
  The adapter shares one snapshot within a file's pass only
  (`passSnapshots`, `eslint.cjs`) and invokes native with `spawnSync`. Cost,
  measured on matched release images, is below. `benchmarks/verify-performance`
  and `compare-performance` measure the retained library session, not this CLI
  route, so their reuse thresholds are unchanged; `measure-retained-memory`
  requests a base check and keeps its `--serve` process.
- `packages/cli/README.md` no longer promises retention for all optimized checks.
- The retained dependency-source tests became fresh-routing and freshness
  controls; base daemon regressions (idle watchdog, truncated response,
  membership, relative paths, failed refresh) are built with an explicit
  `--no-dependency-source` so none silently turned into a fallback test.

## Measured cost

One corpus project (`probus-hk`, 141 `src` TypeScript files, rc.13 corpus, a
qualifying default: declared target and build, Solid Vite plugin installed), AC
power, other agents' tests running (load average 3 to 6), serialized under
`sc-heavy`. Baseline: the release image of the reviewed source that became
`2c24840bf` (retained dependency-source daemon, SHA-256 `e9961203…c529d`).
Candidate: this change built as release through `make PROBE_NODE=… build-checker-release`
(SHA-256 `91810ec0…d10d245`). Same producer (`5ab60066…`), Node 24.21.0, one run
per cell, five native checks each, **not a benchmark**: single runs on a busy
machine, a project and a day.

Native, five consecutive checks, daemon switch at its release default:

| Request | Baseline | Candidate |
| --- | --- | --- |
| default (target and build declared) | 5628 ms cold, then 211 to 213 ms, daemon `cacheHit` true | 5304, 5341, 5300, 5319, 5353 ms, no daemon |
| explicit `--dependency-source '*'` | 5473 ms cold, then 212 to 219 ms | 5329 to 5339 ms, no daemon |
| `--no-dependency-source` | 4349 ms cold, then 4 to 5 ms | 4349 ms, then 4 to 5 ms (the recorded rows are equal at this rounding; unrounded clocks were not kept) |

Findings are 139 in every source-mode run and 138 in every opt-out run of both
images. The ADR 0296 figures (0.24 s warm against 5.7 to 5.9 s) are
historical; this run gives about 0.21 s against about 5.3 s for the same
project, a factor of about 25 on a warm repeat.

The actual ESLint adapter (`packages/cli/eslint.cjs`, real `Linter`, one
certification rule, native reached through the launcher with an invocation
wrapper) linting three files for two passes (six file passes):

| Mode | Native invocations | Baseline wall | Candidate wall |
| --- | --- | --- | --- |
| default (declared host) | 6 | 6817 ms (5577 cold, then about 248 each) | 32231 ms (about 5370 each) |
| explicit `--dependency-source '*'` | 6 | 6775 ms | 32309 ms |
| `dependencySource: false` | 1 (the cross-pass cache stands) | 4388 ms | 4385 ms |

The adapter shares one snapshot inside a file's pass and none across passes, so
it already made one native call per file pass before this change; what changed
is that each call is now a full project check. For a source-mode adapter that
is about 4.7 times the wall on six passes and grows linearly with the passes
(each is a synchronous editor stall of about 5.4 s here). The retained actors
eliminated for a dependency-source project are the per-project daemon process
and its two producer processes (ADR 0296); their resident memory was not
measured. The contract opt-out keeps both the daemon and the adapter's
cross-pass cache: the retention behavior is unchanged, and its observed
six-pass lint time was nearly equal in the two images (4388 and 4385 ms). That is
a recorded observation of one run, not a guarantee of equal cost.

## Evidence

Recorded in `/tmp/sc-scratch.noindex/f1-fresh-native/report.md`: the red
baseline (four freshness cases whose requested answer was the stale one, with
`cacheHit` true and generation 2, and the policy cases), the green run, the
focused checks and the measured cost.
