# ADR 0303: The generated program is never written into the project

- Status: accepted (2026-10-14).
- Owner: `dependency_sources::{write_for_process, write_retained,
  logical_path}`, the daemon's program tracking, and the Type Facts
  producer's `-project-config` flag (`tsgo.OpenProjectWithConfig`).
- Amends ADRs 0284 (decision 3), 0296 (the sibling) and 0297 (review round 1,
  read-only projects).

## Context

Source mode analyzes a generated tsconfig that extends the project's own
(ADR 0284). It was written beside the project's tsconfig, because TypeScript
anchors a configuration to its own directory: default type roots, the
compiler host's working directory and `${configDir}` in an extended config.
Moving the file elsewhere would change what it means.

Writing it into the project had three costs:
- a retained daemon (ADR 0296) kept `.solid-checker-dependency-sources.*.tsconfig.json`
  in the user's project for as long as it lived, where a commit could pick it
  up;
- a read-only project could not be analyzed from source, so the default fell
  back to contracts there (ADR 0297, review round 1);
- a gate running the release binary started one daemon per fixture. A
  coverage run left hundreds of daemons (about 25 GB) and their files, which
  invalidated the gate cache while it ran.

## Decision

The generated program's bytes are written to a scratch directory
(`$TMPDIR/solid-checker-programs`, or `SOLID_CHECKER_PROGRAM_DIR`). The
producer opens them with `-project-config=<file>` at a logical path,
`.solid-checker-dependency-sources.tsconfig.json` beside the project's
tsconfig. The bytes are an overlay at that exact path in the producer's
filesystem, so the configuration is anchored exactly as before, every rebuild
reads the same bytes, and nothing is written into the project.

- **One-shot:** a scratch file named by the process id and time, removed when
  the run ends.
- **Daemon:** a scratch file named by a SHA-256 of its domain, the project's
  tsconfig and its bytes. It is reused only when the bytes match, never
  rewritten, and removed when replaced or when the daemon exits. The daemon's
  freshness tracking fingerprints that file. It no longer restamps the
  project directory, because it writes nothing there.
- **Read-only projects** get source analysis like any other. The default no
  longer checks that the tsconfig directory is writable.
- **Gates** (coverage, the ownership gate, the contract corpus and the tsc
  oracle) run the checker with `SOLID_CHECKER_DAEMON=0`.

## Evidence

- `dependency_daemon_process`: no mode, one-shot or retained, leaves any file
  in the project. A read-only project gives the same JSON as a writable
  one, both by default and with an explicit `*` selection, one-shot and
  retained.
- `dependency_sources` unit test: the logical path is beside the tsconfig and
  never exists; the scratch file is reused and never rewritten.
- Coverage: no snapshot moved.
