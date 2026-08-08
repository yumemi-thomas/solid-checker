# coverage

Runs the checker over every fixture project and records what it found.

```sh
make coverage           # compare against the snapshots
make coverage-update    # rewrite them
```

The point is to make "no finding moved" a checkable claim. Before this, the
only assertions about rule behaviour were hand-maintained counts in
`rust/crates/solid-facts-backend/tests/rule_quality_process.rs` — good at catching a
rule that stops firing on a file someone remembered to list, useless for
catching a finding that moved somewhere nobody listed. ADR 0002 verified a
refactor as zero-diff across 56 fixture runs, but the method was never
committed, so the next refactor had to invent it again.

## What a snapshot holds

One file per fixture project, named `<group>__<project>.json`, containing the
project's status and every finding sorted by path, byte offset, code and rule:

```json
{
  "status": "uncertifiable",
  "findings": [
    {
      "rule": "reactive-read-after-await",
      "code": "SC1002",
      "kind": "violation",
      "severity": "error",
      "path": "fixtures/reactive-ir/tracer/App.tsx",
      "start": 412,
      "end": 431,
      "fixes": 0
    }
  ]
}
```

Deliberately excluded: messages and hints. Rewording a hint should not churn 30
snapshot files, and the wording is not what a rule change is about. Also
deliberate: **byte offsets rather than line and column**, so inserting a line
into a fixture does not read as every finding below it moving; and
**repository-relative paths**, so snapshots do not carry anyone's home
directory.

Four projects are excepted and keep their text (the `KEEPS_WORDING` set in
`scripts/coverage.mjs`): `dialect-solid-1x`, `dialect-solid-2`,
`import-location`, and `solid-1x-leftovers`. For those, the wording *is* the
behaviour under test — a dialect-specific diagnostic quoting the wrong
signature is exactly the failure they exist to catch.

Included on purpose: `status`. A change that keeps every finding but flips the
project's verdict is still a change.

## When a snapshot changes

The runner prints the first differing line per project. Either the change was
intended — re-record with `make coverage-update` and let the diff be part of
review — or it was not, and the diff says which rule moved where.

A snapshot diff is evidence, not a verdict. `--update` is cheap and the diff is
the artifact worth reading.

## The dialect pair

`dialect-solid-1x` and `dialect-solid-2` hold byte-identical sources and differ
in one file: `node_modules/solid-js/package.json`, which says `1.9.14` in one
and `2.0.0-beta.31` in the other. That is the whole mechanism — the checker
reads the version the project would actually import and picks its dialect, so
the fixtures exercise the real detection path rather than a test-only override.

The diff between their two snapshots is the only automated evidence that the
1.x adapter does anything:

- `createEffect(fn)` is a complete effect in 1.x and a missing-apply violation
  in 2.0 (SC7001), because the callback moves from argument 0 to argument 1.
- `createEffect(undefined)` is SC7001 in both, quoting a different signature.
- The async `createMemo` is a different defect per dialect: SC5004 in 1.x
  (async tracked scope, no sync option exists), SC7002 in 2.0, whose hint
  names `<Loading>`.
- `createEffect`'s second argument splits the dialects: a dormant seed in 1.x
  (silent), the apply callback in 2.0 — where its untracked read is SC1001 and
  its unproven return is SC9002.
- The v1-only rules (SC8002 imports, SC8009 proxy APIs, SC3001/SC4002 leaf
  cleanup) fire on the 1.x half alone, because 2.0 has no such catalog
  entries.

Two rules the runner enforces for this pair, both of which fail loudly:

- Their snapshots keep `message` and `hint`, because for these the wording *is*
  what is under test. A dialect diagnostic that quotes the other dialect's
  signature is the failure they exist to catch.
- Their shared sources must stay byte-identical. Duplicated source drifts, and
  a drifted pair makes the snapshot diff meaningless rather than wrong — the
  worse of the two failures, so the runner checks it directly.

Adding a dialect-gated rule means adding its case to both halves.

## Scope, and what this does not cover

Every directory under `fixtures/reactive-ir/` and `fixtures/engine/` holding
a `tsconfig.json` — 45 projects, 426 findings as of this writing; the run's
own summary line is the authoritative count. Fixture groups outside those
two (`fixtures/package-contracts/`, the parity corpus) have their own
harnesses and are not snapshotted here. A snapshot whose project was deleted
fails the gate as an orphan rather than silently dropping its findings.

It does not cover: the corpus (`make corpus`, Solid Primitives, a separate and
much larger body), compiler execution facts (`make conformance`), or anything
about performance. It runs the release-shaped path through the CLI, so it also
does not exercise the LSP or WASM entry points.
