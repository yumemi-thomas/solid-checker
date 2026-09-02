# Type Facts invocation transcripts

Status: Phase 3 implementation contract, 2026-08-27.

This document specifies the compiler-independent TypeScript facts used to prove
package-call semantics. The producer is `apps/solid-typefacts`; the only public
consumer seam is the Rust `typefacts::Session`. TypeScript-Go nodes, symbols,
types, signatures, and flow objects never cross that seam.

Invocation transcripts are a demand-shaped, read-only lifecycle operation.
They are not retained entity-table rows. Ordinary editor analysis therefore
does not retain proof-only callable trees or censuses, and asking for a
transcript does not change the retained demand set or state token.

## Demand and subject

An invocation demand names the exact source range of one call or construct
expression and selects two independently optional proof families:

- `callableDepth` is the maximum number of fixed property/tuple edges explored
  for callable-path and value facts. Zero asks only about roots. The producer
  rejects values above the protocol limit; it never silently truncates them.
- `census` asks for parameter-use and control-flow censuses when the selected
  signature has one exact current implementation.

The complete call-expression range is the subject. A range that names no exact
call-like expression produces one unresolved transcript at the requested
location. It is never widened to the nearest contained or containing call.
Demand order is preserved in the response, including duplicate subjects.

## Selected signature identity

A valid, non-composite call has a `SelectedSignature` only when TypeScript-Go
provides one selected signature and its declaration can be remapped into the
current program generation. Its identity is a SHA-256 digest over:

- call versus construct kind;
- canonical target identity;
- exact current declaration location, selected overload ordinal, and complete
  same-kind declaration count;
- minimum argument count and rest presence;
- instantiated formal type descriptors in formal order; and
- instantiated return type descriptor.

The digest is an equality key, not a nominal runtime brand. The transcript also
carries the digest inputs needed for review. A recovery signature, unresolved
call, signature-less `Function` call, `.call`/`.apply`/`.bind` indirection, or
composite union dispatch has no selected-signature identity. Composite dispatch
retains its already-proved exhaustive target set but does not invent a common
signature. Function prototype indirection remains explicitly open until the
transcript models its receiver and argument remapping; the selected wrapper
method's signature is never substituted for the receiver's signature.

## Actual-to-formal binding

Each written argument has one `ArgumentBinding`:

- `direct` has exactly one expanded slot and one formal index;
- `exactTupleSpread` has one expanded slot per required fixed tuple element;
- `unknownLengthSpread` has no invented slots and carries the possible formal
  range beginning at the current expanded position;
- `unmapped` carries a reason and no exact binding.

An expanded slot records the written argument index, optional tuple slot,
expanded position, formal index, and whether the selected formal is the rest
parameter. A rest formal may therefore receive arbitrarily many exact slots.

Only tuples with a compiler-proved exact length are expanded. Optional tuples,
rest tuples, arrays, open unions, `any`, `unknown`, recovery types, and unequal
tuple unions are `unknownLengthSpread`. Once such a spread occurs, later written
arguments have position-dependent mappings and remain explicitly unmapped.

`omittedParameters` is emitted only when binding is complete. It includes every
unbound optional/defaulted formal, and never treats a possibly covered formal as
omitted. A required omitted formal can appear only on a recovery call, for which
binding is open.

## Callable paths and recursive value facts

A parameter and the instantiated return each carry a root value fact plus a
flat list of callable paths. Paths use typed segments (`property` or `tuple`),
never dotted strings. Each union constituent is a numbered alternative. Facts
are local to `(alternative, path)` and record:

- required, optional, absent, or unknown presence;
- callable, untyped-callable, non-callable, mixed, or unknown callability;
- constructability independently;
- declaration provenance when TypeScript exposes it; and
- local completeness and open reasons.

The producer first enumerates fixed paths from every closed alternative, then
emits `absent` for a path missing from a particular closed alternative. An open
index signature, unconstrained type parameter, `any`, `unknown`, checker error,
cycle, or demanded-depth boundary opens only that path/alternative. It cannot
erase known sibling paths or prove them absent.

Discriminated union alternatives carry their exact literal discriminants.
Consumers branch on those records; they do not infer alternatives from rendered
type text or array order.

## Finite value partitions

Finite partitions are independent axes attached to a value fact:

- `literal`: exact string, number, boolean, null, and undefined alternatives;
- `callability`: callable, untyped-callable, and non-callable alternatives;
- `protocol`: Promise, AsyncIterable, and plain alternatives;
- `tuple`: exact fixed tuple alternatives; and
- `discriminant`: exact object-union discriminants.

A partition is present only when TypeScript proves the listed alternatives
exhaustive. Open strings/numbers, `any`, `unknown`, open index signatures,
unresolved generics, recovery types, or a partition exceeding protocol bounds
omit that partition and add a local open reason. The absence of a partition is
unknown, never a complete-negative fact. Multiple partitions may describe the
same value without claiming their Cartesian product.

## Parameter-use census

When `census` is requested and one exact current implementation exists, every
semantic reference to each formal binding is emitted in source order. Binding
patterns produce one root per destructured binding with its fixed path.
References are classified as:

- direct call or proven-alias call;
- argument to an exactly known or unknown target;
- property access;
- return;
- storage;
- capture by a nested callable; or
- unknown escape.

Every row also records whether it occurs through a proven local alias and
whether it is captured. `unknownEscape` is a complete census classification,
not a dropped row; it prevents consumers from closing eventual-invocation or
escape claims. Alias proof is limited to immutable, identifier-to-identifier
local bindings with exact symbol equality. Destructuring, mutation, property
storage, and calls through unknown helpers never become proven aliases.

