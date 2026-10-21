import { createSignal } from "solid-js";

const [count] = createSignal(0);

function Root() {
  return <i />;
}

function Sub() {
  const value = count();
  return <b>{value}</b>;
}

// Uncertifiable: a string-built key writes `Object.assign` through a computed
// member of `Object`, which no static name or prototype scan sees. Any
// computed write with a non-literal key whose object may be `Object` (the
// identifier, an alias, the global object, an unresolved value) withholds
// every `Object.assign` holder in the project.
const key = ["as", "sign"].join("");
(Object as any)[key] = (target: unknown, _members: unknown) => target;

const Tabs = Object.assign(Root, { Sub });

export function Screen() {
  return <Tabs.Sub />;
}
