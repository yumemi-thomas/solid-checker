import "./CycleRender";
import { createSignal } from "solid-js";

const [count] = createSignal(0);

function Root() {
  return <i />;
}

function Sub() {
  const value = count();
  return <b>{value}</b>;
}

// Uncertifiable: this module is on an import cycle. `CycleRender` runs while
// this module is still evaluating its imports, renders `Screen` (a function
// declaration, already callable) and reaches `Tabs` uninitialized, so the tag
// throws before `Sub` runs.
const Tabs = Object.assign(Root, { Sub });

export function Screen() {
  return <Tabs.Sub />;
}
