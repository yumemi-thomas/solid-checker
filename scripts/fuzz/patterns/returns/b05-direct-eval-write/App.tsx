import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const A: typeof Object.assign = Object.assign;
function useThing() { return { read: (): number => 0 }; }
export function App() {
  const [n] = createSignal(1);
  const x = useThing();
  (globalThis as any).__n = n;
  eval("x.read = () => globalThis.__n()");
  log("r=" + x.read());
  return <div />;
}
