import { For, createContext, useContext } from "solid-js";
const Ctx = createContext(7);
export function App() {
  const unused = <For each={[1]}>{() => <li><p ref={() => { useContext(Ctx); }}>x</p></li>}</For>;
  void unused;
  return <div>x</div>;
}
