import { createContext, useContext } from "solid-js";
const Ctx = createContext(7);
const fromHandler = async () => {
  await Promise.resolve();
  useContext(Ctx);
};
export function App() {
  return <button onClick={() => fromHandler()}>x</button>;
}
// The lane clicks nothing by itself: the handler runs here.
export const after = async () => { document.querySelector("button")!.click(); };
