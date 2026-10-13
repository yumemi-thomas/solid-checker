# ADR 0286: A typed member root requires an unreplaced slot

- Status: accepted and implemented (2026-10-13).
- Owner: `LiteralMemberSlots` (`literal_member_slots.rs`), consulted by the
  type-only fallback in `source_discovery.rs`.
- Found as a false SC1001 in project code, with no dependency involved.

## Context

```tsx
function relabel<T extends { label: () => string }>(target: T): T {
  target.label = () => "fixed";
  return target;
}
export function Tagged() {
  const [label] = createSignal("t");
  const tag = { label };
  relabel(tag);
  const v = tag.label(); // SC1001: "tag.label" is read directly in Tagged
  return <p>{v}</p>;
}
```

`tsc --noEmit` is clean against published rc.13. At runtime `tag.label` is the
plain `() => "fixed"`, so the read is not a reactive read, and SC1001 was
false. The same was reported for the longhand `{ label: label }`, for a
factory that hands its literal to `relabel` before returning it, for a consumer
that hands off a factory's object, for an alias written through, for a write
through the receiver, and for a method of the literal handing `this` off.

### The code path

The read did not come from symbol aliasing. Type Facts aliases only import
symbols. The finding's evidence named `Accessor<T>` in the stub as the
declaration. It came from source discovery's **type-only fallback**
(`discover_sources`, after the per-file stage). That fallback admits every
Type Facts entity whose type descriptor carries the dialect's `Accessor`
alias, a member included, provided the member's property resolves apart from
its receiver. The entity at `tag.label` resolves to the object literal's own
property symbol. Its inferred type is the getter's `SourceAccessor<string>`.
So the fallback put that property symbol in `accessors`, with the alias as its
declaration. `local_access` then resolved the callee `tag.label` to the same
symbol (`callee_symbol`) and filed a direct read: no origin, so the evidence
reads "the cross-file reference resolves to that accessor declaration".

Nothing asked whether the slot still held the accessor. The two existing
refusals did not cover this. `binding_has_write` sees only an assignment
naming the symbol itself, and `relabel`'s write names the generic
constraint's `label`, a different symbol. ADR 0273's written-name veto belongs
to the transport resolver and is never consulted here. The property symbol's
Type Facts reference census is demand-driven and empty: no consumer asks for
it.

## Decision

Before the fallback admits a member root, every read of that property symbol
is judged by its receiver. The reads are every Type Facts entity that resolves
to the symbol. Each consumer of the root finds its reads by entity identity,
so these are exactly the reads the root can produce. A receiver is:

- **Closed**: a `const` binding (identifier pattern, unwritten, not exported,
  never a shorthand value) whose every reference is the receiver of a static
  member that is not written. It is initialized by an object literal, or by a
  plain call of one exact synchronous project function. Every completion of
  that function returns a fresh object literal, written in the return itself
  or through a `const` the function declares in its own body and uses only
  for member reads and those returns. The literal binds no `this`: its own
  properties are all data, none a `function` expression. It spreads nothing
  in.
- **Escaped**: the same project-literal provenance, but some use on the way
  is anything else: an argument, an alias, a property value, a JSX value, a
  return to unknown code, a write through it. Or the literal has a
  `this`-binding member.
- **Unknown**: anything else: a parameter, a context value, an import, a call
  of unknown code.

What is refused depends on where the property is declared:

- **A property an object literal declares** (`{ label }`, `{ label: label }`).
  The type is only what that literal stored. The root is kept when every read
  is Closed over a literal that declares the property as its own static data
  property. Any other read refuses it, and so does any site that is not a
  static member Get (a destructuring key, a computed access).
- **A property declared anywhere else** (an interface, a type literal, a
  class). The type-only premise stays for Unknown receivers. The root is
  refused only when some read is Escaped, which needs a provably project
  literal object that left the analysis's sight. This covers
  `const tag: Labelled = { label }; relabel(tag); tag.label()`.

A refused root is **silent**, not uncertifiable. That is how the fallback
already treats its other refusals: a written binding, a receiver that is the
member itself, a contracted container. The fallback is the only evidence that
the read is reactive at all, so withholding it leaves nothing to obligate.
ADR 0273's veto likewise "only withholds".

A use that cannot replace the member is judged exactly. Passing `tag.label` itself
(`inspect(tag.label)`) and reading another member (`tag.other`) are member
reads of a Closed receiver and keep the violation.

## Consequences

- The verdict is per property symbol, because `accessors` is keyed by symbol.
  One Escaped read withholds every read of that symbol. Two consumers of one
  factory share the factory literal's property, so a hand-off by either
  silences both. That over-withholds; it never adds a claim.
- A hand-off to project code that provably writes nothing (a reading JSX
  component, a helper that only reads) is still a hand-off. Following the
  object into the callee is not done.
- An interface-declared member read through an Unknown receiver keeps the
  type-only premise. A parameter typed `{ label: Accessor<string> }` whose
  caller replaced `label` is still reported. That is the existing premise, not
  this rule.
- No cache change: the fallback runs on every analysis, after the per-file
  discovery cache.

## Evidence

- **Fixture** `reactive-ir/literal-member-handoff`: 5 violations (`Kept`,
  `MemberOnly`, `FactoryKept`, `ArrowKept`, and `External` for the unchanged
  interface premise) and 8 silent refusals (`Tagged`, `Longhand`,
  `FactoryHanded`, `Consumer`, `Alias`, `Written`, `Method`, `Declared`).
  Before the change all 13 were SC1001. `tsc --noEmit` is clean against the
  stub and against installed rc.13 (`skipLibCheck: false`).
- **Coverage:** no other snapshot moved (1,877 findings before, 1,882 with the
  new fixture).
- **rc.13 corpus:** release binary, browser host. Unchanged at 562
  violations and 3,731 uncertifiable, so the violation diff against the
  baseline is empty. The corpus does not use this shape; the rule only
  removes a false claim.
