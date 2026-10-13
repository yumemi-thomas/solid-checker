import { createSignal } from "solid-js";
import { resolveFirst } from "@solid-primitives/refs";

export default function App() {
  const [count] = createSignal(0);
  resolveFirst(() => {
    count();
    return document.body;
  });
  return <p>{count()}</p>;
}
