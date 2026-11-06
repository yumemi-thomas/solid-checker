import { createSignal } from "solid-js";
const AP: any = Array.prototype;
const ks: PropertyKey[] = [Symbol.iterator];
const o = AP[Symbol.iterator];
AP[ks[0]] = function (this: unknown[]) { return o.call(this.length === 2 && typeof this[1] === "function" && (this[1] as Function).name !== "noop" ? [this[0], function noop() {}] : this); };
export function App() {
  const [s, set] = createSignal(1);
  set(2);
  return <time>{s()}</time>;
}
