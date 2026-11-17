import { createSignal } from "solid-js";

// A project `Map` that calls what it is handed: no standard-library carrier, so the write stays a violation.
class Map<K, V> {
  forEach(f: (value: V, key: K) => void) {
    f(undefined as V, undefined as K);
  }
}
export function ShadowedMap() {
  const [, setS] = createSignal(0);
  new Map<number, number>().forEach(() => {
    setS(1);
  });
  return <p />;
}

// A local `Array` whose `from` calls its callback.
export function ShadowedArray() {
  const [, setS] = createSignal(0);
  const Array = { from: (_source: unknown, f: () => number) => f() };
  Array.from([], () => {
    setS(2);
    return 0;
  });
  return <p />;
}
