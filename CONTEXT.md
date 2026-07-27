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

### Demands and closure

**Demand**:
A request for named facts at one source location.
_Avoid_: query, request, lookup

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

**Retained contribution**:
One file's share of a demand closure, reusable for as long as the file stays
outside every accepted update's affected set and its demand list is unchanged.
_Avoid_: cache entry, memo, fragment

**Durable symbol identity**:
A symbol identity derived from its declaration, and therefore still meaningful
in generations after the one that minted it. The alternative is an identity
scoped to a single generation, which no retained state may outlive.
_Avoid_: stable id, persistent id, global id

**Transport manifest**:
The record of exactly which rows of a fact table may differ from the
immediately preceding one.
_Avoid_: changeset, diff, dirty set
