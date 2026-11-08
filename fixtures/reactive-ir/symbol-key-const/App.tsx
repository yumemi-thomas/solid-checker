import { createSignal } from "solid-js";

// A `const` holding a symbol names no member: this is no open write of a global,
// so the symbol-keyed sites below open nothing and `.sort()` stays standard.
const SLOT = Symbol.for("app.slot");
const LOCAL = Symbol("app.local");
const held = (globalThis as any);
held[SLOT] = () => {};
held[LOCAL as symbol] = () => {};

const site: any = { [Symbol.for("app.site")]: () => {} };
export const request = { sort: site.sort ?? "score" };

export function App() {
  const [names] = createSignal(["b", "a"]);
  const sorted = () => [...names()].sort();
  return <i>{sorted().length}</i>;
}
