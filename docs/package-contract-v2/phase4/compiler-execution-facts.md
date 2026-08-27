# Compiler execution-facts protocol 2

This document fixes the Phase 4 contract between Solid compiler semantic
producers and `solid-facts`. It is narrower than the future package-contract
semantic model: it covers only behavior created or selected by compiler
lowering.

## Identities

A source operation ID is deterministic from its source byte span and semantic
kind: `s:<start>:<end>:<kind>`. Two different kinds at the same span remain
different operations. IDs are local to one source/configuration/output identity
envelope.

A generated operation ID is assigned only after canonical sort as `g<index>`.
Its `sourceId` is a stable source-origin identity and need not name a source
operation: structural wrappers can be generated from an enclosing JSX node
rather than one censused expression. Source operations that depend on a
generated operation cite its exact `g<index>` ID, and normalization enforces
that foreign key.

An identity envelope contains exact compiler package, official upstream, and
semantic implementation revisions; source/output/source-map digests; mode; and
the full effective compiler configuration. The semantic implementation and
distribution commits are separate because no commit can contain its own hash.

## Execution facts

Disposition, trigger, schedule, tracking, cardinality, and owner are orthogonal
fields. `unknown` applies to one axis only. A consumer must not widen it to
another axis or erase known siblings.

Terminal contradictions are invalid. At minimum:

- discarded means no trigger, no schedule, no tracking, no owner, and never;
- reactive rerun means dependency-triggered, render-scheduled, tracked, and
  one-or-more;
- event-triggered means event-triggered, deferred, untracked, and zero-or-more;
- SSR evaluation and SSR render callbacks are render-scheduled, inherited, and
  exactly once; and
- a generated-operation reference must resolve within the same envelope.

Owner relation describes the owner under which an operation executes. It does
not imply an owner precondition and it is not the same as creating or returning
an owner. Generated owner-creating operations retain their own identities.

## Completeness

`sourceOperationsComplete` and `generatedOperationsComplete` close only their
respective operation sets. They do not turn an `unknown` axis into a negative
fact. An empty complete set proves no operations in that set; an empty open set
is unknown.

Solid 2 trace v3 closes source operations after reconciliation. Its reported
generated operations are exact positive facts, but the set remains open until
the producer has an independent census over every generated semantic wrapper.
Solid 1 trace v2 also closes source operations only. Neither adapter may turn
an absent generated-operation row into complete-negative knowledge.

## Deep normalization

Dialect adapters translate producer enums and legacy decisions into private
normalization inputs. `solid-facts::compiler` alone:

1. recomputes source, output, source-map, and canonical configuration identity;
2. expands legacy decisions;
3. validates spans, order, IDs, references, completeness, and contradictions;
4. creates the normalized `CompilerSemanticModel`; and
5. derives and validates all legacy region/role views.

Downstream crates consume the normalized model or query its compatibility
views. They must not decode trace versions, reconstruct execution axes, or
guess generated operations.

## Supported and open domains

Solid 2 DOM and SSR lowering are supported. Universal/dynamic generation is
refused. The separate directive transform emits exact server-function
reference/registration facts, but transport and runtime invocation remain
open. Compiler facts do not certify package callback behavior, async emissions,
transition scheduling, request lifetime, response commitment, cleanup, or
framework artifact selection.

## Acceptance invariants

- The adapter's consumer-owned trace-version literal equals the producer
  constant at compile time and the trace field at runtime.
- Solid 2 producer identity is complete and equals the pinned producer
  constants.
- Operation arrays are canonical, IDs unique, and spans valid UTF-8 byte
  ranges.
- Every source-generated reference resolves; no compatibility array can add or
  remove a semantic fact.
- Closure is trusted only for a revision with passing corpus reconciliation,
  adversarial probes, trace-on/off neutrality, and exact-upstream output
  parity.
- Cache keys include dialect, producer identity, protocol, source, path, and
  configuration.
