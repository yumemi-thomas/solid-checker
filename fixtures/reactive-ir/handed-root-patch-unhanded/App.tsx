import { createSignal } from "solid-js";

const patch = (target: any, key: string, impl: unknown) => {
  Object.assign(target, { [key]: impl });
};

export function C1() {
  const [r1] = createSignal(3);
  // The helper writes an object of the program's own: no standard root reaches it.
  const own: Record<string, unknown> = {};
  patch(own, "isFinite", (f: any) => (typeof f === "function" ? (f(), true) : false));
  // `map` calls `Number` and keeps it nowhere: no hand-off.
  const sizes = ["1", "2"].map(Number);
  const sf1 = () => r1();
  Number.isFinite(sf1 as unknown as number);
  return <p>{sizes.length}{sf1()}</p>;
}
