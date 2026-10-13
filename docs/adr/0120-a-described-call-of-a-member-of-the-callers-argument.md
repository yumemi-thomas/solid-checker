# ADR 0120: A described call of a member of the caller's argument

- Status: accepted and implemented (2026-09-25); written with the implementation
- Date: 2026-09-25
- Owners: the Type Facts producer's parameter-value roots
  (`export_value_transcripts.go`, handshake protocol 63), the generator
  (`solid-facts` AST facts, `interproc.rs`, `creates_walk.rs`,
  `inferred_contract.rs`), the policy-2 `callbacks` census (`type_facts.rs`),
  the synthesized veto (`synthesized_vetoes.rs`), and the consumer projection
  (`contracts.rs`, `interproc.rs`)
- Relation: lifts ADR 0100's rule 4 for exactly one described shape and keeps
  ADR 0101's reading of a dotted member invocation as a `parameter-member`
  read. Builds on ADR 0118 (the member read is a `get` of the caller's value).
  No new wire field: a `callbacks` item's `from` has always admitted a path.
  Ways-to-improve § 3.3, item B.

## Context

`@kobalte/utils`' `callHandler` (112 consumer sites) is

```js
function callHandler(event, handler) {
	if (handler) if (typeof handler === "function") handler(event);
	else handler[0](handler[1], event);
	return event?.defaultPrevented;
}
```

`handler[0](…)` calls the caller's code, at the call, on the caller's stack.
ADR 0100 rule 4 refused every call whose callee is a member of a parameter,
and the producer did not root an element access at the parameter at all, so
the census saw an unresolved callee. That one refusal kept `callbacks` open
and stopped the `creates` walk from proposing (`unresolved-callee
computed-member`). Nothing about the call was unknown; the model had no way to
say "calls the value at `[0]` of your argument".

## Decision

**A `callbacks` call item may name a member of a parameter:
`from: {arg: i, path: [key]}` is a call of the value at that path of the
caller's argument, never a call of the argument itself.** Its shape is an
ordinary call item: `at` the call event, same stack,
`ambient-at-execution`, count call `0..many`, unguarded.

### The producer roots a literal element access (protocol 63)

`parameterValueSourceLocked` roots `p["str"]` at `p`'s property `str` and
`p[<n>]`, for a canonical non-negative integer literal `n`, at `p`'s tuple
index `n`, optional chains included. A computed non-literal key, a negative,
fractional or exponent key roots nothing, as before. A model path segment
names a tuple segment by its index's decimal spelling, which is the property
key the runtime gives it.

### The generator derives the item

For a call (not `new`), in the export's own body, whose callee is a
literal-keyed member of a parameter's own unwritten binding (path length 1),
the generator writes the member-path call item and the `get` of that parameter
that reading the member performs (ADR 0118). Its arguments are the called
member's business, exactly as a bare parameter callee's are, so the call no
longer raises the unresolved-callee obligation for them. A dotted member callee
(`props.onClick()`) derives nothing new: that is still ADR 0101's
`parameter-member` read.

### The census confirms site for site

ADR 0100's rule 4 is lifted for exactly the described `(parameter, path)`: such
a site is confirmed under rules 5 to 8 (depth 0, uncaptured, every such site
described, every described member called), and only on a parameter the
producer lists in `unwrittenParameters`. Every other member call refuses at
rule 4 as before, in its own words. A member path on a non-call item refuses:
ADR 0118's census confirms a protocol use of the bare parameter only.

A literal-keyed element-access callee is also now a parameter-rooted call in
the `creates` census and a member-invocation site in the `reads` census, so a
described `reads` enumeration beside `h[0]()` refuses at ADR 0101 rule 3. No
corpus fixture moved.

### The veto

The synthesized veto observes member calls at tuple indices only. A candidate
with a string-keyed member item is confirmed by the census but gets no
synthesized recipe, so it is withheld as `no recipe in corpus` and its domain
stays open. Candidates without member items synthesize byte-identical modules.

### The consumer

A consumer folds a member-path row as an inline call of the member's value
only when the argument written at the slot is an array literal or an exact
object literal naming that member (`callHandler(e, [readCount, data])` calls
`readCount`). Any other argument, including an identifier, a member expression
or a spread, folds nothing and raises nothing, and never folds the whole
argument, which would claim the caller's array is itself called. That loses
nothing the consumer modelled before: the export's `callbacks` were open, and
a consumer read no row of it. A member row whose argument is the owner's own
parameter is restated as the owner's row with its path unchanged.

## Why this is sound

The item claims only what the census saw: a call at a literal path of the
caller's value, performed before the export returns. The unwritten-binding
condition is what makes the member the caller's: the producer's
`calleeParameter` ignores writes, so without it `h = other; h[0]()` would be
confirmed as a call of the caller's member. The consumer's literal-only fold
means no finding rests on what an opaque argument holds at that key.

## Consequences

- Handshake protocol 62 -> 63: a rebuilt `bin/solid-typefacts` and new
  certification pins. No existing document's digest moves, and no existing
  generator-corpus file or findings snapshot moved.
- Measured on the 2026-09-25 coverage census (release binary, pinned corpus):
  `callHandler` closes `callbacks` and `creates` in both `dist` cases and
  `composeEventHandlers` closes `creates`; `totals.consumer` every-import
  454 -> 294, some-uses 119 -> 279 (all 160 on `@kobalte/utils`), clean
  unchanged at 421, nothing lost.

Still open:

- `callHandler`'s `returns` (`event?.defaultPrevented`) needs a
  property-of-argument return container (round 2 of the campaign);
- `composeEventHandlers` (48 sites) needs a deferred-invocation item and a
  returned-function `returns` shape; its member call is `callHandler`'s,
  reached from a returned closure, and refuses at depth 1;
- string-keyed member items certify only with a hand recipe;
- the census treats any call with a `calleeParameter` as parameter-rooted even
  when that parameter is written (`cb = other; cb()`); only member items check
  `unwrittenParameters`.
