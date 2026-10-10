import { onSettled } from "solid-js";
import { createTimer } from "@solid-primitives/timer";
export default function App() {
  onSettled(() => {
    createTimer(() => {}, 30_000, setInterval);
  });
  return <p>polling</p>;
}

