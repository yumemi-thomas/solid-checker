import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const A: typeof Object.assign = Object.assign;
const Ctx = createContext<{ read: () => number }>();
function Child() { const v = useContext(Ctx); log("r=" + v!.read()); return <i />; }
export function App() {
  const [n] = createSignal(1);
  const v = { read: (): number => 0 };
  A(v, { read: () => n() });
  return <Ctx value={v}><Child /></Ctx>;
}
