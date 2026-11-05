import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
function useThing(n: () => number) { return { read: (): number => n() }; }
export function App() {
  const [n] = createSignal(1);
  const x = useThing(n);
  log("r=" + x.read());
  return <div />;
}
