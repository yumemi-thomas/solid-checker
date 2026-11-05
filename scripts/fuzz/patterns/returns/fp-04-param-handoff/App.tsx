import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const A: typeof Object.assign = Object.assign;
function use(o: { read: () => number }) { log("r=" + o.read()); }
export function App() {
  const [n] = createSignal(1);
  const t = { read: (): number => n() };
  A(t, { read: (): number => 0 });
  use(t);
  return <div />;
}
