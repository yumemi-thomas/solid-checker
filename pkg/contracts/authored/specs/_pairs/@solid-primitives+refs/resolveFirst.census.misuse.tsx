import { createSignal } from "solid-js";
import { resolveFirst } from "@solid-primitives/refs";

export default function App() {
  const [count, setCount] = createSignal(0);
  resolveFirst(() => {
    setCount(1);
    return document.body;
  });
  return <p>{count()}</p>;
}
