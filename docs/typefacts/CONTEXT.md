# Type Facts

Type Facts answers compiler-independent semantic questions about one configured
TypeScript project. This repository owns both halves of the exchange: the
process that answers, and the client library that asks.

## Language

### The exchange

**Producer**:
The process that answers Type Facts questions for one configured TypeScript
project.
_Avoid_: server, service, sidecar, backend

**Session**:
One retained analysis lifetime held against a single Producer.
_Avoid_: connection, client, handle

**Generation**:
A numbered version of a project's source state. Every accepted update advances
it by exactly one.
_Avoid_: version, revision, snapshot

**Affected set**:
The source paths a Producer reports as invalidated by an update.
_Avoid_: dirty files, changed files

**State token**:
A consumer's proof of which analysis result it last accepted. A Producer
refuses any token other than the one it issued most recently, so a consumer
may hold exactly one resumable position.
_Avoid_: cursor, etag, sequence number

**Table mode**:
Which of three forms an analysis answer takes — the whole table, a
transformation of the previously accepted one, or an assertion that the
previously accepted one still stands.
_Avoid_: response type, encoding, transfer mode

**Wire table transition**:
A packed frame that either establishes a Wire table or transforms the one
identified by its base Generation and State token. Reuse needs no frame.
_Avoid_: packed table, packed delta, payload

### Demands and closure

**Demand**:
A request for named facts at one source location.
_Avoid_: query, request, lookup

**Semantic demand run**:
One source file's canonically ordered demands for a generation.
_Avoid_: demand group, per-file batch

**Retained demand set**:
The Session-owned, path-ordered Semantic demand runs last accepted by the
Producer. An analysis edits this set transactionally and publishes the edit
only with its resulting Fact table.
_Avoid_: demand cache, request snapshot, demand map

**Analysis transaction**:
The unpublished candidate that owns one Retained demand set edit, its Demand
closure, Fact table, Transport manifest, Wire table transition, and successor
State token. It publishes all of them together or discards all of them.
_Avoid_: partial update, pending response, staged cache

**Demand closure**:
The facts transitively reachable from a demand set, taken to a fixed point.
The project's complete fact universe is never enumerated.
_Avoid_: analysis, expansion, crawl

**Symbol closure**:
The alias, declaration, and reference expansion over the symbols a demand set
reaches.
_Avoid_: symbol table, resolution, binding

**Full tier**:
The symbols whose reference lists a demand set requires. Symbols reached only
to be classified stay outside it.
_Avoid_: tier, level, priority

**Stable symbol closure**:
A retained Symbol closure whose raw reachable roots, raw Full-tier roots, and
alias edges are proven unchanged. Its prior alias fixed points remain exact, so
the Producer patches only invalidated symbol and reference rows.
_Avoid_: unchanged cache, assumed-stable closure, incremental symbol table

**Structural accessor**:
A symbol whose type descriptor is deliberately withheld from the fact table.
_Avoid_: suppressed symbol, filtered symbol

### Facts

**Fact table**:
One generation's answer to a demand set, as the Producer holds it.
_Avoid_: result, payload, response

**Wire table**:
The transport form of a fact table: source digests in place of source bytes,
and none of the Producer's process-local indexes.
_Avoid_: serialized table, DTO, frame

**Retained Wire table**:
An immutable client-side Wire table whose unchanged facts may share storage
with earlier Generations while those earlier tables remain readable.
_Avoid_: client snapshot, cached table

**Retained contribution**:
One file's share of a published table, held only until the packed transition
owns its rows. A table asks for every contribution (ADR 0364); what outlives
a table is the producer's per-file pass outputs, each with the read set it
was computed under (ADR 0398).
_Avoid_: cache entry, memo, fragment

**Canonical symbol identity**:
A symbol identity (`symbol:h:<24 hex>`) that is a pure function of the program:
a binder-level symbol's key (a checker singleton, a global by name, a module
export, else its minimum declaration), or a multi-origin property's combination
and origins when they fix its type (ADRs 0366, 0371). Whatever asked first, and
whatever the checker computed before, it is the same. Every other symbol
(an instantiation, a `this`-type or widened copy, a mapped property) has a
**derived identity**, the hash of its exact derived key (its source and type
context), the same in every checker, or no identity at all; a derived symbol
is not erased to its root (ADR 0372). No identity is a counter (ADR 0398).
_Avoid_: durable id, stable id, persistent id, global id

**Transport manifest**:
The record of exactly which rows of a fact table may differ from the
immediately preceding one.
_Avoid_: changeset, diff, dirty set

**Resolved module graph**:
The module inventory of one accepted program plus, on request, the provenance of
its module specifiers. It is a property of the program, not of a demand set.
_Avoid_: module map, import graph, dependency tree

**Module inventory**:
Every file a Producer's program actually resolved and included. Its authority is
that the process reporting it is the process that resolved them.
_Avoid_: file list, closure, module set

**Resolved call**:
The callable signature selected for one call or construction Demand. Its
validity is `valid` when selection is proven, `recovery` when the compiler
supplies a fallback after a call-resolution error, and `unresolved` when no
callable signature can be selected.
_Avoid_: invocation result, call graph edge
