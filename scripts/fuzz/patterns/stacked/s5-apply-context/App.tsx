import { createContext, createEffect, useContext } from "solid-js";
const Ctx = createContext<number>();
export function App() {
  createEffect(() => 1, () => {
    useContext(Ctx);
  });
  return <i>app</i>;
}
