import { createOptimistic, createOptimisticStore, createProjection } from "solid-js";
import { identity } from "reactive-package";

// `identity`'s contract returns argument 0, so every read below is traced
// *through* the wrapper to the primitive the argument calls. What that
// primitive's call yields -- a two-slot tuple, or the source itself -- is the
// claim `Dialect::returns_reactive_tuple` makes.

const [value, setValue] = identity(createOptimistic(0));
const [, setOnly] = identity(createOptimistic(0));
const [store, setStore] = identity(createOptimisticStore({ count: 0 }));
const whole = identity(createOptimisticStore({ count: 0 }));
const projected = identity(createProjection<{ count: number }>(() => {}, { count: 0 }));

// Slot 0 of the signal tuple is the accessor. Reported either way: a wrong
// answer about the *shape* still lands on this name, because an unstructured
// reactive return is attributed to the binding's first name. This is the case
// the defect could not reach, and it is here to say so.
export function SignalSlot() {
  const snapshot = value();
  return <div>{snapshot}</div>;
}

// The falsifier. With the first slot elided, the binding's first name is the
// *setter*. Under the tuple shape it is slot 1, which the claim leaves empty,
// so nothing is reported. Under a bare accessor it is the whole return, and
// calling a setter is reported as an untracked read of a reactive accessor --
// a violation invented on correct code that `tsc` is entirely happy with.
export function ElidedSlot() {
  setOnly(1);
  return <div />;
}

// Slot 0 of the store tuple, same shape as SignalSlot and reported either way.
export function StoreSlot() {
  const read = store.count;
  return <div>{read}</div>;
}

// The second falsifier, in the store flavour. The binding is not a destructure
// at all, so under the tuple shape there is no slot to attribute and nothing is
// reported. Under a bare store path the *tuple* becomes the store, and the
// finding names a path that does not exist on it.
export function WholeTuple() {
  const read = whole[0].count;
  return <div>{read}</div>;
}

// The other direction, and the reason the list is a list rather than "every
// primitive that creates a source": `createProjection` returns the store
// itself. Its absence from the tuple row is what makes this read a store path.
export function Projection() {
  const read = projected.count;
  return <div>{read}</div>;
}

// Setters are never reads. Under either shape, and stated so that a change
// which starts reporting them fails here rather than in a real project.
export function Writes() {
  setValue(1);
  setStore({ count: 1 });
  return <div />;
}
