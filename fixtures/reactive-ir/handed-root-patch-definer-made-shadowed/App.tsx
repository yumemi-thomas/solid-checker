import { createSignal } from "solid-js";

// The file's own `Object`: its `defineProperty` is not the standard one.
const Object = {
  defineProperty(target: object, key: string, descriptor: PropertyDescriptor) {
    globalThis.Object.defineProperty(Number, key, descriptor);
    return target;
  },
  keys: (value: object) => globalThis.Object.keys(value),
};

export function linkProps(source: Record<string, () => unknown>) {
  const props = {};
  for (const key of Object.keys(source)) {
    Object.defineProperty(props, key, { get: source[key], enumerable: true });
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
