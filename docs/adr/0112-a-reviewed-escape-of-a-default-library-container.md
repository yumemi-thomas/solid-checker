# ADR 0112: A reviewed escape of a default-library container

- Status: accepted and implemented (2026-09-18); written with the
  implementation
- Date: 2026-09-18
- Owners: Type Facts producer (`default_library_alias.go`,
  `immutable_callee_alias.go`) and the policy-2 alias premise (`type_facts.rs`)
- Relation: narrows one clause of ADR 0103's stability guard. Handshake
  protocol 59 → 60.

## Context

ADR 0103 closes `reads`, `creates` and `callbacks` for an export that **is** a
reviewed default-library member by identity. It has closed **zero rows of the
ecosystem corpus** since it landed, and the reason was recorded as unknown:
`default_library_alias_test.go` attributed it to "something downstream of the
producer" on the evidence that the producer states the fact for the published
shape.

That attribution was wrong, and the cause is one line of one bundle.
`@solid-primitives/utils@7.0.0-next.4`'s `dist/index.js` aliases four members
the reviewed table admits — `Object.entries`, `Object.keys`, `Object.values`,
`Object.is` — and also contains

```js
const defaultEquals = Object.is.bind(Object);
```

`immutableAliasLibrarySourceIsStable` requires every occurrence of the
*container* to be the expression of a property access, because a container
handed to an arbitrary callee may be mutated by it before the alias is taken —
`Object.defineProperty(Object, …)` is the shape it exists to refuse. The check
is deliberately whole-file, so that one bare `Object` argument withdraws the
fact from every export of the bundle. `entries` and `keys` are 37 and 22
consumer call sites in the pinned demand census, and their `reads` domain stayed
open for want of it.

The consuming half was never the problem:
`a_reviewed_default_library_alias_closes_by_identity` certifies a fixture export
that is `Object.entries` and asserts `reads` closes on the stated identity. Run
armed it takes 10.8s, so it is not the silent early return those tests take
without the certification pins. The arm fires whenever the fact is stated.

## Decision

**`Container.member.bind(Container)` is reported as a named escape instead of
refusing the file's aliases.** `DefaultLibraryAlias` gains `containerEscapes`:
the qualified member whose `bind` received the container, once per distinct
member, in source order of first occurrence.

The producer decides nothing about them. **A consumer must require every entry
to be a member its own reviewed table admits, and read the whole fact as not
stated otherwise.**

### Why the shape is safe to state

`Function.prototype.bind` neither mutates its `thisArg` nor invokes the target;
it returns an exotic bound function capturing both. So the container the alias
was read from is the library's own at the moment the alias is taken, which is
exactly what the guard exists to establish.

### Why it is not safe to *decide*

The bound function runs later with the container as its `this`, and whether that
rewrites the container depends on the bound target. `Object.is.bind(Object)`
cannot; `Object.defineProperty.bind(Object)` obviously can. Which members leave
their receiver alone is the same reviewed question as which members a call
domain may close on, and that table already lives with the consumer
(`REVIEWED_DEFAULT_LIBRARY_ALIASES`). Duplicating it in the producer would be
two answers to one question, and the producer's would be the one that silently
admits a claim the certifier never reviewed.

So the division is the one ADR 0103 already drew: the producer states identity
and the shape of what it saw; the certifier decides what may be concluded.

### The four premises that gate the shape

Each is a way the escape could fail to be the reviewed one, and each refuses
outright rather than being reported:

- the container is argument **0** — the `thisArg` slot. A later argument is a
  value the bound target *receives*, which is a different claim;
- the callee is a property access named `bind` resolving to a default-library
  member, so it is `Function.prototype.bind` and not a `bind` this file or a
  dependency installed;
- the object `bind` is read from is a property access whose object resolves to
  the **same** container symbol, so the bound target is a member of the thing
  being handed over rather than of anything else;
- that member resolves to a default-library member of that container, which is
  what makes the reported name meaningful to the consumer's table.

Every other escape of the container still withdraws the fact from the whole
file, unchanged.

## Alternatives considered

- **Decide non-mutation in the producer.** Rejected: it duplicates the
  certifier's reviewed table, and a divergence between the two would be a
  producer that admits what the certifier refused.
- **Make the guard textual — refuse only escapes that precede the alias.**
  Sound for a top-level escape after the alias, and useless here: the escape in
  the motivating bundle is on line 18 and the aliases are on lines 141 and 145.
  It also needs a hoisting analysis to be sound at all.
- **State a two-value `stability` enum rather than the member names.**
  Rejected: "reviewed-escape" would hand the consumer a verdict it cannot
  re-derive, which is the absence-as-evidence ADR 0043 refused. The member name
  is the smallest thing the consumer can check for itself.
- **Widen silently, without moving the protocol.** Rejected for the reason
  ADR 0043 gave when it separated `parameter-default` from `parameter`: a
  consumer that reviewed protocol 59 reviewed the premise list including "nor
  escaping as anything but a read or a call", and relaxing it without a number
  leaves that review stale.

## Consequences

- Handshake protocol 59 → 60; the schema digest moves with it. A protocol-59
  consumer decodes with `deny_unknown_fields` and would reject a transcript
  carrying the new field, which is the safe direction.
- `immutableAliasLibrarySourceIsStable` keeps its signature and its meaning for
  its other caller (the immutable *callee* alias premise), which still requires
  zero escapes. The escape-collecting walk is
  `immutableAliasLibrarySourceStability` beside it.
- Tests: `TestDefaultLibraryAliasNamesAReviewableContainerEscape` pins the
  positive and all five negatives on the producer side;
  `a_container_escape_the_table_has_not_reviewed_withdraws_the_alias` pins the
  consumer's review, decoding from the wire so the field's round trip is pinned
  too. `TestDefaultLibraryAliasIsSuppressedByAReceiverEscapeInTheSameFile`
  predicted this change and was updated rather than deleted: it now pins that a
  non-reviewed escape shape still withdraws the fact.
- **Measured effect: 59 of 59 predicted sites.** The Solid 2 coverage census,
  same corpus and same frozen demand, moves degenerate 186 -> 127 and
  determined-negative 526 -> 585. `@solid-primitives/utils` goes 137 -> 78
  degenerate, which is `entries` (37) and `keys` (22) and nothing else; nothing
  regressed and `operations` is unchanged at 440, because a `reads: []` closure
  is a determined negative rather than a stated operation. In-surface coverage
  over the 1,152 answerable sites goes 83.9% -> 89.0%.
- The fifteen `@solid-primitives/utils` probe recipes written on 2026-09-18
  closed nothing on their own and were kept because they turn a masked blocker
  into a stated one. They are why this landed as a measurement rather than as a
  further investigation: `entries` and `keys` already had a finished mandatory
  veto, so the domain closed the moment the premise arrived.
- Recorded in `docs/precision-backlog.md`'s 2026-09-18 entry beside the
  measurement that motivated it.
