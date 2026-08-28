# Phase 15 completion report — adversarial hardening

Date: 2026-08-28
Branch: `codex/phase15-adversarial-hardening`

## Outcome

Malformed contract inputs are bounded before they reach protocol-specific
models, and every seeded false-closure mutation is rejected. The work stays
inside the existing temporary-v2 and normalized semantic abstractions: there is
no public schema migration, generator cutover, probe redesign, receipt format
change, analyzer migration, or bundle regeneration.

The backend now owns one shared bounded JSON boundary for main contracts,
proposal/proof workflow documents, proof and runtime-probe sidecars, probe
plans/evaluations/transcripts, acceptance receipts, and accepted catalogs. Each
family supplies an explicit byte, container-depth, semantic-node, and string
limit. File-backed catalog, contract, and receipt inputs are checked by metadata
before allocation. Object keys are bounded as strings, and wide arrays cannot
bypass the node census.

Exact import coordinates bind the package specifier to the manifest package and
canonical requested entrypoint. Entry, closure, and catalog paths reject empty,
dot, parent, repeated-separator, backslash, absolute, UNC, and Windows-drive
spellings as applicable. A different framework cannot reuse matching artifact
bytes under a substituted package identity. Receipt digest spellings must be
canonical lowercase SHA-256 values, and any main-document byte reformatting
invalidates `wireDigest`.

## Normalized invariants under attack

The four local knowledge states are unchanged:

- `Unknown` means no usable premise;
- `Partial` means known positives without an exhaustive census;
- `CompletePositive` means a proved exhaustive non-empty set;
- `CompleteNegative` means a proved exhaustive empty set.

An empty open collection is never normalized into complete-negative knowledge.
Recursive tuple, object, array, choice, Promise, AsyncIterable, resource, and
returned-value leaves retain independent knowledge. Closing or corrupting one
summary cannot close a sibling, move closure to a different domain, or erase an
unrelated known positive.

Operation validation now constructs one causal graph from explicit edges and
`Trigger::Operation` references. A trigger cycle, edge cycle, or cycle using
both representations is rejected with a bounded iterative topological walk.
Resources form a separate lifetime-dependency graph. A resource may be the
explicit typed anchor for its own lifetime, but an indirect owner, request,
transition, async-source, or generic resource cycle is contradictory and is
rejected. All operation/resource foreign keys remain mandatory.

Trigger, schedule, tracking, ownership, cardinality, capabilities, and resource
lifetime remain independent axes. Contradictory capability combinations do not
normalize. Restricted guards must remain disjoint, and a partition marked
complete must cover its finite remainder; an unresolved selection only performs
the existing monotone join and cannot create guaranteed behavior or negative
proof.

## Adversarial coverage

| Plan item | Attack and enforcing coverage |
| --- | --- |
| 172 | Seeded closure relocation to a sibling summary and to the wrong domain is rejected. |
| 173 | Empty open domains and dangling summary, operation, and resource references are rejected. |
| 174 | Explicit-edge, operation-trigger, mixed causal, indirect resource-lifetime, and attempted summary cycles are rejected. |
| 175 | Contradictory accessor/call capabilities and existing ownership/resource contradictions remain invalid. |
| 176 | Overlapping guards and a falsely complete uncovered remainder are rejected; open remainder joins remain monotone. |
| 177 | Standalone resolver tests prove manifest key order controls `default`, built-in, and custom condition precedence independently of caller condition order. |
| 178 | Same bytes under a different closure path or accepted dependency edge retain different canonical identities. |
| 179 | Stale, cross-package, cross-artifact, orphan, content-mismatched, reformatted, noncanonical, and oversized sidecars/receipts fail closed. |
| 180 | Producer, Rust client, and proof-consumer tests require canonical completeness and reject duplicate/false completeness, stale generations, stale producer identity, and cross-artifact replay. |
| 181 | Compiler-facts tests reject stale source/output identity, dangling generated-operation references, contradictory axes, and compatibility projections that disagree with normalized operations; compiler reconciliation remains a mandatory closure-proof family. |
| 182 | Shared structural limits reject byte, depth, node, string/key bombs; package, entrypoint, closure, and catalog traversal is rejected cross-platform. |
| 183 | Exact specifier/package/entrypoint binding rejects mixed-framework artifact substitution. |
| 184 | A deterministic 512-case byte-mutation corpus fuzzes decode, normalize, encode, and semantic round-trip. Any surviving input must encode deterministically and normalize identically after re-decode. |
| 185 | One table-driven seeded mutation test requires every named false-closure mutation to produce a refusal. |

