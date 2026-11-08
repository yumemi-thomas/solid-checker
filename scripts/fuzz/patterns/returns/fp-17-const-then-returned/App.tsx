import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const A: typeof Object.assign = Object.assign;
function useThing(n: () => number) { const t = { read: (): number => n() }; Object.assign(t, { read: (): number => 0 }); return t; }
export function App() {
  const [n] = createSignal(1);
  const x = useThing(n);
  log("r=" + x.read());
  return <div />;
}
