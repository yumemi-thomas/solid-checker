import { For, createContext, useContext } from "solid-js";
const Bare = createContext<number>();
export function App() {
  return (
    <div>
      <For each={[1]}>{() => { useContext(Bare); return <li>x</li>; }}</For>
    </div>
  );
}
