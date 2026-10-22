import { createSignal } from "solid-js";

const [count] = createSignal(0);

function Root() {
  return <i />;
}

function Sub() {
  const value = count();
  return <b>{value}</b>;
}

// Uncertifiable: `Object.defineProperty` on a parameter. Its target may be
// `Object`, and nothing in the project proves otherwise.
export function define(target: object, key: string) {
  Object.defineProperty(target, key, { value: () => null });
}

const Tabs = Object.assign(Root, { Sub });

export function Screen() {
  return <Tabs.Sub />;
}
