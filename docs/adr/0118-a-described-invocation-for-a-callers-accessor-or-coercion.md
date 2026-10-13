# ADR 0118: A described invocation for a caller's accessor or coercion

- Status: accepted and implemented (2026-09-25); written with the implementation
- Date: 2026-09-25
- Owners: the semantic model and wire format (`contract_semantics.rs`,
  `validate.rs`, `canonical.rs`, `contract_document.rs`, the schema), the
  generator (`solid-facts` AST facts, `interproc.rs`, `inferred_contract.rs`),
  the policy-2 `callbacks` census and its positive facts (`type_facts.rs`),
  the synthesized veto (`synthesized_vetoes.rs`), and the consumer projection
  (`contracts.rs`, `interproc.rs`, `indexes.rs`, `source_discovery.rs`)
- Relation: widens ADR 0100's described `callbacks` enumeration by four
  protocols beside the direct call. ADR 0004 is not amended: the protocol is
  an additive field in its own digest family, as ADR 0114's `computations`
  was. Ways-to-improve § 3.3, item A.

## Context

The semantic model already counts a getter reached by property access, an
iteration-protocol member, a coercion and `Symbol.hasInstance` as invocations
of caller-supplied code, and the producer records every such form with its
subject parameter. ADR 0100 could only *refuse* them (rule 2): an enumeration
had no way to say "this export reads a property of your argument". So
`@solid-primitives/utils`' `access` (`typeof v === "function" && !v.length ?
v() : v`, 157 consumer sites) and `compare` (`a < b`, 3) kept `callbacks` open
although nothing about them is unknown. Nothing had to be discovered; a
positive item had to be expressible.

## Decision

**An `invoke` operation may state a protocol: `call` (the default, never
written), `get`, `iterate`, `coerce` or `has-instance`.** A non-call item says:
this export uses parameter *i* by that protocol **at the call event, on the
caller's stack, in the caller's tracking context; any trap of the caller's
value may run.**

### Shape

A non-call operation is exactly: kind `invoke`, `at` the call event,
schedule same-stack, tracking **`ambient-at-execution`**, count scope call,
min 0, max many, no guard, and referenced by exactly one `callbacks` item
whose `from` is the bare parameter (`path: []`). Validation refuses any
other shape; in particular it refuses `untracked`, which the consumer reads as
a proven tracking clear. A decoded `call` normalizes to absent, so there is
one spelling of one meaning.

### Identity

A contract with a non-call protocol writes
`solid-checker:semantic-invoke-protocol:v1` first and then each operation's
protocol; every other contract hashes byte for byte as before (pinned by a
golden vector per family). ADR 0117's recipe address uses the same technique
per claim, so the 95 migrated addresses do not move.

### The generator derives the items (ADR 0006)

- **`get i`**: a non-call, non-write member read in the export's own body
  (not inside a nested callable) whose object is directly parameter *i*'s
  unwritten binding. A member that is itself a callee (`v.x()`) derives
  nothing: that is a member invocation, a different item.
- **`coerce i`**: parameter *i*'s identifier as the operand of a coercing
  operator, mirroring the producer's list exactly, including that `==`/`!=`
  against the `null` keyword coerce nothing (a new `coercing_operands` AST
  fact; `coercive_operands` is unchanged).
- Nothing derives `iterate` or `has-instance` items yet, and an enumeration
  with non-call items is not proposed closed when any parameter is iterated
  (a new `iterated_operands` fact): the generator keeps the items and leaves
  the domain partial.

### The census confirms per protocol

ADR 0100's rules 5-8 apply per protocol. Every protocol site must be rooted at
the bare parameter (`subjectRoot: parameter`), at depth 0 and uncaptured, and
must be described; every described item must have a site. A form of a
protocol the enumeration names no item for refuses at rule 2 in ADR 0100's
original words, and accessor-write, element and own-result forms refuse
there as before. An enumeration with non-call items is refused whole when the
census ran under the declared-signature premise for a parameter whose
declared type admits an object, or read a helper frame at depth ≥ 1: under
that premise the producer records no form for a use of an engine-owned
container, so its silence would not be the absence of a trap.

### A primitive-typed item narrows instead of opening

When a non-call item's positive facts find no use because every declared
overload types that parameter primitive-only, the item is withdrawn with the
reason `narrowed out of a closed callbacks enumeration:` and the enumeration
stays proposed closed without it. The narrowed enumeration is re-planned and
re-confirmed by the census site for site, so nothing certifies that the census
did not confirm. A call item, an object-admitting item, or a mixed-reason
withdrawal opens the domain as before.

### The veto

A described enumeration with non-call items is vetoed through a recording
Proxy per described parameter: `apply` is a call; `get`, `has`, `ownKeys` and
`getOwnPropertyDescriptor` on string keys are `get`; `Symbol.iterator` is
`iterate`; `Symbol.toPrimitive`, `valueOf` and `toString` are `coerce`;
`Symbol.hasInstance` is `has-instance`. A protocol used outside the
description, or outside the call, emits the contradiction. Trap results are
the target's own, so `.length` reads 0 and `access` takes its call branch.
Candidates with no non-call item synthesize byte-identical modules.

### The consumer

A non-call item is not an inline invocation: it adds no call-graph edge, no
invoked parameter, no callback wrapper, and forwarding and re-export
inheritance carry the protocol unchanged. A closed `callbacks` domain with
non-call items is closed. What the item describes runs the caller's own traps,
on the caller's stack, in the caller's tracking context; before this ADR the
same use of a non-callable argument raised no obligation at all, so nothing
the consumer modelled is lost. The item is not the basis of any finding.

## Why the tracking word is `ambient-at-execution`

A property read or coercion inherits whatever tracking scope the caller is
in. `untracked` would claim the export clears it, and a future rule reading
`untracked` as a clear would report a false positive on every accessor
wrapper. This ADR never writes it for a non-call item, and validation refuses
it.

## Consequences

Measured on the 2026-09-25 coverage census (release binary, pinned corpus):
`access` and `compare` close `callbacks`; clean callables 261 -> 421,
`some-uses` 259 -> 99, `every-import` unchanged at 474; no export lost a
closure.

The first census run of an earlier draft found a gap the census missed:
`@kobalte/utils` `isPointInPolygon` destructures its tuple-typed parameter,
and under the declared-signature premise the producer records no iteration
form for an engine-owned container. The mandatory Proxy veto contradicted the
claim, correctly, and refused both `.` cases. The premise rule above is the
fix, and the fixture `implementation-census-described-accessor` pins that
export verbatim.

Still open: an empty `callbacks: []` enumeration (proposed, or reached by
narrowing) keeps the premise gap for object-typed parameters it had before
this ADR, and its veto uses no Proxy; the generator's iteration detection is
syntactic (a `||` fallback or an alias is not seen), with the census premise
rule as the backstop; member invocations of a parameter (item B) are still
refused.
