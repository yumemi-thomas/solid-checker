import { createContext, useContext } from "solid-js";
const Ctx = createContext(7);
export function App() {
  return <p ref={() => { const p = (async () => { await Promise.resolve(); useContext(Ctx); })(); void p.catch(() => {}); }}>x</p>;
}
