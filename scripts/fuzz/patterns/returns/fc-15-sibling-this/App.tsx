import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const A: typeof Object.assign = Object.assign;
function useThing(n: () => number) { return { read: (): number => 0, self: function (this: any) { return this; } }; }
export function App() {
  const [n] = createSignal(1);
  const x = useThing(n);
  const y: any = x.self();
  const k = String("read");
  y[k] = n;
  log("r=" + x.read());
  return <div />;
}