No implementation, composite dispatch, declaration-only signature, cancelled
walk, or checker recovery leaves the census absent with its domain open.

## Return, throw, and control-flow census

The control-flow census enumerates returns, throws, branch predicates, and
returned-closure captures without assigning Solid ownership, tracking, or
scheduling. Each site records `reachable`, `unreachable`, or `unknown`.
Unsupported constructs are named in `unsupported`; they open reachability only
for the affected census, not selected signature or binding.

Nested callable bodies are separate execution regions. They are not counted as
returns or throws of the selected implementation. A closure returned directly
from the implementation records which parameter bindings it captures.

## Identity envelope

Every response carries one `InvocationEnvelope` binding:

- project identity and generation;
- ordered demand digest;
- schema digest and producer build identity;
- ordered source/declaration digest set actually consulted;
- resolved module-graph digest; and
- response-level open reasons.

Digests use lowercase `sha256:` values. Demand and selected-signature digest
inputs are length-delimited UTF-8 strings and unsigned decimal integers in the
field order defined by the producer tests. The module-graph digest is over the
schema-defined deterministic-CBOR record. Map iteration and rendered JSON never
determine identity. The demand digest retains the exact path spelling sent by
the caller; source-envelope paths are normalized and sorted before hashing.

The module digest covers the complete configured module inventory and import
resolution records. If the backend cannot attest that graph, the operation
fails. An unresolved import remains part of the hashed graph and locally opens
closure; it is not omitted.

The request generation must equal the open session generation. Update races,
old replies, mismatched project IDs, schema mismatch, and producer-build
mismatch fail before facts are exposed. A transcript never survives an update
without a fresh request and fresh envelope.

Policy-2 certification calls `Session::certification_invocations` through the
backend's private-execution adapter. Its non-serializable identity additionally
binds the compile-time executable and source-manifest pins, exact launched PID,
session and restart epoch, project generation, snapshot root, demand-graph root,
and every proof-demand ID. Changing the launch arguments or transport mode drops
the certification pin.

## Completeness lattice

Every proof-bearing domain uses the same four states:

| State | Encoding |
| --- | --- |
| unknown | no facts and domain absent from `complete` |
| partial | facts present and domain absent from `complete` |
| complete-positive | facts present and domain listed in `complete` |
| complete-negative | no facts and domain listed in `complete` |

`complete` is local to one transcript or nested fact and names only sibling
domains defined at that exact level. Unknown domain names are rejected. A
parent completion bit never closes child paths, alternatives, partitions, uses,
returns, throws, or branches.

The transcript domains are `signature`, `bindings`, `omissions`, `parameters`,
`result`, `uses`, and `controlFlow`. Nested value domains are `paths`,
`partitions`, and `discriminants`. This vocabulary is closed for this protocol
revision.

## Validation invariants

Producer and Rust decoder both enforce:

1. transcript count equals demand count and locations match by index;
2. valid selected signatures name an exact declaration and unique identity;
3. signature formal indices are contiguous from zero and contain at most one
   final rest parameter;
4. direct bindings contain one slot; exact tuple spreads contain one or more
   ordered tuple slots; unknown spreads contain none;
5. complete binding slots have contiguous expanded positions and every slot
   maps to an existing formal;
6. omitted formals are unique, optional/defaulted, and unbound;
7. paths and alternatives are unique, bounded, and deterministically ordered;
8. `absent` paths carry no positive type, callable, or provenance fact;
9. a complete finite partition is nonempty, duplicate-free, and exhaustive by
   its axis-specific producer proof;
10. census rows name existing formal bindings and exact source locations;
11. complete-negative domains contain no positive rows;
12. every source digest consulted by a transcript appears exactly once in the
    envelope; and
13. every digest, schema, generation, project, and build identity agrees with
    the live handshake and request.

## Proof obligations before closure

The producer may close:

- `signature` only for a valid compiler-selected, non-recovery signature with
  a current exact declaration;
- `bindings` only after exact expansion of every written argument and exact
  mapping of every expanded position;
- `omissions` only when bindings are complete and all formals were censused;
- `paths` only for fixed properties/tuple slots after excluding open index
  signatures, cycles, unknown types, and depth truncation;
- a finite partition only after every type constituent belongs to exactly one
  emitted alternative;
- `uses` only after enumerating the whole current implementation body by symbol
  identity, including nested captures and unknown escapes; and
- `controlFlow` only after enumerating the whole implementation execution
  region and marking any unsupported reachability construct.

Neither source syntax, parameter names, rendered type text, package API names,
runtime probes, nor a successful call alone can discharge these obligations.

## Protocol transition

Phase 3 widens the lifecycle operation set with `invocations`, so the handshake
protocol moves from 2 to 3. The lifecycle schema number remains 1: existing
operation meanings and the retained table are unchanged, while the mandatory
schema digest changes atomically and refuses mixed peers. The packed retained
table remains schema 17 because invocation transcripts are not table rows.
Rebuilding the local producer changes the source-manifest digest and build ID in
the same commit.

Phase 19 subsequently moves the handshake from 3 to 4 while retaining lifecycle
schema 1. Protocol 4 adds `overloadCount`, includes it in selected-signature
identity v2, and marks unresolved instantiable type leaves locally open. This
lets policy 2 refuse cherry-picked overload closure without contaminating exact
sibling value paths.
