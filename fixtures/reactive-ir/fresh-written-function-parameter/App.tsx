import { createSignal } from "solid-js";

// The written object is the caller's: it may be a standard root.
export function decorate(fn: any, key: string, value: unknown) {
  fn[key] = value;
}

export function assignOnto(fn: any, config: Record<string, unknown>) {
  return Object.assign(fn, config);
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
