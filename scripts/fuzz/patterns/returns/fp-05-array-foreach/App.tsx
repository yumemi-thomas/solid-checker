import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const A: typeof Object.assign = Object.assign;
export function App() {
  const [n] = createSignal(1);
  const fns = [{ run: (): number => n() }];
  A(fns, [{ run: (): number => 0 }]);
  fns.forEach((f) => log("r=" + f.run()));
  return <div />;
}
