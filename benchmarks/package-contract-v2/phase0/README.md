# Package-contract Phase 0 evidence

This directory is the reproducible legacy baseline and published Solid 2 RC.3
authority record required by Phase 0 of the replacement package-contract plan.
It does not implement the replacement schema and does not promote the current
RC.0 bundled Solid 2 contracts to RC.3.

## Artifact inventory

- `baseline.json` classifies all 418 verifier-selected rows exactly once and
  records pins, schema structure, bundled-contract size, inline evidence size,
  performance, memory, fixture freezes, and every input hash.
- `baseline.md` is the human rendering of the same baseline.
- `measurements/ecosystem-generation.json` records the complete uncached
  generation run.
- `measurements/ecosystem-verification.json` records the independent uncached
  verification run and fresh state directory.
- `measurements/contract-corpus.json` records the uncached legacy bundled-
  contract gate.
- `rc3/audit.json` is the machine-readable audit of exact published
  `solid-js`, `@solidjs/signals`, and `@solidjs/web` RC.3 tarballs.
- `rc3/audit.md` is its human rendering.
- Each directory below `rc3/` preserves registry metadata, the published
  manifest, the ordered export map, and the complete extracted-file hash list.

Raw tarballs are deliberately not committed. The audit binds their registry
URLs, SHA-1, SRI SHA-512, SHA-256, size, safe archive layout, complete file
manifest, source `gitHead`, and concrete export targets.

## Authority boundary

Published RC.3 runtime artifacts and declarations are the behavioral authority
for the replacement format. The current checker still consumes RC.0 bundled
Solid 2 contracts, dialect facts, and TypeScript-oracle inputs. Consequently:

- RC.3 audit results may establish package identity and future conformance
  requirements;
- legacy corpus results establish only the comparison behavior to preserve or
  intentionally improve;
- neither result certifies that the current checker understands all RC.3
  semantics.

The RC.3 audit does not retain an isolated exact transitive installation
closure for `csstype`, `seroval`, `seroval-plugins`, peers, and declaration
imports. That is an explicit Phase 7 proof obligation, not an implied success.

## Reproduction

Build the exact pinned producer and a fresh release checker, then copy both
binaries to stable paths so another build cannot change a long run. Substitute
those paths below.

```sh
make build-typefacts
make build-checker-release

bun scripts/audit-solid-rc3.mjs \
  --output-dir benchmarks/package-contract-v2/phase0/rc3 \
  --solid-repo /path/to/solid

bun benchmarks/measure-command.mjs \
  --json benchmarks/package-contract-v2/phase0/measurements/ecosystem-generation.json \
  -- env SOLID_CHECKER_GATE_CACHE=0 \
  SOLID_CHECKER_NATIVE_BIN=/stable/path/solid-checker-rust \
  SOLID_TYPEFACTS_BIN=/stable/path/solid-typefacts \
  bun scripts/ecosystem-benchmark/run.mjs --timeout 600

verification_state=$(mktemp -d /tmp/solid-checker-phase0-verify.XXXXXX)
bun benchmarks/measure-command.mjs \
  --json benchmarks/package-contract-v2/phase0/measurements/ecosystem-verification.json \
  -- env SOLID_CHECKER_GATE_CACHE=0 \
  SOLID_CHECKER_NATIVE_BIN=/stable/path/solid-checker-rust \
  SOLID_TYPEFACTS_BIN=/stable/path/solid-typefacts \
  bun scripts/ecosystem-benchmark/verify-corpus.mjs --state-dir "$verification_state"

bun benchmarks/measure-command.mjs \
  --json benchmarks/package-contract-v2/phase0/measurements/contract-corpus.json \
  -- env SOLID_CHECKER_GATE_CACHE=0 \
  SOLID_CHECKER_NATIVE_BIN=/stable/path/solid-checker-rust \
  SOLID_TYPEFACTS_BIN=/stable/path/solid-typefacts \
  bun scripts/contract-corpus.mjs

bun scripts/package-contract-v2-phase0.mjs
bun scripts/package-contract-v2-phase0.mjs --check
```

## Invariants

- Unknown refusal or generation classes abort baseline generation.
- Duplicate or missing verifier row IDs abort baseline generation.
- A missing cache-disable token or nonzero measured exit aborts baseline
  generation.
- Fixture tree hashes include ignored `node_modules` inputs.
- Input identity covers schema, pins, reports, measurements, bundled contracts,
  RC.3 audit, and every frozen fixture file.
- Inline evidence, compact main bytes, and expanded bytes are measured
  independently.
- The report states measurement limitations instead of inventing an isolated
  Rust query number where no benchmark seam exists.
