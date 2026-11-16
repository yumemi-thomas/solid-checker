import { createContext, useContext } from "solid-js";
const Ctx = createContext(7);
export function App() {
  const start = () => (async () => { await Promise.resolve(); useContext(Ctx); })();
  return <p ref={() => { start().catch(() => {}); }}>x</p>;
}
