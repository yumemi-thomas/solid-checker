# A destructured name is bound to a slot, not to the value it destructures

`@solid-primitives/spring@0.1.2` `createDerivedSpring` is

```js
const [springValue, setSpringValue] = createSpring(target(), options);
createEffect(() => setSpringValue(target()));
return springValue;
```

Its declared return is `Accessor<WidenSpringTarget<T>>`, and `solid-js` 1.9.14
declares `type Accessor<T> = () => T`. The generator nonetheless published a
return output byte-identical to `createSpring`'s — `{"kind": "tuple", "items":
[{"kind": "reactive", "role": "accessor"}, "unknown"]}` — because
`leaf_with_depth`'s binding-initializer fallback
(`rust/crates/solid-reactive-ir/src/interproc.rs`) followed `springValue` to
the binding's *initializer* and derived that call's shape, discarding the
destructuring slot entirely. A tuple index `[0]` on the declared `Accessor<T>`
does not exist, `tsc` would reject the access, and certification refused
recursive-value-shape demand
`sha256:b7e8980d06a1a3988e863d51da1d6504dc0f51181cf9e3b5130688fd9c7e0f66`
correctly: the census was right and the claim was wrong.

Three paths dropped the slot, and this fixture pins all three plus every form
in which the slot may not be guessed.

## The cases

**Claims that must be made, exactly.** These four are what separate carrying a
value's shape correctly from abandoning the derivation:

- `literalIndex` — `createPair(initial)[0]`. An element access with a literal
  index names the item, and `projection` reads it from the tuple. Answers the
  accessor.
- `wrappedLiteralIndex` — the same access under `as Accessor<T>`. A transparent
  TypeScript wrapper preserves the runtime value, so the item is still the
  answer. This is the second slot-dropping path: the widened call lookup
  matched `createPair(initial)` by its start byte and answered with the whole
  tuple. Answers the accessor.
- `wholeBinding` — `const pair = createPair(initial); return pair`. An
  identifier binding *is* bound to the whole initializer, so the tuple claim
  survives.
- `splitVarDeclaration` — `var pair; var pair = createPair(initial)`. One
  symbol, two `BindingFact`s, only the second carrying the value. The binding
  scan has to keep looking for the matching name *with* an initializer;
  committing to the first name match and then testing for an initializer lost
  the tuple claim silently.
- `staticProperty` — `createRecord(initial).value`. A static property name
  really does resolve against the object claim. This is the control that makes
  every property negative below mean something: without it, a silent
  `computedProperty` could be silence for want of any base shape rather than
  the refusal of a text match.

**Claims that must be withheld: the slot is real, its shape is not derivable
from the initializer.**

- `arraySlot` — `createDerivedSpring`'s exact shape. Was the whole tuple; now
  no return operation at all.
- `arraySlotSetter` — the mirror. The binding-initializer fallback used to ask
  for the pattern's *first* name, so a reference to `setValue` inherited
  `value`'s discovered accessor identity and the setter was published as an
  accessor. It now resolves the exact name the reference names. Visible in this
  fixture twice: `arraySlotSetter` publishes nothing, and `createPair`'s own
  tuple moved from `[accessor, accessor]` to `[accessor, unknown]`.

**Claims that must be withheld: no exact slot exists to name.** Each of these
must stay silent rather than answer with the shape it was read from.

- `computedIndex` — a non-literal index (`[at]`) names no particular item.
- `wrappedComputedIndex` — the same under a wrapper. The peel *commits* to the
  projection's answer: falling through on absence handed the widened call
  lookup the original span, whose start byte still matches
  `createPair(initial)`, and published the whole tuple for an index that named
  nothing.
- `computedProperty` — `createRecord(initial)[value]`. A computed member's
  property span is an *expression*, not a name, and its source text is not the
  property it reads. The parameter is named `value` on purpose, so it spells
  one of the object's own property names: a text match answered the `value`
  accessor for a read that may just as well be `setValue`.
- `wrappedQuotedMember` — `createRecord(initial)["value"] as Accessor<T>`. The
  quoted spelling resolves no property, and under a wrapper it must not fall
  through to the whole object either.
- `wrappedUnknownProperty` — `createRecord(initial).other as number`, a
  property the object claim does not carry. Same fall-through.
- `atZero` — `.at(0)` is a member call, not an element access.
- `defaultedSlot` — `[value = fallback]` binds the item *or* the default.
- `nestedSlot` — `[[value]]` binds a value *inside* item 0. `array_slots`
  reports a nested element's *first identifier*, which is precisely why the
  slot may not be inferred from that table.
- `restSlot` — `[, ...rest]` binds a slice of the tuple, not an item.
- `objectSlot` — `const { value } = createRecord(...)` binds a property of the
  result. Before the fix this published an object whose *every* property
  carried the whole inner tuple.

Each of the four fixes was measured load-bearing by reverting it alone against
this fixture: without the peel's commitment `wrappedComputedIndex` answers the
tuple and `wrappedQuotedMember`/`wrappedUnknownProperty` the whole object;
without the computed-member guard `computedProperty` answers the `value`
accessor; without the binding scan `splitVarDeclaration` goes silent; without
the slot gate six exports answer with the initializer's shape.

## Why `createRecord` destructures `createSignal` directly

`createPair`'s result is a tuple of items no consumer can name (that is the
point of this fixture), so an object built out of *its* slots has no shape at
all and every property case would be silent for the wrong reason —
`projection`'s object arm would never have a base. `createRecord` therefore
destructures the dialect primitive, which makes `value` a discovered accessor
and gives the helper a real one-property object claim. `staticProperty` pins
that the arm is live.

## What this does not prove

The exact positive claim for `arraySlot` is available in principle — item 0 of
`createPair`'s tuple *is* the accessor — and recovering it needs one exact
fact the AST tables do not carry: each array pattern element's own span, so a
consumer can tell `[value]` from `[value = fallback]`, `[[value]]` and
`[{ value }]`, all three of which report their first identifier in
`array_slots`. Until that fact exists the honest answer for a destructured
name is silence, which is what this fixture pins. Recorded in
docs/precision-backlog.md.

## Stub faithfulness

`node_modules/solid-js/index.d.ts` transcribes `Accessor`, `Setter`, `Signal`
and both `createSignal` overloads byte-faithfully from solid-js@1.9.14
(`types/reactive/signal.d.ts:104-111`, `:138-139`) — the result side above all,
since `Signal<T>`'s exact two-item shape is what every slot case reads.
`SignalOptions` is reduced to its two caller-visible fields; a reduced option
object cannot widen an argument or create a claim. `.at(0)` in `atZero` needs
the ES2022 lib to type-check; no gate type-checks this fixture, and nothing in
it produces a finding.
