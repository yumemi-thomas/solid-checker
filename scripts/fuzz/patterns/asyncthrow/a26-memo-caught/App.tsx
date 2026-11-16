import { createContext, createMemo, useContext } from "solid-js";
const Ctx = createContext(7);
export function App() {
  const m = createMemo(() => { (async () => { await Promise.resolve(); useContext(Ctx); })().catch(() => {}); return 1; });
  return <div>{m()}</div>;
}
