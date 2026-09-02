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

The runner defaults to `min(available CPUs, 8)` install/generation workers.
Historically expensive rows start first, while each package proposal retains a
separately bounded adaptive fan-out across its exact source programs. On hosts
with more than eight CPUs, already-complete rows certify in the otherwise-idle
outer slots without reducing generation below eight. Once proposal work
drains, certification can expand to `min(available CPUs + 6, 20)` outer
workers — wider than the core count, because a certification child mostly
waits on filesystem metadata rather than a core once registry bytes are cached
— with each child's ordinary artifact-analysis width reduced to preserve a
host-wide bound. Certification
reuses the exact install already verified for generation; native code still
replays every archive, lockfile, graph root, source closure, proposal, and
policy input before issuing a receipt. `--concurrency N` and
`--certification-concurrency N` remain available for controlled comparisons or
memory-constrained hosts.

Certification children share a content-addressed registry cache,
`rust/target/registry-cache` by default (`--registry-cache DIR`,
`SOLID_CHECKER_REGISTRY_CACHE`, or `--no-registry-cache` to fetch every byte
fresh). Each certification acquires the packument and archive of its root and
of every compiler-source dependency the lockfile selects from the registry, and
nearly every probe names `solid-js`; before the cache, a wide-surface root
spent minutes in sequential registry round trips and the corpus wall time
tracked registry latency rather than the checker. An entry is addressed by the
exact (origin, package, version, integrity), is written only after the archive
hashed to that integrity, and is used only when it still does and its packument
still names that exact record — the checks a fresh acquisition passes — with
Rust re-deriving the snapshot from the bytes as before. The report records the
cache location under `checker.registryCache` (null when disabled) so a wall
time can be read knowing whether it includes registry latency.

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

Artifact cases the generator recorded as *inapplicable* are counted separately,
as `artifactCasesInapplicable`, and never added to `artifactCasesRefused`. An
entrypoint no consumer reaches as a module — an unpublished target behind a
custom export condition, or a sourcemap/JSON/CSS entrypoint — asserts nothing
about certifiable behavior, so counting it as a refusal would make a row look
unproven where nothing was ever provable. See the sidecar's `inapplicable`
array and `artifactCaseDisposition` in
`packages/cli/scripts/generate-package-contract.mjs`.

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
regression there would go unnoticed. The PR workflow runs one matrix shard per
pinned family with `--timeout 120`: 120 seconds keeps every probe on its
expected classification, while the family boundary limits each process tree
to 1–6 related probes. Motion and Solid Recharts serialize their two measured
heavy probes with `--concurrency 1`; the other six families use four workers.
A final aggregate job retains the stable `sentinel` check name and passes only
when every pinned family passed. Each shard uploads its own
`report-sentinel-family-<family>.{json,md}` artifact.
`make ecosystem-sentinel` deliberately keeps the 300-second default for local
reproduction. Lower the timeout elsewhere only after confirming every probe,
not just the expected timeout, keeps the same classification.

The CLI also writes a progress-only heartbeat to stderr every 30 seconds while
probes are active because child output is deliberately buffered into the final
report. Heartbeats never enter result rows, report files, semantic digests, or
threshold evaluation; they only bound how long an operator or CI runner sees
no output.

Do not hand-invent a probe id — always copy it from a manifest row's `probes`
array, since the id encodes the exact package version, Solid target, and
probe kind the manifest actually selected.
