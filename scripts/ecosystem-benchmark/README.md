# ecosystem-benchmark operator guide

See [../../docs/ecosystem-benchmark.md](../../docs/ecosystem-benchmark.md) for
what this measures, the failure-class vocabulary, and the CI split. This file
is the short day-to-day operator reference.

## The two commands

```sh
make ecosystem-discover    # network: refresh manifest.json from the registry
make ecosystem-sentinel    # no registry metadata: run the pinned sentinel subset
make ecosystem-benchmark   # no registry metadata: run every row's every probe
```

`ecosystem-discover` is the only one of these that touches the network on its
own account; `ecosystem-sentinel` and `ecosystem-benchmark` read
`manifest.json` and only reach the network to `bun install` each probe's exact
pinned versions.

## Required environment

`run.mjs` (invoked by both `ecosystem-sentinel` and `ecosystem-benchmark`)
requires both of these to point at real, existing binaries, and exits 2
immediately with an explicit message if either is missing:

- `SOLID_CHECKER_NATIVE_BIN`
- `SOLID_TYPEFACTS_BIN`

The sentinel target uses `$(CURDIR)/rust/target/debug/solid-checker-rust`: its
job is semantic regression coverage, including a deliberately retained timeout
case. The full-corpus target uses a fresh
`$(CURDIR)/rust/target/release/solid-checker-rust`, because it is also the
product-speed measurement and an unoptimized binary can exaggerate generation
time by more than an order of magnitude. Both avoid the checked-in
`bin/solid-checker-rust`, which can lag the current source tree. Build the
appropriate target first if invoking `run.mjs` directly:

```sh
make build-checker-debug
make build-checker-release
```

The runner defaults to `min(available CPUs, 8)` workers. `--concurrency N`
remains available for controlled comparisons or memory-constrained hosts.

Each JSON result records `installDurationMs` and `generationDurationMs`
separately. The Markdown report aggregates those as worker timings, with the
remaining project creation, verification, and bookkeeping shown as harness
time.

## Contract content

Every probe that emits a contract also gets a `contractContent` block in its
JSON result, and the report gains a `combined.contractContent` aggregate plus a
"Contract content" Markdown section. This is the only part of the harness that
opens the emitted `solid-reactivity.json` (and its sibling `.review.json`); it
counts unknown claims per domain, refused entrypoints, closure notes, and
positive behavioral rows. It reads them inside `runProbe`, before cleanup
removes the output directory — moving that read later would silently measure
nothing.

It is additive: no outcome class, success rate, baseline comparison, or
floor/head diff depends on it, and a result with no `contractContent` still
builds and renders a report. See `lib/contract-content.mjs` for the counting
rules (per export *name*, variants folded into their export, an absent domain
is a positive claim) and
[../../docs/ecosystem-benchmark.md](../../docs/ecosystem-benchmark.md) for what
the numbers mean and do not mean.

## Tests

```sh
make ecosystem-benchmark-test    # Vitest over every *.test.mjs here
```

Hermetic — no network, no registry, no checker binary.

`SOLID_TYPEFACTS_BIN` should point at the checked-in `bin/solid-typefacts`
unless you have deliberately rebuilt it against a different pinned revision;
rebuilding it needlessly risks a producer/client handshake mismatch (see
AGENTS.md's "Contracts and dependency pins").

## Refreshing the manifest

```sh
bun discover.mjs --print-diff
```

first, to review exactly what would change — additions, removals, and any
`changed` entry, especially an `integrity` change, which discovery always
reports rather than merging silently. Only once that diff looks correct,
write it:

```sh
make ecosystem-discover
```

`discover.mjs --check` (used by the CI `full-corpus` job) does the same
comparison without writing, and exits 1 on any drift — use it to confirm the
checked-in manifest still matches what discovery would produce, without
touching the file.

Discovery validates the manifest before writing it and fails on any problem
`lib/manifest.mjs#validateManifest` reports — an unknown family, a missing
required-family row, a bad probe count, an out-of-order row, and so on — so a
successful `ecosystem-discover` run is already a validated manifest, not
merely a written one.

## Adding a probe to the sentinel set

`sentinel.json` is a pinned `{ "schemaVersion": 1, "probes": [...] }` list of
probe ids (`<package>@<version>|<solidTarget>|<kind>`) drawn from the current
`manifest.json`. To add one:

1. Find the row and probe you want in `manifest.json` (or run
   `discover.mjs --sentinel <file>` after a fresh discovery, which writes a
   candidate pinned subset alongside the full manifest).
2. Copy its exact `probes[].id` string into `sentinel.json`'s `probes` array.
3. Keep the invariants the sentinel set exists to preserve: it must include a
   representative probe from every family listed in
   `docs/ecosystem-benchmark.md`, cover both `solid1` and `solid2`, and
   include at least one probe for every failure class seen in the last full
   run plus at least one known success — a sentinel set that has quietly
   drifted to all-success would stop catching a regression in classification
   itself. If the last full run produced any `partial-success` probe (an
   exit-0 generation that refused entrypoints), keep one of those too: it is
   the only probe shape that distinguishes a complete contract from a partial
   one, and both exit 0.
4. Run `make ecosystem-sentinel` locally to confirm the new probe installs
   and classifies as expected before committing the change.

Note on runtime: the pinned set keeps a `timeout`-class probe, which by
definition burns the full `--timeout` budget (300s by default) every run. That
one probe dominates the sentinel's wall clock -- 23 probes take a little over
five minutes, almost all of it waiting for that one. It is kept deliberately:
dropping it would leave the `timeout` path unexercised, and a classification
regression there would go unnoticed. The PR workflow passes `--timeout 120`:
the pinned probe still reaches the same `timeout during generate` path, while
the workflow finishes below the external runner-shutdown window observed with
the 300-second operator default. `make ecosystem-sentinel` deliberately keeps
that default for local reproduction. Lower the timeout elsewhere only after
confirming the probe still times out in the same phase.

Do not hand-invent a probe id — always copy it from a manifest row's `probes`
array, since the id encodes the exact package version, Solid target, and
probe kind the manifest actually selected.
