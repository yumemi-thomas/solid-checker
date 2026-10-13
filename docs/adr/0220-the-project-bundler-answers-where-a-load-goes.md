# ADR 0220: The project's bundler answers where a load goes

- Status: accepted and implemented as an experimental opt-in (2026-10-06).
- Owners:
  - `solid_facts::runtime_resolution::RuntimeResolutionIndex`, on
    `ProjectFacts::runtime_resolutions`;
  - `solid_facts_backend::runtime_resolution::resolve_runtime_imports`;
  - the worker `packages/cli/scripts/runtime-resolver.mjs`;
  - the first branch of `specifier_reaches`
    (`solid-reactive-ir/src/attribution.rs`).
- Relation: replaces, when requested, the resolution inferences and premises
  of ADR 0219 decisions 4 and 6 with the runtime's own answer.

## Context

ADR 0219 needs to know which module each import, export-from, dynamic
`import()`, `require` and `import =` loads. TypeScript's resolution answers
for declarations, not runtime modules. It also does not see bundler aliases,
extension lists, directory `main` fields or shims. So ADR 0219 states
premises about relative paths and built-in names. Each one was falsifiable
by a realistic project setup (`rust/target/research/review-r6.md` to
`review-r8.md`).

## Decision

1. **`--runtime-resolution required` asks the project's own Vite.** The
   backend collects every load occurrence from the syntax facts and sends one
   batch to a Node worker. The worker loads the project's installed Vite and
   its `vite.config`, which runs project code, and queries a client
   `DevEnvironment`'s plugin container (`resolveId`). It creates and listens
   on no server and disables dependency optimization. The config is the
   nearest `vite.config.*` at or above the project, up to the repository or
   workspace root. `SOLID_CHECKER_RUNTIME_RESOLVER` names the worker script,
   and `SOLID_CHECKER_PROBE_NODE` the Node executable.
2. **Each occurrence gets one outcome, joined by its exact literal span:**
   a file (path and real path), an external, a built-in, or unknown.
   Unknown covers:
   - an unresolved request or a resolver error;
   - a virtual or query-qualified module;
   - a declaration target;
   - any CommonJS `require` or `import =`, since a browser bundle has no
     `require` and the worker cannot attest that a plugin rewrites one.
3. **With the table present, `specifier_reaches` uses it alone.**
   - A file that is a program file is followed.
   - A file outside the program, an external or a built-in imports no
     project module (ADR 0193).
   - Unknown, or a missing row, may reach anything.
   - Nothing falls back to TypeScript's answer or to ADR 0219's inferences.
4. **Every failure fails closed.** No config, no worker, a load error, or
   the 120 s deadline all leave the table attached and empty, so every
   occurrence is unknown.
5. **A check with the flag bypasses the retained daemon.** Its result
   depends on project code the daemon's identity does not cover.
6. **Off by default.** Without the flag, ADR 0219 applies unchanged.

## Consequences

- An opt-in check is free of ADR 0219's resolution premises. Only ADR
  0193's whole-program premise remains.
- It is stricter than the default, not more productive. On the rc.13 corpus
  it re-adds the 16 obligations ADR 0219 clears in `app-game`, which a
  `require("fs")` blocks there.
- Not yet: production builds, SSR, workers, webpack and other bundlers, OS
  isolation of the worker, pinning the worker image, and fixture coverage
  that installs Vite.

## Evidence

- **Design** `rust/target/research/runtime-resolution/design.md` (a Codex
  agent). This MVP takes its resolveConfig and `DevEnvironment` path, and
  leaves out its isolation, scope binding and auto mode.
- **rc.13 corpus**, release binary, browser host, flag on: the worker
  answers 70–74% of loads per app. Examples:
  - `app-game`: 3,905 of 5,432 loads, 15 s;
  - `solid-groove`: 1,827 of 2,480 loads, 4 s;
  - `sefer`: 1,144 of 1,560 loads, 4 s.

  Against the ADR 0219 default, violations are unchanged at 195 and the 16
  `app-game` obligations return. The sweep's summed wall time rises from
  158 s to 180 s.
- **Unit test:** a missing or mismatched row is unknown
  (`runtime_resolution::tests`).
