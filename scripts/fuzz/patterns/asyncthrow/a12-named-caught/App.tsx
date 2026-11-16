import { createContext, useContext } from "solid-js";
const Ctx = createContext(7);
export function App() {
  return <p ref={() => { const run = async () => { await Promise.resolve(); useContext(Ctx); }; run().catch(() => {}); }}>x</p>;
}
