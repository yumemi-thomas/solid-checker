# Type Facts repatriation conformance

Date: 2026-08-27

## Verdict

The Type Facts producer/client source move is behavior-neutral and the active
build is fully local. The external repository remains only as provenance at
`92c53392388518d69ef27220729f5c061479deed`; it must not be archived until the
separate two-CI-runs-and-one-release-build retirement gate completes.

No fact vocabulary, protocol number, schema digest, lifecycle behavior, or
checker finding changed. No compiler code changed in this repatriation.

## Ownership and identity

- history import commit: `b969b483`;
- history-preserving relocation commit: `8d02fdc8`;
- imported external tree: `92c53392388518d69ef27220729f5c061479deed`;
- external development binary SHA-256:
  `2c518663dfa11e9cfabde62bb7db49b0318423f35d3db1984708fb4e7a59a29b`;
- request golden SHA-256:
  `bfda90e8cc17a0129dce70e990fa8272fead6bb0272681960afdd16cec12a578`;
- response golden SHA-256:
  `8494ad7a589611e139af1f89a488789ea58cede9a69e975ba9ae0a56a8c2ff62`.

The ignored producer binary now carries a JSON build stamp computed from the
local Go producer, Rust client, shims, wire schemas, root Go module graph,
TypeScript-Go pin, build scripts, actual Go toolchain identity, and build ID.
CI keys the producer cache on the same source digest, resolved Go version,
build ID, platform, architecture, and destination.

The final local development build stamp records source digest
`f00697a5a0692eebb468c1a7977d382b98df60b0df8b0c2d0102ed41dbb3c3f7`
with Go `1.26.5` on Darwin arm64 and build ID `dev`.

The runtime handshake remains exactly the frozen protocol/schema/build-ID
triple. Codec limits are validated against `schema/typefacts-codec-limits.json`
and bound by the local build identity; they are not falsely described as an
existing fourth handshake field. A separate wire-level codec digest is a later
protocol migration.

## Cross-pair compatibility

All four test-only producer/client combinations passed the 26 real process
tests and three codec golden tests. Production switched producer and client
atomically; the mixed pairs exist only as parity evidence.

| Rust client | Go producer | Result |
| --- | --- | --- |
| external `92c53392` | external `92c53392` | pass (frozen source audit) |
| external `92c53392` | local relocation | 26 process + 3 golden pass |
| local relocation | external `92c53392` | 26 process + 3 golden pass |
| local relocation | local relocation | 33 unit + 1 public API + 26 process + 3 golden pass |

The process coverage includes startup refusal, retained lifecycle, update,
cancellation, crash/restart/replay, stale generation/state, full/delta/reuse
transitions, shared-arena and inline transport, module graphs, resolved calls,
runtime domains, constant values, tuple/array shapes, and compiler semantic
facts.

## Transcript parity

`scripts/typefacts-transcript-proxy.mjs` recorded both producers behind the
same local client and same fixed project. The complete request byte streams
were identical:

`c38ff3e383d7147242c4ca0672749ca6be149fe03aaa8c9fa5e8bc7c371294a8`

The startup handshake frames were independently byte-identical:

`7fdd2be306915a2c54adc700cb4d5df71d4e7cbffc251c767cd50bf73f4e93af`

Raw materialized responses legitimately contain measured nanosecond durations
and an independently allocated temporary source-arena pathname. The
repository-owned normalizer zeroes only duration fields and canonicalizes only
that ephemeral pathname. It retains timing counters/flags, source lengths,
table transitions, evidence, identities, errors, and every semantic field.
The resulting complete framed streams were byte-identical:

`699f22438cff2a3bcbc12cbaa477ae7d072b106f861d9006251b8e8493d8dbf4`

## Checker and performance parity

With gate caching disabled, the same fresh checker reported exactly 557
findings across 94 fixture projects with the external producer and exactly the
same 557 findings with the local producer.

An interleaved five-round A/B run used the same release checker benchmark and
changed only the producer:

| Metric | External | Local | Ratio | Gate |
| --- | ---: | ---: | ---: | ---: |
| incremental | 35,216,125 ns/edit | 35,316,709 ns/edit | 1.003 | 1.10 |
| first Reactive IR | 69,257 ns/source | 68,904 ns/source | 0.995 | 1.10 |

The imported Go race suite, retained-storage tests, sparse materialization
tests, and Rust retained-session benchmark remain the memory proof for this
source-only move. No production algorithm changed; module/import-path strings
are the only compiled-code identity delta.

## Verification performed

- exact external `make test`: pass;
- local `go vet ./apps/solid-typefacts/...`: pass;
- local `go test -race ./apps/solid-typefacts/...`: pass;
- local Type Facts Rust suite and retained-session benchmark: pass;
- all four client/producer pairings: pass;
- exact request, handshake, and canonical response transcript comparisons:
  pass;
- old/local checker coverage comparison with cache disabled: 94 projects,
  557 findings on both sides;
- interleaved 1.10x performance gate: pass.

The final repository-wide `make verify` passed all 24 steps in 13.20 seconds,
including Go format/vet/race, Rust format/Clippy/workspace/process tests, both
dialect feature matrices, coverage, ownership, performance, CLI, TypeScript
oracle, obligation audit, schema validation, and contract conformance.

## Remaining operational gate

T13 is intentionally not satisfied by local testing. Before making
`yumemi-thomas/solid-ts-facts` read-only or archived, require two clean CI runs
and one release build from the monorepo. Until then, do not delete the ignored
`.typefacts` compatibility checkout or claim the external repository is
retired.

## Independent reviews

- [source and history audit](../subagent-reports/2026-08-27-typefacts-source-audit.md)
- [checker integration audit](../subagent-reports/2026-08-27-typefacts-checker-integration-audit.md)
- [parity and performance audit](../subagent-reports/2026-08-27-typefacts-parity-audit.md)
