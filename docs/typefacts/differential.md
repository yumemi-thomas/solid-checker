# Type Facts differential harness

Acceptance tool that compares any two Type Facts producer binaries: a
**reference** and a **candidate**. It records what the checker asked the
reference, replays the same request stream on the candidate, decodes both
answers with the Rust client's own decoder and diffs the rows. It was built to
prove the Rust producer against the Go one (ADR 0365) and stays for comparing
Rust versions, such as a change of symbol ids.

- Reference: `--reference BIN`, else `SOLID_TYPEFACTS_REFERENCE_BIN`, else the
  archived Go producer, `$TSGO_ARCHIVE/bin/solid-typefacts-go` (`TSGO_ARCHIVE`
  defaults to `~/Documents/Github/sc-worktrees.noindex/tsgo-archive`). With none
  of these present the tool exits 2 and says so.
- Candidate: `--candidate BIN`, else `SOLID_TYPEFACTS_CANDIDATE_BIN`, else
  `bin/solid-typefacts` (`--rust` and `--go` are accepted aliases).
- Names inside transcripts and reports keep their history: the reference side is
  `go` (`responses.bin`, `go-evidence.json`), the candidate `rust`
  (`rust-responses.bin`, `rust-evidence.json`), so the archived transcripts and
  the allow-list stay valid.

To compare two Rust builds, point the reference at the older binary (build it
from the older commit; `make typefacts-differential` records through it). The
producer's differential tests fail on a reference named by
`SOLID_TYPEFACTS_REFERENCE_BIN` that is absent or whose handshake names another
protocol or schema, or was built on another tsrs rev (its `.buildinfo`; other TypeScript means other lib offsets and ids). The implicit archive default is skipped in that case (since
protocol 82, ADR 0368, the archived Go producer speaks 81) with a `SKIPPED` line
every run shows; `SOLID_TYPEFACTS_REQUIRE_REFERENCE=1` fails instead.

Pieces: `scripts/typefacts-differential.mjs` (CLI), `scripts/lib/typefacts-differential.mjs`
(normalise, diff, report, allow-list; unit-tested without a producer),
`rust/crates/typefacts/src/transcript.rs` and `src/bin/typefacts-transcript.rs`
(decode and replay; feature `transcript`, which the checker never enables),
`scripts/typefacts-transcript-proxy.mjs` (the recording proxy).

## Run it

```
make typefacts-differential                       # record `coverage` through the reference, replay on the candidate, report
make typefacts-differential GATE=ownership-gate   # or tsc-oracle-gate, obligation-audit
make typefacts-differential DIFFERENTIAL_ARGS="--allow-unavailable --canonicalize-requests"
make typefacts-e2e                                # `coverage` with the checker on the candidate
```

Exit 0 means every cell is equal or allow-listed. `--allow-unavailable` also
accepts the "Rust has not implemented this yet" cells, so a baseline can exit 0.
Transcripts, replays and `differential-report.json` live under
`rust/target/differential/` (ignored by git).

## Record

`record` runs a gate with `SOLID_TYPEFACTS_BIN` pointed at the proxy, which
forwards to the reference producer and writes one numbered directory per producer
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

`replay DIR` sends each session's recorded request frames to the candidate
(same argv, same working directory) and stores `rust-responses.bin` and
`replay.json`. Two things are rewritten, because each names something one
producer minted: `stateToken` (each reference token is mapped to the token the
candidate returned for the same exchange) and the ids in a `symbols` request. A
symbol id belongs to the producer that issued it: `symbol:<generation>:<n>`
follows its visit order, and the canonical id of a library symbol hashes offsets
in its TypeScript snapshot's lib text. Such an id is replaced by the id the
replaying producer gave the entity at the same source location in the preceding
analyze tables (`rewrittenSymbolIds` in `replay.json`), unless two locations
disagree about it, and an id no entity carries is sent as recorded. The tables
themselves are compared as they came. Cancel frames are not replayed.

