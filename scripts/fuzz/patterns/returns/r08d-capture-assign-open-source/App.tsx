import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
export function App() {
  const [n] = createSignal(1);
  const src: { read: () => number } = { read: n };
  const t = { read: (): number => 0 };
  const get = () => t;
  Object.assign(t, src);
  log("r=" + get().read());
  return <div />;
}
