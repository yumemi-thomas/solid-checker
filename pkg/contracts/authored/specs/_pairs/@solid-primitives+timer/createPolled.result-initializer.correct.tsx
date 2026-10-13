import { createSignal, untrack } from "solid-js";
import { createPolled } from "@solid-primitives/timer";
export default function App() {
  const [count, setCount] = createSignal(0);
  untrack(() => createPolled(() => () => {  return count(); }, 1000));
  return <p>{count()}</p>;
}
