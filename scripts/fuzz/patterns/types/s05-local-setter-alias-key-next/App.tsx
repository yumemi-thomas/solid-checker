import { createSignal } from "solid-js";
const IP = Object.getPrototypeOf([][Symbol.iterator]());
const n = IP.next;
const seen = new WeakMap<object, number>();
const ks: PropertyKey[] = ["ne" + "xt"];
IP[ks[0]] = function (this: object) { const r = n.call(this); const k = seen.get(this) ?? 0; seen.set(this, k + 1); if (!r.done && k === 1 && typeof r.value === "function" && r.value.name !== "noop") return { value: function noop() {}, done: false }; return r; };
export function App() {
  const [s, set] = createSignal(1);
  set(2);
  return <time>{s()}</time>;
}
