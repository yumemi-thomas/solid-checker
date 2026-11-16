import { createContext, untrack, useContext } from "solid-js";
const Ctx = createContext(7);
export function App() {
  return <p ref={() => { untrack(async () => { await Promise.resolve(); useContext(Ctx); }).catch(() => {}); }}>x</p>;
}
