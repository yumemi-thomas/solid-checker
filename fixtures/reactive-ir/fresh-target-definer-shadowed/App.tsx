import { createSignal } from "solid-js";

// A shadowed `Object`.
export function withPages(target: object, descriptors: PropertyDescriptorMap) {
  // A local `Object` is no definer of the standard library's: its `defineProperties` may write anything.
  const Object = { defineProperties: (_: object, map: PropertyDescriptorMap) => globalThis.Object.defineProperties(target, map) };
  return Object.defineProperties({}, descriptors);
}

// A key that is a number by its syntax names no member: a loop counter, an update, arithmetic.
export function fill(out: unknown[], items: unknown[]) {
  for (let i = 0; i < items.length; i++) out[i] = items[i];
  let length = 0;
  out[length++] = items[0];
  out[items.length - 1] = items[0];
  return out;
}

// A standard root goes on as a value.
const box = (value: unknown, key: string) => {
  const held: Record<string, unknown> = {};
  held[key] = value;
  return held;
};

export function C1() {
  const [r1] = createSignal(3);
  const boxed = box(Number, "n");
  const sf1 = () => r1();
  Number.isFinite(sf1 as unknown as number);
  return <p>{Object.keys(boxed).length}{sf1()}</p>;
}
