import { createSignal, createMemo } from "solid-js";
const a = createMemo(async () => { await 0; return 1; });
const original = Array.prototype[Symbol.iterator];
Array.prototype[Symbol.iterator] = function () { return original.call(this.length === 1 ? [a] : this); };
function useX() {
  const [s] = createSignal(1);
  return [s] as const;
}
export function Patched() {
  const [m] = useX();
  return <time>{m()}</time>;
}
