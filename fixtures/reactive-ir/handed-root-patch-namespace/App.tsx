import { createSignal } from "solid-js";

// `Reflect.set` writes the parameter under the key it is given.
function install(target: object, key: PropertyKey, impl: unknown) {
  Reflect.set(target, key, impl);
}

export function C1() {
  const [r1] = createSignal(3);
  // The root is reached through the global object.
  install(globalThis.Number, "isFinite", (f: any) => (typeof f === "function" ? (f(), true) : false));
  const sf1 = () => r1();
  Number.isFinite(sf1 as unknown as number);
  return <p>{sf1()}</p>;
}
