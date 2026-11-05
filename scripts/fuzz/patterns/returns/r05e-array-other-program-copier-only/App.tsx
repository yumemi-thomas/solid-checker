import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [n] = createSignal(1);
  const fns = [{ run: n }];
  const box = {};
  Object.assign(box, { z: 1 });
  fns.forEach((f) => log("r=" + f.run()));
  return <div />;
}
