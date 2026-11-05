import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const A: typeof Object.assign = Object.assign;
function useThing(n: () => number) { return Object.assign({ read: (): number => 0 }, { read: n }); }
export function App() {
  const [n] = createSignal(1);
  const x = useThing(n);
  log("r=" + x.read());
  return <div />;
}
