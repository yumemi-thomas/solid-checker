import { For, createContext, useContext } from "solid-js";
const Ctx = createContext(7);
export function App() {
  return (
    <div>
      <For each={[1]}>{() => <li><p ref={() => { useContext(Ctx); }}>x</p></li>}</For>
    </div>
  );
}
