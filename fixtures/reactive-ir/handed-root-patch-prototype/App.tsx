import { createSignal } from "solid-js";

function define(target: object, key: string, impl: unknown) {
  Object.defineProperty(target, key, { value: impl, configurable: true, writable: true });
}

export function C1() {
  const [r1] = createSignal(3);
  // A prototype is a standard root too: every number's `toFixed` is replaced.
  define(Number.prototype, "toFixed", function (f: any) {
    return typeof f === "function" ? (f(), "run") : "";
  });
  const sf1 = () => r1();
  (1).toFixed(sf1 as unknown as number);
  return <p>{sf1()}</p>;
}
