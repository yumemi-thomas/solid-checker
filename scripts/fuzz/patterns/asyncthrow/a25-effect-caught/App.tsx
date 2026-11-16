import { createContext, createRenderEffect, useContext } from "solid-js";
const Ctx = createContext(7);
export function App() {
  createRenderEffect(() => { (async () => { await Promise.resolve(); useContext(Ctx); })().catch(() => {}); }, () => {});
  return <div></div>;
}
