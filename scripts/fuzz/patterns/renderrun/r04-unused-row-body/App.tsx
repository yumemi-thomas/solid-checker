import { For, createContext, useContext } from "solid-js";
const Bare = createContext<number>();
export function App() {
  const unused = <For each={[1]}>{() => { useContext(Bare); return <li>x</li>; }}</For>;
  void unused;
  return <div>x</div>;
}
