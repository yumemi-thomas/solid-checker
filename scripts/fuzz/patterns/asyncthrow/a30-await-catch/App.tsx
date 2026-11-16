import { createContext, resolve, useContext } from "solid-js";
const Ctx = createContext(7);
export function App() {
  return <p ref={() => { (async () => { await (async () => { await Promise.resolve(); useContext(Ctx); })().catch(() => {}); })(); }}>x</p>;
}
