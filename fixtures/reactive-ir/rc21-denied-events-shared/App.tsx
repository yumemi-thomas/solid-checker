import { createSignal } from "solid-js";

export function App() {
  const [count, setCount] = createSignal(0);
  const shared = () => {
    console.log(count());
    setCount(1);
  };
  return <button onBlur={shared} onClick={shared}>go</button>;
}