Unless `--no-evidence` is given, each producer is also asked, just before the
recorded `close`, for the declarations of every canonical id its own tables
mention (`rust-evidence.json`, and `go-evidence.json` from a second replay
against the reference; the extra request is not part of any compared
stream). The recorded streams almost never carry declarations for the symbols
their entities name, and `compare` needs them to prove a canonical id
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
  entity pointing at it still compares. Canonical ids hash declaration offsets and
  the lib text differs between the producers' TypeScript snapshots, so `JSON`
  has a different `symbol:h:` id in each; the proof that an id is library-only is
  a symbol row for it with every declaration in lib, from the transcript's own
  symbol rows or from that producer's evidence file. An id with no such proof is
  left alone: a canonical id that differs without proof is a difference. A
  well-known symbol's counter suffix (`__@hasInstance@73`) is dropped from the
  name.
- Generation-scoped `symbol:<gen>:<n>` ids are renamed from content (first
  declaration, else first use), one mapping per transcript. Canonical
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

## Producer-level differentials (cargo tests, a reference binary beside the Rust one)

Under `apps/solid-typefacts/tests/`, each drives the reference (`SOLID_TYPEFACTS_REFERENCE_BIN`, default the archived Go producer; the test prints a NOTICE and passes vacuously without one) and the candidate through the
client `Session` and diffs decoded rows. `TSRS_DIFF_ALL=1` runs every
`fixtures/**/tsconfig.json` (default: a deterministic subset).

- `families_differential`: every family demanded at every word, dotted chain
  and literal span, cold, after a span-shifting update and after the restore.
  Entity rows must be equal after renaming generation-scoped ids and dropping the
  unique-symbol counter of well-known symbol names. `TSRS_DIFF_PROJECT=<tsconfig>`
  probes one project, `TSRS_DIFF_FILES=a,b` limits the demanded files.
- `delete_restore_differential`: a dependency deleted and restored, each family
  alone and all together (Go 3991f6919); the restored analysis must equal the
  cold one on each producer.
- `families_corpus`: no demanded family is counted unavailable on the fixtures
  and each family is answered somewhere (the precondition for the switch).

`TYPEFACTS_DIFF_CLIP=<n>` widens the report's cell clip (default 400 bytes).

## Cells no port can equal

Go itself differs between two runs on these (a Go-vs-Go replay of the same
session reports the same cells), so they are not port defects:

- the id of an ES private-name member (`#add`) in Go: the binder spells the
  symbol name with the class symbol's id (`GetSymbolNameForPrivateIdentifier`),
  a global counter handed out while files bind in parallel;
- the member order of a union of route-handler result types on `sensor-sim`
  (`Hono<...>` texts): two Go runs print different orders; the cause in Go is
  not isolated.

Rust differs from Go by design in these cells (ADR 0367, 0398). It runs
tsrs's canonical check history (`tsrs_core::compat`), so a normalized
object-literal union's properties can print in another order: text only, and
no decision reads type text. An entity's own descriptor is its location's
type, where Go gave every entity of a symbol the first one answered
(`families_differential` counts these rows). Wire names and runtime identities
leave the checker's counters out (`__@iterator`, the runtime identity of
`[Symbol.hasInstance]` at `instanceof`). Each file is checked first by a
checker of its own, which can change a fact to the source-order answer (a
`.value` read on an object literal gets reference space `Value`), and derived
symbols take exact derived keys where Go handed out counters.

## Replaying the archive

The archive (`$TSGO_ARCHIVE`, ADR 0365) holds transcripts of the Go producer's
answers for `coverage`, `ownership-gate`, `tsc-oracle-gate`, `obligation-audit`
and `corpus-sweep`. Replay one against the current producer:

```
A=$TSGO_ARCHIVE
bun scripts/typefacts-differential.mjs replay $A/transcripts/ownership-gate --candidate bin/solid-typefacts
bun scripts/typefacts-differential.mjs compare $A/transcripts/ownership-gate --allow-list scripts/typefacts-differential-allowlist.json
```

Replay writes into the transcript directory, so copy it first to keep the
archive pristine. The recorded `-project` paths must still exist (the gates
regenerate them under `rust/target/`).
