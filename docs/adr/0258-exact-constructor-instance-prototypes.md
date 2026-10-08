# ADR 0258: Exact constructor-instance prototype recipes

- Status: accepted and implemented (2026-10-09).
- Base: c397ea69b, including the accepted ADR 0256 capture implementation.
- Owners: ValueShape::PrototypeInstance, wire/schema/validation/canonical
  encoding and certification refusal; prototype_instance.rs; the Solid 2 leaf
  wording; prototype-instance-contracts.mjs; the set and map specs.

An authored constructor return may state `prototype-instance`, a finite member
list and an explicit constructor population protocol (`opaque`, `values`, or
`entries`). Each exact named method/getter or `@@iterator` names its per-instance
TriggerCache and an invocation argument or an inaccessible shared key. This is
the audited getObserver-gated recipe, not a general arbitrary-class graph.
Signal creation is conditional on a cold cache; successful signal creation is
followed by cleanup and a signal read. A retained cache goes directly to cleanup
and the read. An earlier throw does not prove a later operation was reached.
Iterator work occurs on first resume.
No graph is inferred, generation proposes none, and certification refuses it.
Wire version 1 is extended additively; canonical value tag 27 is appended.

The initial consumer requires direct imported `new`, one exact const binding,
and binder-selected references (ADR 0250). It proves tracked JSX/direct compute
reads and direct synchronous first iteration/next. An absent observer exits
without a signal read. Unknown observer/resumption, wrappers, extracted
methods, computed members, writes, aliases, exports, arrays, shorthand,
passing, returning and JSX values remain obligations. An imported constructor
used to mutate/escape its prototype anywhere in the project invalidates the
instance premise. Namespace/default imports, dynamic loads, re-exports and
opaque module hazards conservatively withhold the prototype premise project-wide.
Local-access cache reuse is disabled while recipes are present or a cached
contribution observed one, including contract withdrawal, until a dedicated
dependency digest owns that cross-file premise.

The caller's execution role is retained (ADR 0254). An untracked contract
spelling never forces a tracked caller into an untracked role. These collection
methods are not unconditional accessors: getObserver() normally returns null
when tracking is off. No top-level has/get/size strict-read violation is invented.
If a future exact observer premise establishes a read in a warning context,
the ordinary ReactiveRead projection decides that context; unknown stays open.

The leaf finding deliberately states the disjunction: a cold cache attempts
signal construction, a retained cache attempts cleanup. Both are forbidden in
a present tracked-effect leaf. It does not claim cleanup was reached after an
earlier construction threw, and it does not infer cache priming.

ReactiveSet and ReactiveMap constructors close their own reads, creates and
exact instance return. No argument and null population are safe; a fresh native
literal array is accepted for `values`, and `entries` additionally requires
fresh entry arrays. Calls, spreads, members and project prototype/computed
accesses withhold that input proof. Dynamic iterables and yielded Map tuples
remain open and need result provenance. Caller iterable callback closure stays
open even where this call-site slice proves the input. Mutators and forEach
are unstated. union
retains its existing accessor return, but its tracked-created iteration and
broader constructor census remain open (B-results/C-results).

## Measured consequences

- All set and map probe pairs pass in Chrome on rc.13.
- Primitives ledger, browser: 93 of 111 report correctly (was 92):
  `ReactiveSet`'s leaf-owner case. No correct twin on any host has a
  violation.
- rc.13 corpus: primitive import declarations still raising SC9005 drop from
  21 to 19 of 43 (`ReactiveMap`, `ReactiveSet`). No violation is added or
  lost; uncertifiable +8.
- A proven leaf-owner violation at a prototype member call can carry a
  redundant member-dispatch obligation at the same site. It is noise, not
  unsoundness, and is recorded in the precision backlog.
- Landing fixed three clippy lints and one unit test, which expected
  normalization to refuse a nested prototype value that the wire decoder
  already refuses.
