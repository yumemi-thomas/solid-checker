# Type Facts and compiler execution-facts plan

The replacement format should stay small by improving authoritative fact
domains instead of adding generator heuristics or public schema fields.

Before these improvements, complete the
[compiler and Type Facts bootstrap](compiler-and-typefacts-bootstrap.md): port
the existing trace to `solidjs/solid/packages/compiler` without changing
compiler behavior, and bring the Type Facts producer/client back into this
repository with protocol parity.

## Type Facts ownership

The co-located Type Facts module owns compiler-independent TypeScript semantics.
Its Go producer lives under `apps/solid-typefacts` and its Rust session client
under `rust/crates/typefacts`. It should add the following demand families
without exposing TypeScript-Go AST nodes, checker types, or symbols across its
interface.

### Resolved invocation

For one demanded call:

- durable target and declaration identity;
- selected overload/signature identity;
- instantiated return identity and protocols;
- actual argument to formal parameter mapping;
- omitted/defaulted parameters;
- rest mapping;
- exact tuple-spread expansion;
- explicit unknown-length spread;
- callability and constructability.

Tests cover aliases, namespace imports, reexports, overloads, generics,
optional parameters, tuple and array spreads, `.call`, `.apply`, `.bind`, and
unresolved calls.

### Callable paths

For object/tuple/union arguments and parameters:

- fixed property or tuple path;
- required, optional, or absent;
- callable, non-callable, or unknown;
- durable symbol/declaration provenance;
- exact discriminated alternative.

This supports options such as nested effect/error callbacks without asking the
generator to inspect type text.

### Finite domains

Return a complete finite partition only for compiler-proved alternatives:

- literal unions;
- boolean;
- null/undefined alternatives;
- callable versus non-callable;
- fixed tuple choices;
- discriminated object unions;
- Promise/AsyncIterable/plain protocols.

Unconstrained strings/numbers, `any`, open index signatures, and unresolved
generics remain open.

### Parameter-use census

Enumerate every semantic reference to a parameter or destructured binding and
classify it as direct call, proven alias call, argument to known/unknown target,
property access, return, storage, capture, or unknown escape. Type Facts proves
identity and census completeness; Reactive IR proves eventual invocation and
runtime context.

### Control-flow census

Extend current async/await facts with reachable returns, throws, branch
predicates, returned-closure captures, finite discriminant partitions, and
unsupported-flow markers. Solid tracking, ownership, and scheduling do not
belong here.

### Transcript identity

Every proof-bearing response binds project generation, source/declaration
digests, module graph, demand set, schema digest, producer build, and census
completeness.

## Type Facts delivery sequence

1. Specify model types and completeness semantics in the local Type Facts
   module.
2. Implement TypeScript-Go adapters.
3. Implement producer serialization and Rust client decoding together.
4. Add Go and Rust round-trip tests.
5. Add retained-session, update, cancellation, and stale-generation tests.
6. Add differential tests against published `tsc` behavior.
7. Bump the local protocol, schema digest, source-manifest identity, and checker
   consumer atomically.
8. Rebuild the producer and verify the handshake.
9. Add end-to-end contract-generation fixtures.

Additive demand families may retain the lifecycle frame protocol only if old
semantics remain unchanged and the schema digest prevents mismatch. Any change
to completeness or error meaning requires a protocol bump.

## Solid compiler execution-fact ownership

The compiler fact domain owns behavior established by actual JSX and server
function lowering, not runtime-library semantics.

Solid 2 facts are added to the compiler at
`solidjs/solid/packages/compiler` through an exact revision of the
`yumemi-thomas/solid` fork. The fork is semantic-only. It may add trace models,
output-neutral recorder calls at existing decisions, validation/serialization,
and fact-specific tests. It may not change lowering, generated output, source
maps, diagnostics, runtime behavior, features, performance, unrelated
dependencies, or unrelated compiler implementation. A behavior defect goes
upstream independently and leaves its fact open until the semantic branch
rebases.

### Protocol 2 operation census

Port the existing semantic trace version 2 unchanged first. Only after
trace-on/trace-off output identity and checker parity pass should the producer
move to semantic trace version 3 and the checker adapter move to compiler
execution-facts protocol 2. Those are distinct version namespaces.

Each compiler-controlled source site reports:

- stable source and generated operation identity;
- source and generated spans;
- terminal execution disposition;
- execution trigger and schedule;
- tracking relation;
- compiler-created owner relation and capabilities;
- cardinality where the emitted code proves it;
- DOM/SSR/server-component mode;
- emitted output digest.

Required dispositions include discarded, eager once, deferred, reactive rerun,
event-triggered, ref factory, ref application, component property getter,
control-flow render, and SSR evaluation.

### Emit facts from lowering

Facts must be recorded at the point of the actual lowering decision. A separate
predictive census may seed reconciliation but cannot be the final authority.
Every compiler-controlled source site has exactly one terminal disposition, and
every generated semantic callback maps back to source.

Recording a fact at a lowering decision means observing the existing branch. It
does not authorize changing that branch. If observation cannot be added without
altering output or behavior, the site remains explicitly unsupported.

### Owner relations

Compiler facts report only owners the generated code establishes:

- none;
- ambient at transform site;
- ambient at generated invocation;
- captured generated owner;
- created generated owner.

Child-owner, cleanup, and lifetime capabilities remain separate.

### Server-function transformation

Report recognized directives, source export identity, generated client
reference, generated server registration, serialization boundary, transform
mode, and output hash. Runtime transport behavior remains an `@solidjs/web`
package-contract fact.

### Reconciliation and parity

The semantic trace binds compiler revision, protocol, source hash, transform
configuration, mode, and output digest. Missing or contradictory sites fail the
file closed. Every known compiler-fork divergence receives a direct output
probe and regression fixture. The fork must additionally prove a zero generated-
output, source-map, and diagnostic diff against its exact upstream base.

## Main-repository integration

Update together:

- `apps/solid-typefacts` and `rust/crates/typefacts`;
- `rust/crates/solid-facts/src/compiler.rs`;
- both dialect compiler adapters;
- cache equality and invalidation;
- backend protocol validation;
- Type Facts demand planning;
- Reactive IR call binding and escape analysis;
- generator proof transcript creation;
- process and corpus fixtures.

## Exit criteria

- Variadic and overload behavior uses exact call binding.
- Nested callback selectors use callable-path facts.
- Closed callback absence uses a complete parameter-use census.
- Guard exhaustiveness uses finite-domain facts.
- Compiler-controlled execution comes from reconciled lowering facts.
- No generator name or source-text heuristic substitutes for a missing fact.
- Trace instrumentation does not change compiler output or behavior.
- No external Type Facts pull request or revision move is required to add a
  checker fact.
