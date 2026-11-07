import { createSignal } from "solid-js";

// Each of these may name a member, so each keeps the key open.
let rebound: symbol | string = Symbol.for("a");
rebound = "sort";
const { held } = { held: Symbol.for("b") };
const replacer = Symbol.replace;
const g = globalThis as unknown as Record<string | symbol, unknown>;
g[rebound] = () => {};
g[held] = () => {};
g[replacer] = () => {};
const defined = Symbol.for("c");
Object.defineProperty(g, defined, { value: () => {} });

const site: any = { [Symbol.for("d")]: () => {} };
export const request = { sort: site.sort ?? "score" };

export function App() {
  const [names] = createSignal(["b", "a"]);
  const sorted = () => [...names()].sort();
  return <i>{sorted().length}</i>;
}
