# Phase 3 completion report: Type Facts invocation transcripts

Date: 2026-08-27

## Outcome

Phase 3 is complete. The repository-owned Type Facts producer and Rust client
now expose a demand-shaped `invocations` lifecycle operation for exact
call/construct expressions. The operation is read-only and proof-oriented: it
does not add rows to the retained editor table or alter its state token.

The public consumer seam is `typefacts::Session::invocations`, also forwarded by
`solid_facts_backend::TypeFactsSession::invocations`. TypeScript-Go AST, type,
symbol, signature, and flow objects remain private to
`apps/solid-typefacts/internal/typefacts/tsgo`.

## Delivered scope

| Plan items | Delivered proof surface |
| --- | --- |
| 28-30 | exact call/construct demands; selected declaration/overload identity; direct, optional/defaulted, rest, exact tuple-spread, unknown-spread, and omission facts |
| 31-32 | alternative-local property/tuple callable paths; literal, callability, Promise/AsyncIterable/plain, tuple-length, and common-discriminant finite partitions |
| 33-34 | complete symbol-identity parameter-use census; return, throw, branch, returned-closure capture, and locally open unsupported-flow census |
| 35 | project/generation, ordered demand, module graph, consulted source, schema, and producer-build identity envelope |
| 36-37 | TypeScript-Go implementation, deterministic-CBOR producer fields, Rust model, decoder, independent digest recomputation, and invariant validation |
| 38-39 | aliases, namespace reexports, overloads, generics, constructs, optional/rest/spread, union sibling absence, destructuring, structural AsyncIterable, census, invalid-range, stale-update, and published-`tsc` differential tests |
| 40-41 | handshake protocol 3, new schema digest, source-manifest rebuild, and live Go/Rust handshake |
| 42 | Rust process-seam test plus backend process integration fixture |

The packed retained table stays at schema 17. Lifecycle schema remains 1; the
mandatory schema hash and handshake protocol refuse mixed producer/client
pairs. The source-manifest stamp changes with every producer, client, shim, or
schema input and the local producer was rebuilt from that identity.

## Accuracy decisions

- A finite callability partition enumerates the actual constituent categories,
  including `untypedCallable`; `mixed` never invents a non-callable branch.
- An object union is called discriminated only when every alternative has one
  common literal property and those literal values are pairwise distinct.
- AsyncIterable recognition uses TypeScript's iteration-protocol resolver, so
  structural implementations, aliases, extensions, and unions do not depend on
  the nominal name `AsyncIterable`.
- Compiler-internal escaped property names are neither emitted nor interpreted
  as public callable paths.
- The Rust client recomputes the ordered demand digest and selected-signature
  digest from transmitted inputs. It rejects duplicate completion domains,
  contradictory omissions, out-of-range formals, malformed path segments,
  positive facts on absent paths, and a closed unsupported control-flow census.
- `.call`, `.apply`, and `.bind` are refused before signature closure. Their
  TypeScript-selected signature belongs to the Function wrapper method, not to
  the receiver invocation, so accepting it would certify the wrong target and
  argument mapping.

## Remaining fail-closed cases

These are explicit open facts, not inferred negatives:

- `.call`/`.apply`/`.bind` receiver remapping;
- signature-less `Function` invocations, recovery signatures, unresolved exact
  ranges, and composite union dispatch with no single selected signature;
- optional/rest/array or unequal-union spreads whose runtime length is not exact;
- censuses for declaration-only signatures or calls with no one exact current
  implementation;
- reachability through loops, `switch`, and `try` constructs; and
- paths beyond the caller-selected depth, open index signatures, unconstrained
  type parameters, `any`, `unknown`, checker recovery types, and cycles.

Reactive timing, tracking, ownership, scheduling, and runtime invocation remain
outside Type Facts. Phase 4 obtains compiler-controlled execution from semantic
lowering facts; runtime package behavior remains contract/probe evidence.

## Handoff checks

Focused checks exercised the Go producer packages, the TypeScript-Go
adversarial suite, the Rust `typefacts` library, a live cross-process transcript
before and after an update, and the backend integration fixture. The final
handoff uses the repository-wide `make verify`; its result is recorded in the
phase PR rather than predeclared here.

No Solid compiler source, lowering, output, diagnostic, runtime behavior, or
dependency was changed in this phase.
