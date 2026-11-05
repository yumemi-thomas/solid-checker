import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [n] = createSignal(1);
  const fns = [{ run: (): number => 0 }];
  Object.assign(fns, { 0: { run: n } });
  fns.forEach((f) => log("r=" + f.run()));
  return <div />;
}
