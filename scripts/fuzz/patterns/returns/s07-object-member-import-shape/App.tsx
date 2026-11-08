import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const A: typeof Object.assign = Object.assign;
const timer = Object.freeze({ read: (): number => 0 });
const box = { read: (): number => 0 };
export function App() {
  const [n] = createSignal(1);
  A(box, { read: () => n() });
  log("r=" + box.read() + timer.read());
  return <div />;
}
