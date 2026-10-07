# primitives-misuse

The Solid Primitives misuse ledger: one misuse twin and one correct twin per
case, for an export of a package pinned in
`scripts/ecosystem-benchmark/primitives-checkpoint-corpus.json`. Each case runs
on its hosts (`none`, `browser`, `node`).

## Running it

Against the release binary, the whole ledger takes about 2–3 minutes; a batch
that touched a few packages takes seconds with `--package`:

```sh
make build-checker-release
SOLID_CHECKER_NATIVE_BIN="$PWD/rust/target/release/solid-checker-rust" \
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" \
bun scripts/primitives-checkpoint.mjs --misuse --json rust/target/primitives-checkpoint/misuse.json \
  [--package @solid-primitives/timer]... [--case <id>]... [--concurrency <n>]
```

Cases run in parallel (half the cores, at most 8). An install is reused while
its manifest is unchanged, and each twin's `tsc` result is cached under a
digest of its source, compiler options, TypeScript release and lockfile.

## The other ledgers

Run these only when Rust analysis code changes; a spec-only batch cannot move
them. `CHROME` is a Chromium binary, `APPS_ROOT` the rc.13 corpus apps.

```sh
# app-patterns base ledger (33 cases)
MISUSE_CASES_ROOT=$APPS_ROOT/openbot/.solid-checker-app-patterns \
  node benchmarks/reviewed-package-models/development/misuse-runtime-ledger.mjs out.json "$CHROME" \
  --cases fixtures/app-patterns-misuse/cases.json --concurrency 3
# corpus twins (38 cases)
fixtures/app-patterns-misuse/run-corpus-twins.sh <out-dir>
# rc.13 corpus sweep (sweep.mjs is a local, untracked harness)
bun rust/target/defect-sweep/sweep.mjs <checker> out.json --jobs 4 --browser
```
