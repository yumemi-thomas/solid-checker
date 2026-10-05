# ADR 0193: An application is the whole program

- Status: accepted and implemented (2026-10-05). Owner decision of 2026-10-05:
  an app project is closed-world by default, and a published library stays
  open-world.
- Owners: `default_program_boundary` and its use in `DiagnosticSession`
  (`solid-facts-backend/src/diagnostics.rs`). The boundary's meaning is
  unchanged (`RuntimeEnvironment::program_boundary` and its two IR consumers,
  `owner_node` and `classify_one_component`).
- Relation: changes the default of `--program-boundary` (commit 3403f3133)
  from "open unless told" to "decided by the project's manifest unless told".
  An explicit `--program-boundary`, `programBoundary` in the ESLint adapter's
  runtime settings, or a fixture's `.solid-checker/runtime.json` still wins.

## Context

An exported component or hook is assumed callable by code the build cannot
see. So its props' backing and its owner stay proof obligations, however
completely the project is analyzed. Over the 38-app corpus (after ADR 0192),
1,328 of the 8,250 uncertifiable results rest at least partly on that
assumption:

| Rule | Results |
|---|---|
| `strict-read-untracked` | 562 |
| `missing-owner` | 543 |
| `reactive-handler-frozen` | 114 |
| other | 109 |

In an application, every caller of an export is in the project. Since August
the boundary could be closed by the user (`--program-boundary closed`), but
nobody passes the flag.

## Decision

When no boundary is selected, the nearest `package.json` at or above the
project decides:

1. `"private": true`: an application, **closed**;
2. otherwise, a manifest that publishes an entry (`exports`, `main`, `module`,
   `types`, `typings` or `bin`): a library, **open**;
3. a manifest that publishes nothing: an application, **closed**;
4. no manifest, or one that cannot be read: **open**, so a build that cannot
   tell stays fail-closed.

The default is applied where a check builds its rule options, so the CLI, the
daemon and the ESLint adapter get the same answer. It is part of the
diagnostic identity, so a cached analysis is not reused across boundaries.
Contract generation does not go through this path, so a package's own
analysis stays open-world.

## Consequences

- Closing removes only the assumption of an unseen caller. As 3403f3133
  states, the caller set must still be enumerated exactly. A component handed
  to a receiver as a value, spread, or referenced outside JSX still escapes,
  and a missing reference list is never proof of no callers.
- **Framework-invoked exports.** A file-routed page or a `lazy()` target is
  called by the framework, not by a project call site.
  - A component that nothing in the project references still escapes:
    `classify_one_component` reads an empty reference list as a missing fact.
    So its props keep their obligation, and it is still owned when rendered.
  - A non-component export that no project code calls gets no owner context
    once the open-world seed is gone, so nothing inside it is reported as
    unowned. That can hide a defect the framework's own call would expose (a
    false negative), and never produces a finding.
- A workspace library inside an app's monorepo, if it publishes an entry,
  stays open-world, which is right if other programs import it.

## Evidence

- **Unit test** `the_default_program_boundary_follows_the_nearest_manifest`:
  - private, and unpublished, manifests are closed;
  - `exports`, `main` and `bin` are open;
  - an unreadable manifest and no manifest are open.
- **Gates.** Coverage (165 fixture projects, 859 findings) and the tsc oracle
  gate (103 cases, 27 keystones) are unchanged. No fixture sits under a
  manifest that closes it, and the open-world fixtures keep their default.
- **38-app browser sweep** with no boundary flag (against ADR 0192's
  `a2-browser.json`): identical to `--program-boundary closed` on every app,
  since all 38 are applications.
  - **+1 true positive** (`glasselated.ts:48`, `onCleanup` inside `onSettled`
    in an owned hook; the rc.9 dev build throws `CLEANUP_IN_FORBIDDEN_SCOPE`).
  - **−8 false positives** (`components-return-once` on props every call site
    passes statically).
  - **Uncertifiable:** 8,250 → 7,277 (860 distinct sites removed, none
    added).
- **Misuse ledger:** unchanged at 79 of 123, with no twin moved.
