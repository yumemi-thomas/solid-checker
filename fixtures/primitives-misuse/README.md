# primitives-misuse

The Solid Primitives misuse ledger: one misuse twin and one correct twin per
case, for an export of a package pinned in
`scripts/ecosystem-benchmark/primitives-checkpoint-corpus.json`. Each case runs
on its hosts (`none`, `browser`, `node`).

```sh
make build-checker-release
SOLID_CHECKER_NATIVE_BIN="$PWD/rust/target/release/solid-checker-rust" \
SOLID_TYPEFACTS_BIN="$PWD/bin/solid-typefacts" \
bun scripts/primitives-checkpoint.mjs --misuse --json rust/target/primitives-checkpoint/misuse.json \
  [--package @solid-primitives/timer]... [--case <id>]... [--concurrency <n>]
```

The app-patterns ledgers (`fixtures/app-patterns-misuse/`) run only when Rust
analysis code changes; a spec-only batch cannot move them.
