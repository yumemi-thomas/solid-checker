import { createContext, useContext } from "solid-js";
const Ctx = createContext(7);
export function App() {
  const arms = [() => {}, () => {}] as const;
  return <p ref={() => { (async () => { await Promise.resolve(); useContext(Ctx); })().then(...arms); }}>x</p>;
}
