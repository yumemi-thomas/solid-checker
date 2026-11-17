import { createSignal } from "solid-js";

// ADR 1036: solid-query's createInfiniteQuery and probus-hk's getters define on a fresh literal.
export function withPages(descriptors: PropertyDescriptorMap) {
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
