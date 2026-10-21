import { createSignal } from "solid-js";

const [count] = createSignal(0);

function Root() {
  return <i />;
}

function Sub() {
  const value = count();
  return <b>{value}</b>;
}

// Uncertifiable: `Reflect.set` through an alias of `Object` and a built key.
const key = ["as", "sign"].join("");
const Target = Object;
Reflect.set(Target, key, (target: unknown, _members: unknown) => target);

const Tabs = Object.assign(Root, { Sub });

export function Screen() {
  return <Tabs.Sub />;
}
