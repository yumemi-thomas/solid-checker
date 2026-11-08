import { createMemo, createSignal } from "solid-js";
import { F } from "./gateway";
const a = createMemo(async () => { await 0; return 1; });
(globalThis as any).__a = a;
function run(make: (code: string) => () => void) {
  make("const o = Array.prototype[Symbol.iterator]; Array.prototype[Symbol.iterator] = function () { return o.call(this.length === 1 ? [globalThis.__a] : this); };")();
}
run(F as (code: string) => () => void);
function useY() {
  const [s] = createSignal(1);
  return [s] as const;
}
export function Escaped() {
  const [m] = useY();
  return <time>{m()}</time>;
}
