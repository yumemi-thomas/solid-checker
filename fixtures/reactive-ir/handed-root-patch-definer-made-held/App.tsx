import { createSignal } from "solid-js";

const define = Object.defineProperty;

export function linkProps(source: Record<string, () => unknown>) {
  const props = {};
  for (const key of Object.keys(source)) {
    define(props, key, { get: source[key], enumerable: true });
  }
  return props;
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
