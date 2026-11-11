import { createSignal } from "solid-js";

// The helper defines its parameter under a key no literal names: any member may be replaced.
const patch = (target: any, key: string, impl: unknown) => {
  Object.defineProperty(target, key, { value: impl, configurable: true, writable: true });
};

export function C1() {
  const [r1] = createSignal(3);
  // `Number` goes to the helper: its `isFinite` now runs what it is given.
  patch(Number, "isFinite", (f: any) => (typeof f === "function" ? (f(), true) : false));
  const sf1 = () => r1();
  Number.isFinite(sf1 as unknown as number);
  return <p>{sf1()}</p>;
}
