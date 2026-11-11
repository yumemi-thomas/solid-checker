import { createSignal } from "solid-js";

// The helper defines another member than the one the program calls, under a literal key: `Number.isFinite` is the declared one.
const patch = (target: any, impl: unknown) => {
  Object.defineProperty(target, "isNaN", { value: impl, configurable: true, writable: true });
};

export function C1() {
  const [r1] = createSignal(3);
  // `Number` goes to the helper: its `isNaN` now runs what it is given; `isFinite` never runs the function.
  patch(Number, (f: any) => (typeof f === "function" ? (f(), true) : false));
  const sf1 = () => r1();
  Number.isFinite(sf1 as unknown as number);
  return <p>{sf1()}</p>;
}
