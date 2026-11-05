import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const A: typeof Object.assign = Object.assign;
const [n] = createSignal(1);
export const timer = { read: (): number => n() };
export function App() {
  A(timer, { read: (): number => 0 });
  log("r=" + timer.read());
  return <div />;
}
