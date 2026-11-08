import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const A: typeof Object.assign = Object.assign;
function useThing() { return { read: (): number => 0 }; }
export function App() {
  const x = useThing();
  log("r=" + x.read());
  return <div />;
}
