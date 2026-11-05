import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [n] = createSignal(1);
  const t = { read: n };
  const get = () => t;
  log("r=" + get().read());
  return <div />;
}
