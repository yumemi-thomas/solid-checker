# Type Facts differential harness

Acceptance tool for the Rust producer (`bin/solid-typefacts-rs`) against the Go
one (`bin/solid-typefacts`). It records what the checker asked the Go producer,
replays the same request stream on the Rust producer, decodes both answers with
the Rust client's own decoder and diffs the rows. Fact families that the Rust
producer has not implemented show as their own category, so the same report
tracks progress until it reads all equal.

Pieces: `scripts/typefacts-differential.mjs` (CLI), `scripts/lib/typefacts-differential.mjs`
(normalise, diff, report, allow-list; unit-tested without a producer),
`rust/crates/typefacts/src/transcript.rs` and `src/bin/typefacts-transcript.rs`
(decode and replay; feature `transcript`, which the checker never enables),
`scripts/typefacts-transcript-proxy.mjs` (the recording proxy).

## Run it

```
make typefacts-differential                       # record `coverage` on Go, replay on Rust, report
make typefacts-differential GATE=ownership-gate   # or tsc-oracle-gate, obligation-audit
make typefacts-differential DIFFERENTIAL_ARGS="--allow-unavailable --canonicalize-requests"
make typefacts-e2e                                # `coverage` with the checker on the Rust producer
```

Exit 0 means every cell is equal or allow-listed. `--allow-unavailable` also
accepts the "Rust has not implemented this yet" cells, so a baseline can exit 0.
Transcripts, replays and `differential-report.json` live under
`rust/target/differential/` (ignored by git).

## Record

`record` runs a gate with `SOLID_TYPEFACTS_BIN` pointed at the proxy, which
forwards to the Go producer and writes one numbered directory per producer
process: `requests.bin`, `responses.bin` (handshake first), `argv.json`,
`meta.json`, and `project-config.bin` (the checker's temporary `-project-config`
file, deleted when the run ends). The proxy asks for `-inline-sources` so source
bytes are in the stream and drops `-transition-arena` (inline transitions). Gate
and daemon caches are switched off, since a cached unit never spawns a producer.

```
bun scripts/typefacts-differential.mjs record --gate coverage --out DIR
# gates: coverage, ownership-gate, tsc-oracle-gate (both need `make tsc-oracle-provision`
# and `bun install --cwd packages/cli`), obligation-audit, corpus-sweep
bun scripts/typefacts-differential.mjs record --gate corpus-sweep --out DIR -- \
  --out sweep.json --apps-root ../solid-checker-rc13-corpus/apps \
  --checker rust/target/release/solid-checker-rust --jobs 4
bun scripts/typefacts-differential.mjs record --out DIR -- <any command that runs the checker>
```

The sweep takes its producer as `--typefacts`, so `record` adds that flag. Use the
release checker for a corpus (the debug one is about 19x slower).

## Replay and compare

`replay DIR` sends each session's recorded request frames to the Rust producer
(same argv, same working directory) and stores `rust-responses.bin` and
`replay.json`. Two things are rewritten, because each names something one
producer minted: `stateToken` (each Go token is mapped to the token the Rust
producer returned for the same exchange) and the ids in a `symbols` request. A
symbol id belongs to the producer that issued it: `symbol:<generation>:<n>`
follows its visit order, and the durable id of a library symbol hashes offsets
in its TypeScript snapshot's lib text. Such an id is replaced by the id the
replaying producer gave the entity at the same source location in the preceding
analyze tables (`rewrittenSymbolIds` in `replay.json`), unless two locations
disagree about it, and an id no entity carries is sent as recorded. The tables
themselves are compared as they came. Cancel frames are not replayed.

Unless `--no-evidence` is given, each producer is also asked, just before the
recorded `close`, for the declarations of every durable id its own tables
mention (`rust-evidence.json`, and `go-evidence.json` from a second replay
against the Go producer; the extra request is not part of any compared
stream). The recorded streams almost never carry declarations for the symbols
their entities name, and `compare` needs them to prove a durable id
library-only (below). `compare DIR` decodes both streams, normalises, diffs and writes the
report; `run DIR` does both.

`--canonicalize-requests` re-encodes every request in canonical CBOR key order.
It is a diagnostic: the Rust producer rejects the Rust client's own `modules`
request (nested `moduleGraph` keys in struct order, not length-first), so
without the flag every `modules` exchange reports `rust-error`.

## What is compared

Cells are `(family, column, key)`: response fields (`response`, `affected`,
`sources`, `modules`, `moduleImports`, `symbolEvidence`, ...; a `symbols`
answer packs its rows as a standalone symbol table, which the decoder unpacks
into `symbolEvidence`) and the retained table after each analyze (`table.sources`, `table.files`, `table.entities` keyed
by span, `table.symbols` keyed by id). Row order is irrelevant; array order
inside a column is not.

Categories: `equal`, `allowed`, `value-differs`, `column-missing-in-rust`,
`column-extra-in-rust`, `row-missing-in-rust`, `row-extra-in-rust`,
`rust-unavailable` (the Rust producer's "not available ... migration slice"
error, expanded to every cell Go answered, so it measures what is left; a
state error that follows one counts too), `rust-error` (any other error or a
missing response), `go-error`, `decode-error`, `transport` (a session that did
not replay).

Normalisation, in `normalizeTranscript`:

- `bundled:///` becomes `bundled:/`.
- Default-library modules, symbols and declarations are dropped (which lib files
  exist and where their declarations sit follows each producer's TypeScript). A
  symbol id whose declarations are all in lib becomes `symbol:~lib:<name>`, so an
  entity pointing at it still compares. Durable ids hash declaration offsets and
  the lib text differs between the producers' TypeScript snapshots, so `JSON`
  has a different `symbol:h:` id in each; the proof that an id is library-only is
  a symbol row for it with every declaration in lib, from the transcript's own
  symbol rows or from that producer's evidence file. An id with no such proof is
  left alone: a durable id that differs without proof is a difference. A
  well-known symbol's counter suffix (`__@hasInstance@73`) is dropped from the
  name.
- Non-durable `symbol:<gen>:<n>` ids are renamed from content (first
  declaration, else first use), one mapping per transcript. Durable
  `symbol:h:...` ids compare exactly.
- `stateToken` keeps only its presence; timings and the source-arena path are
  dropped.

## Allow-list

`scripts/typefacts-differential-allowlist.json`: `{"version": 1, "entries": [...]}`.
Every entry is a snapshot decision (AGENTS.md invariant 6) and is reviewed like
one. Fields: `id` (unique), `family`, `column`, `category` (a failing one),
`justification` (at least 20 characters, mandatory), and optional narrowing
`project`/`key` (`*` globs) and `go`/`rust` (exact JSON values). Unknown fields
are refused. The report lists unused entries; delete them.

## End to end

`typefacts-e2e` runs `scripts/coverage.mjs` with `SOLID_TYPEFACTS_BIN` at the
Rust producer, compare-only. It sets `SOLID_COVERAGE_REPORT`, under which
coverage.mjs refuses `--update`, survives a checker that fails outright, and
writes every snapshot's state (`unchanged`, `moved`, `checker-failed`) with the
findings added and removed. The wrapper also checks `git status` of
`fixtures/findings-snapshots` before and after. Output:
`rust/target/differential/e2e/{coverage-moved,summary}.json`.
