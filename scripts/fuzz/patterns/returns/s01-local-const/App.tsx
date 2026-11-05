import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const A: typeof Object.assign = Object.assign;
export function App() {
  const [n] = createSignal(1);
  const api = { read: (): number => 0 };
  A(api, { read: () => n() });
  log("r=" + api.read());
  return <div />;
}
