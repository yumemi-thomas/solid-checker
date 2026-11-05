import { createSignal, createMemo, createEffect, createContext, useContext, untrack, flush } from "solid-js";
const log = (...a: any[]) => (window as any).__log(...a);
const A: typeof Object.assign = Object.assign;
const api = { read: (): number => 0 };
function useApi() { return api; }
export function App() {
  const [n] = createSignal(1);
  A(api, { read: () => n() });
  const x = useApi();
  log("r=" + x.read());
  return <div />;
}
