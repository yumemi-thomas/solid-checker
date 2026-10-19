# coverage

Runs the checker over every fixture project and records what it found, so
"no finding moved" is a checkable claim.

```sh
make coverage           # compare against the snapshots
make coverage-update    # rewrite them
```

## What a snapshot holds

One file per fixture project, `<group>__<project>.json`: the project's `status`
and every finding (`rule`, `code`, `kind`, `severity`, repository-relative
`path`, byte `start`/`end`, `fixes`) sorted by path, byte offset, code and rule.

Messages and hints are excluded (rewording must not churn snapshots), except
for projects in the `KEEPS_WORDING` set in `scripts/coverage.mjs`, where the
wording is the behaviour under test. Offsets are bytes, not line and column, so
inserting a line does not read as every finding moving. `status` is included:
a flipped verdict is a change.

## When a snapshot changes

The runner prints the first differing line per project. Either the change was
intended (re-record with `make coverage-update` and review the diff) or the
diff says which rule moved where. A snapshot diff is evidence, not a verdict.

## Scope

Every directory under `fixtures/reactive-ir/` and `fixtures/engine/` holding a
`tsconfig.json`; the run's summary line is the authoritative count. A snapshot
whose project was deleted fails as an orphan. It does not cover the corpus,
compiler execution facts, performance, or the LSP and WASM entry points.
