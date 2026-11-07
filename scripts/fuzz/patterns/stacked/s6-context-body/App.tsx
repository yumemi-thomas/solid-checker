import { createContext, useContext } from "solid-js";
const Ctx = createContext<number>();
export function App() {
  useContext(Ctx);
  return <i>app</i>;
}
