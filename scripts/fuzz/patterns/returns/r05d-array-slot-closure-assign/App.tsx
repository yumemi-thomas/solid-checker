import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [n] = createSignal(1);
  const fns = [(): number => 0];
  Object.assign(fns, [n]);
  fns.forEach((f) => log("r=" + f()));
  return <div />;
}
