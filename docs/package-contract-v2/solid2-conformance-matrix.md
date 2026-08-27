# Solid 2 RC.3 conformance matrix

Every row requires exact published RC.3 declaration and runtime identity,
normalized expected semantics, proof obligations, a positive observation where
possible, an adversarial or negative case, a consumer fixture, and an explicit
refusal case for unbounded behavior.

## Authority rules

- Published RC.3 runtime artifacts and declarations win over documentation.
- Solid 1 and earlier Solid 2 beta assumptions are invalid inputs.
- `solid-js` and renderer-owned `@solidjs/*` entrypoints remain distinct.
- Native dialect facts may be richer, but must bind the same artifact identity.
- TypeScript-owned invalid calls do not create checker diagnostics.
- Server components remain experimental.

## Matrix

| Case | Required semantic representation | Required proof/evidence |
| --- | --- | --- |
| Split `createEffect` | Initial and repeated compute, queued apply, tracking split, data dependency, defer guard, error path, cleanup replacement/disposal | Exact signature/callback binding, RC.3 runtime operation proof, scheduling markers, cleanup probe |
| `createTrackedEffect` and `onSettled` | Trigger, leaf-owner capability, child prohibition, returned cleanup, lifetime, ambient/created owner relation | Runtime artifact proof, owner relation fixture, forbidden `onCleanup`/primitive fixture, cleanup observation |
| Batched writes and `flush()` | Queued writes, settled-value visibility, invalidation, flush barrier, apply drain, forbidden-scope conditions | Static operation edges plus controlled microtask/flush trace |
| `For`, `Repeat`, `Show`, `Match` | Guarded callback item/index/value/accessor shapes for every keyed mode | Type Facts finite guard partition, exact declarations, runtime callback arguments |
| Promise and AsyncIterable computations | Initial pending, settlement, repeated yields, rejection, cancellation/disposal, invalidation | Return-protocol facts, loading-context fixtures, repeated emission trace |
| Loading, pending, latest, refresh, affects | Initial loading versus revalidation, last-settled read, branded refresh, keyed affects, boundary dependency | Exact branded declaration identity, runtime state transitions, TypeScript oracle for invalid targets |
| Actions and optimistic state | Transition creation, writes, optimistic overlay, settle/error/revert, pending, refresh | Async-generator/action identity, transition state proof, optimistic lifecycle probes |
| Store drafts/projections/snapshots | Draft setter, replacement, readable/writable capabilities, shallow/deep tracking, reconciliation, snapshot immutability | Type/value shape facts, runtime read/write probes, capability contradiction tests |
| Two-phase refs/directives | Factory evaluation, factory owner, returned ref application, DOM timing, cleanup, array composition | Compiler lowering facts plus `@solidjs/web` runtime callback contract |
| Root-owned event delegation | Root registration, event callback, multiple roots, root disposal | Compiler event lowering, renderer runtime resource/lifetime proof, cross-root fixture |
| Browser render/hydrate and SSR | Exact DOM versus SSR artifacts, root ownership, callback execution, streaming lifecycle | Exact export resolution traces, artifact hashes, DOM/SSR compiler mode and probes |
| `httpStatus` and `httpHeader` | Request requirement, response uncommitted/committed states, declaration operations, cleanup | Request-scope runtime fixture and post-commit refusal/diagnostic fixture |
| Server-function references | Compiler transform identity, client/server reference shape, registration, transport, Promise result, serialization boundary | Compiler transformation facts composed with exact renderer runtime contract |
| Experimental server components | Exact experimental export/artifact identity, open unstable protocol domains | Explicit stability source, artifact hashes, no default-stable inference |
| Conditional adapters | Guarded callback selectors, result choices, shared operation subgraphs, unresolved guard joins | Selected signature/finite domain proof and branch-specific runtime observation |
| Mixed-framework packages | Exact Solid artifact and import/dependency closure; refusal for mixed artifacts | Resolved entrypoint/artifact identity, dependency closure, no manual framework label |

## Required fixture shape per row

Each case receives:

1. A contract-generation source fixture using real or byte-faithful declarations.
2. A positive proposal expectation.
3. A proof transcript expectation.
4. A probe event expectation where behavior is observable.
5. A normalized semantic expectation.
6. A consumer project that requires the fact.
7. A clean negative consumer project.
8. An unknown/escape/dynamic case producing an uncertifiable result.
9. A TypeScript oracle case proving the finding is not already TypeScript's.
10. An artifact mismatch or wrong-environment case that fails closed.

## Case-specific refusal policy

- Dynamic keyed/control options without a complete finite partition join all
  possible cases; they do not select a convenient branch.
- Nonliteral async/dynamic module loading opens the affected domain.
- Opaque transition or store helper calls require an accepted dependency
  contract.
- Server-function transforms that do not preserve exact export identity refuse
  the transformed export.
- Experimental server-component protocol details remain open unless directly
  established by published RC.3 artifacts and compiler output.
- A mixed-framework artifact is not classified as Solid merely because the
  package also contains a Solid entrypoint.

## Completion gate

The format cannot reach stable version 1 until all sixteen rows have positive,
negative, partial, and refusal coverage and their normalized semantic
expectations pass against RC.3 artifacts.

Phase 13 satisfies this private-model gate. The checked corpus is
`benchmarks/package-contract-v2/phase13/conformance.json`; its validator and
exact-artifact replay live in `scripts/package-contract-v2-phase13.mjs`, and
the normalized Rust cases live in
`solid-reactive-ir::contract_semantics::solid2_rc3`. Locally open browser,
request/transport, unstable frames, dynamic, and published-declaration leaves
are enumerated in the [completion report](phase13/2026-08-28-solid2-rc3-conformance.md).
They do not block unrelated known facts and are not negative proof. The stable
public version gate remains closed until Phase 14 performs the atomic producer,
consumer, receipt, bundle, and legacy-decoder migration.
