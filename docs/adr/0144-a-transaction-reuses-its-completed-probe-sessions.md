# ADR 0144: A certification transaction reuses its completed probe sessions

- Status: accepted and implemented (2026-09-28); written with the implementation
- Date: 2026-09-28
- Owners: the probe harness binding
  (`rust/crates/solid-facts-backend/src/contract_certification/probe_harness.rs`,
  `launch_every_session`, `ProbeSessionRuns`,
  `PrivateProbeWorkspace::session_reuse_scope`)
- Relation: implements the owner's decision of 2026-09-28 ("make
  certification cheaper") for its largest measured cost. Leaves ADR 0073's
  fresh-transaction rule unchanged: nothing crosses a process.

## Context

Profiling one certification per package with `SOLID_CHECKER_TIMINGS=1`
(release build, the `make certification-metric` flags) put 75-90 % of a
cheap graph-lane row's certification wall in probe-gate batches, and half of
that in batches that had already run. Both gating loops re-run a node's
whole veto batch whenever a pass moves that node's demand graph:

- the graph lane's recipe-gating loop (`certify_graphs_with_recipe_gating`)
  keys a node's gate batch by its demand-graph root, and a withdrawal in the
  gate pre-pass moves the root;
- the plain lane's per-plan loop re-gates the original plan after an
  incomplete gate is withheld.

One withdrawn claim therefore re-launched every other session of the batch
with byte-identical inputs. `@solid-primitives/utils`'s 77-claim batch
(154-156 sessions, about 8 s) ran twice in every row that depends on it, and
twice in its own row.

A session's identity (`session_digest`) hashes the whole runtime-probe plan,
so it moves with the withdrawal even though nothing the session executes does.

## Decision

A certification transaction keeps the completed runs of the probe sessions it
launched, and a later session whose inputs are all equal is answered from that
run, under the new plan's session id, instead of being launched.

- **The key** (`session_reuse_key`) is the exact session frame the worker would
  read with its `id` removed, the recipe module's file name and bytes, and the
  workspace scope. **The scope** (`session_reuse_scope`) is the baseline census
  of every watched input except `recipe-modules` (the private package and
  dependency copies, the harness image, the pinned Node, Type Facts and
  verifier images, the bare-specifier sources and ancestor scopes), plus the
  Node version, the planned specifier, the `--conditions=` flags, the runtime
  and dependency runtime targets, the dependency roots, the closure's requested
  edges, and the inert-execution and JSX-free premises the frame carries, with
  the private directory's path replaced by a placeholder. `recipe-modules` is
  left out because it holds the *other* scheduled recipes and is exactly what a
  withdrawal changes.
- **Only `Completed` runs are kept**, and only after the census that followed
  the run held. A timeout, a throw, a refusal, a worker exit or an isolation
  violation is never replayed.
- **The memo lives in the transaction's `ProbeHarnessConfiguration`** (shared
  by `with_recipe_corpus` clones) and dies with it. It is never persisted, and
  it never crosses a process: a recovery trial and the final transaction after
  it each launch their own sessions, as ADR 0073 requires. A reused run needs
  no census of its own: it reads nothing.
- Timings report `reusedSessions` per batch.

## Why this certifies the same thing

Every input that decides what the session executes is in the key, so a reused
run is the run a fresh launch would perform, and the evaluation
(`evaluate_runtime_probes`) reads only its events, drains, environment and
isolation, not the plan-derived id it is re-labelled with. Repeat runs keep the
distinct isolation identities they were produced with, so the repeat-run
isolation check still sees two processes. The bound harness identity names the
same fields, because those are derived from the plan and the workspace, not
from whether a session launched.

What is weaker: a recipe whose behavior is nondeterministic across launches
was sampled twice before and once now. Repeat runs inside one plan still
launch separately, so a nondeterministic recipe still disagrees with itself
and refuses (`semantic event transcripts differ across isolated repeat
runs`); what is no longer sampled is the second *plan's* pair.

## Measurement

Single-row runs, release build, same flags as `make certification-metric`,
machine shared with other agents (load 10-26), so walls are upper bounds:

| row | before | after | sessions reused |
| --- | ---: | ---: | ---: |
| `@solid-primitives/trigger` | 20.8 s | 14.8 s | 154 of 314 |
| `@solid-primitives/map` | 22.0 s | 14.5 s | 154 of 314 |
| `@solid-primitives/utils` | 29.3 s | 24.3 s | 204 of 516 |
| `@kobalte/core` (certification only) | 165-189 s | 155 s | 340 of 936 |

Every contract document is byte-identical before and after (the same
content-addressed `*.main.json` set in every row), and so is every receipt's
`mainDigest`, `demandGraphRoot`, `probeGateRoot`, `closedClaimsRoot`,
`semanticDigest`, `snapshotRoot`, `exportsRoot`, `closureRoot` and
`policyDigest` — except five `probeGateRoot`s of `@kobalte/core`'s four
dependency-graph frontier cases (`./colors`, `./i18n`,
`./primitives/create-register-id`), which already differ between two runs of
the unchanged baseline binary. Signatures, issuer keys, importer paths and
producer-session roots differ between any two runs by construction.

## Tests

`the_probe_gate_tracer_reuses_the_completed_runs_of_its_transaction`: a
repeated batch on one configuration launches nothing and returns the same
evaluation and harness identity; a fresh configuration launches everything
again.
