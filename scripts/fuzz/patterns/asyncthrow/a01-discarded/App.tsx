import { createContext, useContext } from "solid-js";
const Ctx = createContext(7);
export function App() {
  return <p ref={() => { (async () => { await Promise.resolve(); useContext(Ctx); })(); }}>x</p>;
}
