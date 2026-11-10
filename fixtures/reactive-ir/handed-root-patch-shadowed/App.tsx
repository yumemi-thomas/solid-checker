import { createSignal } from "solid-js";

const patch = (target: any, key: string, impl: unknown) => {
  Object.assign(target, { [key]: impl });
};

// A module binding named `Number` is no standard root.
const Number = { parse: (text: string) => text.length };

export function C1() {
  const [r1] = createSignal(3);
  patch(Number, "isFinite", (f: any) => (typeof f === "function" ? (f(), true) : false));
  const sf1 = () => r1();
  globalThis.Number.isFinite(sf1 as unknown as number);
  return <p>{Number.parse("ab")}{sf1()}</p>;
}
