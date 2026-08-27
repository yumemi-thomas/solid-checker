# Phase 4 completion report: compiler execution facts

Date: 2026-08-27

## Outcome

Phase 4 is complete. Solid 2 compiler-controlled DOM and SSR execution now
comes from reconciled semantic trace version 3, normalized into checker
compiler-facts protocol 2. The compiler fork remains semantic-only and no pull
request was opened against upstream Solid.

## Exact identities

| Role | Identity |
| --- | --- |
| Official upstream base | `solidjs/solid@a10cf1a147209d885f148396068175ab2f0a996a` |
| Fork branch | `yumemi-thomas/solid:solid-checker/compiler-facts-v3` |
| Semantic implementation | `e91bc2ae7fd0e9653db093b1ab74a09c9482042e` |
| Identity-only distribution commit | `a1a2b9a35d3e4ff7a193605aadec200c7e33dc53` |
| Solid 2 trace | version 3 |
| Checker compiler-facts | protocol 2 |
| Solid 1 producer | `yumemi-thomas/solid-1x-compiler@ca3bbfae7d1e00e28ef73f9af58bdb46e248b512`, trace 2 |

The checker pins the distribution commit. The trace reports the semantic
implementation commit; the distribution commit changes only that identity
constant. This provides an auditable non-self-referential identity chain.

## Delivered plan items

| Items | Delivered |
| --- | --- |
| 43-45 | deterministic source/generated IDs; terminal dispositions; independent trigger, schedule, tracking, cardinality, and owner axes |
| 46-47 | output-neutral recording at actual DOM and SSR lowering decisions; total reconciliation, rollback/discard handling, generated-operation foreign keys |
| 48 | exact source, effective configuration, mode, package/upstream/implementation, output, and optional source-map identity |
| 49 | DOM, SSR, refs, events, control flow, discarded paths, generated callbacks, corpus reconciliation, and adversarial regression tests |
| 50 | output-neutral server-function directive facts for exact reference and implementation registration identity |
| 51 | known divergences are tested as semantic observations; no compiler behavior fix or unrelated compiler change is carried |
| 52 | Solid 2 trace v3 and Solid 1 trace v2 normalize through one protocol-2 module; legacy generated facts remain explicitly partial |
| 53 | compiler cache keys include exact producer/protocol identity; Reactive IR reuse compares normalized operations, completeness, producer semantic revision, and compatibility views without treating per-build output digests as semantics |
| 54 | Cargo pin, lockfile, trace assertion, protocol, cache identity, notices, and this report move together |

The normative protocol is [compiler execution facts](compiler-execution-facts.md).
The general consumer documentation is [compiler facts](../../compiler-facts.md).

## Compiler scope proof

The compiler delta is limited to:

- semantic trace types and serialization;
- output-neutral recording/reconciliation at existing lowering decisions;
- source/output/configuration identity hashing;
- host-independent semantic result fields;
- server-function directive semantic facts;
- semantic tests and documentation; and
- `sha2`/`serde_json` dependencies required only by the fact interface.

It does not change lowering, generated JavaScript, source maps, diagnostics,
runtime behavior, compiler features, performance policy, or unrelated source.
Trace-on/off output and source maps are byte-identical over the compiler corpus.
The trace-disabled transform baseline remains identical to the exact official
upstream base.

## Accuracy decisions

- SSR inherited execution is not projected as legacy client-style untracked
  execution.
- Ref factory evaluation and generated ref application are distinct facts.
- Browser-only event/ref source discarded by SSR is not represented as a live
  callback.
- Generated wrapper facts are cited by exact IDs instead of inferred from a
  source disposition; their set remains partial until it has an independent
  emission census.
- Unknown is local to one execution axis.
- Solid 1's absent generated-operation trace is partial, not negative proof.
- Compatibility arrays cannot disagree with semantic operations.
- A stale output hash, source hash, source-map hash, configuration, trace
  version, or compiler identity is refused.

## Verification evidence

Completed while implementing the phase:

- Solid compiler default-feature library tests: 17 passed.
- Solid compiler no-default-feature suite: 8 unit, 6 census, 19 regression,
  and 7 host-interface tests passed; one intentional baseline writer ignored.
- Solid compiler all-target Clippy passes with only the exact inherited
  nonsemantic `dom/element.rs` `collapsible_if` lint allowed.
- `solid-facts`: 55 tests passed before the final added protocol regressions.
- `solid-reactive-ir`: 149 tests passed.
- Solid 2 adapter: 9 tests passed before final identity/SSR regressions.
- Solid 1 adapter: 6 tests passed before final partial-knowledge regression.
- Backend library: 29 tests passed.
- Diagnostics process: 15 tests passed.
- Contract process: 58 tests passed.
- Dialect process: 37 tests passed.

Final focused counts, coverage, ownership, and repository-wide `make verify`
are recorded by the phase PR checks. This report does not predeclare those
results.

## Remaining fail-closed cases

- Solid 2 universal and dynamic compiler modes have no semantic trace.
- Solid 1 generated-operation identity and full producer/configuration identity
  remain partial.
- Solid 2 generated operations are exact positive facts, but their operation
  set remains open because wrapper emissions do not yet have an independent
  completeness census.
- Server-function transformation facts are emitted but are not yet composed
  into package-contract proof receipts.
- Compiler facts do not prove runtime transport, async iteration, cleanup,
  transition semantics, request/response lifetime, or package artifact
  selection.
- Newly introduced upstream lowering paths must first gain explicit census and
  reconciliation before their operation domains may close.

These are open facts or later-phase consumers, not inferred negatives. No
subagents were used for this phase, so there is no separate subagent report.
