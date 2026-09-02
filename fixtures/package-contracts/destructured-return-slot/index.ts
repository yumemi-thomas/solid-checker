import { createSignal, type Accessor, type Setter } from "solid-js";

// The tuple-returning helper every case below reads from. Its own return claim
// is the fixture's baseline: a two-item tuple whose first item is the
// discovered accessor.
function createPair<T>(initial: T): [Accessor<T>, Setter<T>] {
  const [value, setValue] = createSignal(initial);
  return [value, setValue];
}

function createNestedPair<T>(initial: T): [[Accessor<T>, Setter<T>]] {
  return [createPair(initial)];
}

// The object-returning helper the property cases read from. It destructures
// `createSignal` *directly* rather than `createPair`, because that is what
// makes `value` a discovered accessor and so gives this helper a real object
// claim -- without one, `projection`'s object arm has no base and every case
// below would be silent for the wrong reason. `other` is deliberately a value
// no leaf can shape, so the claim carries `value` and nothing else;
// `wrappedUnknownProperty` reads it.
function createRecord<T>(
  initial: T
): { value: Accessor<T>; setValue: Setter<T>; other: number } {
  const [value, setValue] = createSignal(initial);
  return { value, setValue, other: 1 };
}

// === Claims that must be made, exactly ===

// `@solid-primitives/spring` `createDerivedSpring`'s exact shape, written the
// way the slot *is* recoverable: an element access with a literal index names
// the item, and `projection` reads it from the tuple.
export function literalIndex<T>(initial: T): Accessor<T> {
  return createPair(initial)[0];
}

// The same element access wearing a transparent TypeScript wrapper. `as`
// preserves the runtime value, so the item is still the answer; before the fix
// the widened call lookup matched `createPair(initial)` by its start byte and
// handed back the whole tuple.
export function wrappedLiteralIndex<T>(initial: T): Accessor<T> {
  return createPair(initial)[0] as Accessor<T>;
}

// An identifier binding *is* bound to the whole initializer, so the tuple
// claim survives. This case is what separates carrying a binding's shape
// correctly from abandoning the derivation.
export function wholeBinding<T>(initial: T): [Accessor<T>, Setter<T>] {
  const pair = createPair(initial);
  return pair;
}

// One symbol, two declarations, only the second carrying the value. The
// binding scan has to keep looking for the matching name *with* an
// initializer, so this keeps the tuple claim too.
export function splitVarDeclaration<T>(initial: T): [Accessor<T>, Setter<T>] {
  var pair: [Accessor<T>, Setter<T>];
  var pair: [Accessor<T>, Setter<T>] = createPair(initial);
  return pair;
}

// A static property name really does resolve against the object claim. This is
// the control that makes every property negative below mean something: without
// it, a silent `computedProperty` could be silence for want of any base shape
// rather than the refusal of a text match.
export function staticProperty<T>(initial: T): Accessor<T> {
  return createRecord(initial).value;
}

// === Claims that must be withheld: the slot is real, its shape is not
// derivable from the initializer ===

// `createDerivedSpring`'s actual shape. Was the whole tuple.
export function arraySlot<T>(initial: T): Accessor<T> {
  const [value] = createPair(initial);
  return value;
}

// The mirror. The fallback used to ask the binding for its *first* name, so a
// reference to `setValue` inherited `value`'s discovered accessor identity and
// the setter was published as an accessor.
export function arraySlotSetter<T>(initial: T): Setter<T> {
  const [, setValue] = createPair(initial);
  return setValue;
}

// === Claims that must be withheld: no exact slot exists to name ===

// A non-literal index names no particular item.
export function computedIndex<T>(initial: T, at: 0 | 1): Accessor<T> | Setter<T> {
  return createPair(initial)[at];
}

// The same under a wrapper. The peel must *commit* to the projection's answer:
// falling through on absence would hand the widened call lookup the original
// span, whose start byte still matches `createPair(initial)`, and publish the
// whole tuple for an index that named nothing.
export function wrappedComputedIndex<T>(
  initial: T,
  at: 0 | 1
): Accessor<T> | Setter<T> {
  return createPair(initial)[at] as Accessor<T> | Setter<T>;
}

// A computed member's property span is an *expression*, not a name, and its
// source text is not the property it reads. The parameter is named `value` on
// purpose: it spells one of the object's own property names, so a text match
// answered the `value` accessor for a read that may just as well be
// `setValue`. Only a static name resolves a property.
export function computedProperty<T>(
  initial: T,
  value: "value" | "setValue"
): Accessor<T> | Setter<T> {
  return createRecord(initial)[value];
}

// A quoted computed member spells the quotes with the name, so it resolves no
// property either -- and under a wrapper it must still not fall through to the
// whole object.
export function wrappedQuotedMember<T>(initial: T): Accessor<T> {
  return createRecord(initial)["value"] as Accessor<T>;
}

// A property the object claim does not carry. The answer is absence, not the
// object the property was read from.
export function wrappedUnknownProperty<T>(initial: T): number {
  return createRecord(initial).other as number;
}

// `.at(0)` is a member call, not an element access. (Typing it needs the
// ES2022 lib; nothing in this fixture is type-checked by a gate.)
export function atZero<T>(initial: T): Accessor<T> | Setter<T> | undefined {
  return createPair(initial).at(0);
}

// A defaulted element binds the item *or* the default, and the item's shape
// does not describe that union.
export function defaultedSlot<T>(initial: T, fallback: Accessor<T>): Accessor<T> {
  const [value = fallback] = createPair(initial);
  return value;
}

// A nested pattern binds a value *inside* item 0. `array_slots` reports a
// nested element's first identifier, which is exactly why the slot may not be
// inferred from that table.
export function nestedSlot<T>(initial: T): Accessor<T> {
  const [[value]] = createNestedPair(initial);
  return value;
}

// A rest element binds a slice of the tuple, not an item of it.
export function restSlot<T>(initial: T): [Setter<T>] {
  const [, ...rest] = createPair(initial);
  return rest;
}

// An object pattern binds a property of the result, not the result.
export function objectSlot<T>(initial: T): Accessor<T> {
  const { value } = createRecord(initial);
  return value;
}
