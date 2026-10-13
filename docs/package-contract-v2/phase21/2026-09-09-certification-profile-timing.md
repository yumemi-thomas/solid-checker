# Focused certification profile timing

The user identified the full-corpus turnaround as too slow. The preceding
`2026-09-08-original-helper-full.json` took 1,570.421 seconds with two
certification workers. Its certification durations sum to 2,964.5 seconds;
Kobalte core alone took 453.620 seconds. These are observed durations, not
evidence that two workers is optimal.

The inert-initialization full run mistakenly used a debug-profile native
binary for this long measurement. The preceding full run used the `verify`
profile, whose `sha2` dependency is optimized at level 3. Its timing is
therefore not a clean performance baseline for the new run. No functional
coverage conclusion depends on interpreting those timings as comparable.

A live-process observation identified Kobalte's 577-case ordinary discovery
and Corvu graph certification as active work. Kobalte's 577 plans share one
680,909-byte proposal. The discovery request and proposal were retained under
`/private/tmp/inert-discovery-replay/`; the replay request does not retain a
private issuer key. A single planning request was extracted for a short,
actual certification-path comparison.

Built the optimized native executable through `make build-checker-release`
(exit 0, 19.77 seconds). With identical request inputs and structurally
identical complete plan outputs:

| Order | Debug seconds | Release seconds | Ratio |
| --- | ---: | ---: | ---: |
| Debug then release | 0.7541 | 0.1100 | 6.85× |
| Release then debug | 0.8371 | 0.0883 | 9.48× |

All four commands exited 0. Reversing order controls the simplest warm-file-
cache explanation; this is not a comprehensive performance experiment.
The measured command was `--plan-contract-certification` with
`single-planning.json` and `--certification-plan-output`, using the archived
inert debug binary and fresh `rust/target/release/solid-checker-rust`.
Raw observations are in `profile-timings.json` in the retained replay directory.

This establishes a substantially faster focused planning loop with unchanged
output. It does not establish a whole-corpus speedup, isolate hashing as the
sole cause, or benchmark two versus four workers. Use pinned optimized builds
for future package-performance measurements, batch semantic changes before
full-corpus checkpoints, and measure concurrency separately without changing
proof obligations or receipt verification.

Later process inspection identified a second, independent runtime defect:
the CLI worker pool killed only its worker on timeout. Native certification
children survived as parent-PID-1 orphans. One used about 948% CPU while full
verification's incremental performance gate failed at 122 ms against 100 ms.
Two exact task-owned orphan PIDs were subsequently terminated; active workers
and the original report run were preserved. Contention was observed, but no
isolated rerun yet establishes its entire contribution to that gate failure.

The pool now creates isolated POSIX process groups and terminates descendants
on timeout, memory supervision, and worker exit. Ten focused worker-pool tests
pass, including three real descendant-process controls with inherited pipes
(timeout, crash and memory termination). Those controls require settlement
before the descendant's independent lifetime and verify a replacement worker
can serve the next request. This changes resource cleanup, not certification
proof rules, deadlines, or coverage denominators. The already-running benchmark
loaded the old pool and does not measure this fix. Full validation remains due
after that run has ended so the performance gate can have the machine to itself.

The debug full-corpus run was deliberately terminated after more than 75
minutes. Its task-owned process tree received SIGTERM, the owning execution
handle returned exit 143, and a subsequent process inspection found no native
checker left from its archived binary path. No final corpus JSON was published.
Retained projects, catalogs and the heartbeat log remain untouched, but they
are not a completed corpus measurement and must not be globbed into counts.
The last authoritative full counts remain the Sept 8 original-helper report.
This cancelled run predates both the object-root proof and worker cleanup fix.

A clean full `make verify` rerun now uses
`/private/tmp/object-export-clean-verify.log`; its terminal result must be
checked independently. Future corpus measurements should use pinned optimized
binaries and the repaired worker pool after focused changes are batched.

The clean full verification returned exit 0 with `TOTAL 89.98` and no failure
marker. The performance gate passed after the benchmark processes were stopped.
The complete log is `/private/tmp/object-export-clean-verify.log`. This supports
resource contention as an explanation of the earlier failure; it is not a
controlled attribution of the entire runtime difference to either orphan.

## Current host concurrency check

A subsequent user request asked whether the 26-minute run could use more cores.
The host reports 14 available cores and 48 GiB total memory. Current runner
defaults already select 8 generation slots, 20 certification slots and one inner
artifact-analysis slot per certification. No environment override is present.
The historical two-slot run is therefore not a current-default timing estimate.
The existing Make target builds the pinned release executable and uses these
defaults; manually retaining `--certification-concurrency 2` would suppress them.

A fresh `make build-checker-release` completed successfully. The retained exact
single-planning request was then replayed 28 times per batch at five widths,
first ascending and then descending, after a warmup. Every command exited zero
and every complete JSON plan was identical. No receipts were issued or reused.

| Concurrent planning processes | First batch ms | Reverse-order batch ms |
| --- | ---: | ---: |
| 2 | 1030 | 1028 |
| 4 | 582 | 548 |
| 8 | 330 | 330 |
| 14 | 215 | 225 |
| 20 | 233 | 232 |

This CPU-heavy planning slice is about 4.7 times faster at fourteen than at two,
but it is not a full-corpus timing forecast. Twenty was slightly slower here;
the existing default of twenty also serves materialization, I/O and other
waiting work, and earlier mixed-workload measurements support it. No further
global increase or scheduler change is justified by this narrow experiment.

[Measurement and binary identity](2026-09-09-planning-concurrency-measurement.json)
retain all batch results. The harness is `/private/tmp/benchmark-planning-width.mjs`
and full outputs are in `/private/tmp/planning-width-acCF8s`. The previous
process-group orphan cleanup and optimized-build target remain the relevant
implemented performance fixes; this investigation validates the current higher
concurrency rather than inventing another fix. The full-corpus elapsed time with
all recent certification changes remains unmeasured. Use `make ecosystem-benchmark`
for that measurement; no new full-corpus run was launched in this investigation.
