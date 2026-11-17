import { createSignal } from "solid-js";

// The shapes of `@tanstack/solid-router` 2.0.0-rc.9's `link.jsx:317-332` and `@tanstack/solid-query` 6.0.0-rc.5's
// `dev.js:565` (ADR 1045): a standard definer under a key no literal names, on an object this file makes.
export function linkProps(props: object, source: Record<string, unknown>, extra: string[], read: (key: string) => unknown) {
  for (const key of Object.keys(source)) {
    Object.defineProperty(props, key, Object.getOwnPropertyDescriptor(source, key) ?? { value: undefined });
  }
  for (const key of extra) {
    Reflect.defineProperty(props, key, { get: () => read(key), enumerable: true, configurable: true });
  }
  return props;
}

export function infinite(layer: object, next: () => void) {
  return Object.defineProperties({}, {
    ...Object.getOwnPropertyDescriptors(layer),
    fetchNextPage: { value: next, enumerable: true },
  });
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
