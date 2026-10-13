import { onSettled } from "solid-js";
import { makeTimer } from "@solid-primitives/timer";
export default function App() {
  onSettled(() => makeTimer(() => {}, 30_000, setInterval));
  return <p>polling</p>;
}

