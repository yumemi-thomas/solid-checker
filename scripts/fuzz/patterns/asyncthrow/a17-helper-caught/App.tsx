import { createContext, useContext } from "solid-js";
const Ctx = createContext(7);
export function App() {
  const run = async (callback: () => void) => { await Promise.resolve(); callback(); };
  return <p ref={() => { run(() => { useContext(Ctx); }).catch(() => {}); }}>x</p>;
}
