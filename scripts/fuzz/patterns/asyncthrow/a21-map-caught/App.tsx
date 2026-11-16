import { createContext, useContext } from "solid-js";
const Ctx = createContext(7);
export function App() {
  return <p ref={() => { Promise.all([1].map(async () => { await Promise.resolve(); useContext(Ctx); })).catch(() => {}); }}>x</p>;
}
