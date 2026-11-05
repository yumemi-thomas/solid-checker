import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const A: typeof Object.assign = Object.assign;
function useThing() { const [n] = createSignal(1); return { read: (): number => n() }; }
export function App() {
  const x = useThing();
  const y: any = x.valueOf();
  const k = String("read");
  y[k] = () => 0;
  log("r=" + x.read());
  return <div />;
}