The seeded suite is intentionally an all-mutations gate: adding a seed without a
matching refusal makes the test fail. The fuzz test does not treat parse failure
as a semantic result; it either refuses at a bounded boundary or proves
round-trip equivalence for the accepted input.

## Type Facts and compiler facts

No Type Facts producer implementation, protocol model, schema, build identity,
or normalized consumer behavior changed. Focused tests span the existing
authority chain: the Go producer must emit each complete invocation domain once;
the Rust client rejects duplicate and false completeness plus stale generation
or producer identity; and the normalized proof consumer rejects incomplete or
cross-artifact Type Facts replay. The local ignored producer binary may be
rebuilt by the source-manifest stamp gate, but no checked artifact or fact
meaning changes.

No Solid compiler fork code, compiler execution-facts protocol, compiler pin,
identity document, or generated output changed. Phase 15 relies on and audits
the existing protocol-2 reconciliation tests and the mandatory compiler
reconciliation proof family.

## Verification

Focused checks completed before the repository-wide gate:

| Command | Result |
| --- | --- |
| `cargo +1.97 test --manifest-path rust/Cargo.toml -p solid-facts-backend --lib` | 84 passed |
| focused Type Facts session regression | 1 passed |
| `bun run --cwd packages/cli test` | 4 files, 40 tests passed; TypeScript check passed |
| targeted Clippy for backend, IR, facts, and Type Facts | passed with `-D warnings` |
| `make verify` | passed in 248.86 seconds; final post-fuzz-assertion run passed in 161.93 seconds |

The full gate passed Go formatting, vet, and race tests; workspace Clippy;
backend and WASM feature configurations; compiler identity and Type Facts stamp
checks; 61 facts, 84 backend, 189 IR, 35 Type Facts, and all dialect/process
tests; 94 fixture projects with 542 findings; the 161-case TypeScript oracle and
41 keystones; ownership with 289 cases and 465 ledger rows (none pending);
performance certification; CLI and WASM tests; seven obligations and eleven
closures; 24 receipt-issued bundle cases in both physical locations; all seven
package pins; and composed conformance. The local Type Facts producer was
rebuilt at the new source-manifest identity and passed its handshake and process
tests.

One intermediate rerun hit a transient `BrokenPipe` in the pre-existing
`analyze_restarts_the_producer_and_replays_updates_after_a_crash` process test.
The exact failing test passed immediately in isolation, and the complete final
`make verify` passed with that test armed; no source change was made for the
transient failure.

## Generated artifacts

No source-controlled generated artifact changed. In particular, the temporary
main schema version, bundled contracts, receipts, fixture contracts and
snapshots, dialect manifests, runtime locks, WASM declarations, compiler pin,
and Type Facts protocol/build identity are unchanged. Ignored build products
under `bin/` and `rust/target/` are verification outputs only.

## Exact remaining open or uncertifiable cases

- Proposals remain unaccepted until every selected closed claim passes every
  proof family and receipt issuance. A passing runtime probe remains a witness,
  never negative or closure proof.
- Wildcard or unbounded export maps, non-literal dynamic imports, unsupported
  class/namespace surfaces, unresolvable callable kind, and external export-all
  without independent accepted semantics remain generator refusals.
- Linked/local packages without exact registry integrity; missing, ambiguous,
  or byte-different runtime/declaration/export identity; closure hazards; and
  missing or stale receipts remain uncertifiable.
- Incomplete or stale Type Facts generations and unreconciled compiler sites
  keep only their exact domains open. Unrelated known siblings remain usable.
- Unresolved guard inputs and recursive value leaves remain locally open. The
  normalizer does not widen their uncertainty or infer negative behavior.
- The Phase 13 RC.3 open domains remain open: the server-functions client
  declaration's TypeScript-owned self-error; real-browser DOM, delegation, and
  hydration observations; request-context and transport integration; user
  serialization; dynamic payload/target/selection leaves; and unstable frames
  protocol details.
- The explicit resource limits can refuse an otherwise meaningful oversized
  contract family. Such refusal is bounded and fail-closed; it is never a
  semantic negative.

Phase 15 claims complete adversarial coverage of plan items 172–185, not
complete ecosystem package knowledge.

## Handoff

- Branch: `codex/phase15-adversarial-hardening`
- Implementation commit: `ad86652c` (`feat: harden package contracts against
  adversarial inputs`)
- Pull request: <https://github.com/yumemi-thomas/solid-checker/pull/57>

This handoff metadata is a documentation-only follow-up commit. The complete
final `make verify` result above was recorded against the implementation commit.
