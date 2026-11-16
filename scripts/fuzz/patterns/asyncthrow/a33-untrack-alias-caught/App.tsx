import { createContext, resolve, untrack, useContext } from "solid-js";
const Ctx = createContext(7);
export function App() {
  const run = async () => { await Promise.resolve(); useContext(Ctx); };
  const again = run;
  return <p ref={() => { untrack(again).catch(() => {}); }}>x</p>;
}
