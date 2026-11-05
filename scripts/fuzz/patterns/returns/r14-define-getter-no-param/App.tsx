import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const [n] = createSignal(1);
function useThing() { return { read: (): number => 0 }; }
export function App() {
  const x: { read: () => number } = useThing();
  (x as any).__defineGetter__("read", () => n);
  log("r=" + x.read());
  return <div />;
}
