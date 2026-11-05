import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const A: typeof Object.assign = Object.assign;
export function App() {
  const [n] = createSignal(1);
  const t = { read(): number { return 0; }, boot() { A(this, { read: () => n() }); } };
  t.boot();
  log("r=" + t.read());
  return <div />;
}
