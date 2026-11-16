import { createContext, useContext } from "solid-js";
const Ctx = createContext(7);
export function App() {
  return <button onClick={() => { (async () => { await Promise.resolve(); useContext(Ctx); })(); }}>x</button>;
}
// The lane clicks nothing by itself: the handler runs here.
export const after = async () => { document.querySelector("button")!.click(); };
