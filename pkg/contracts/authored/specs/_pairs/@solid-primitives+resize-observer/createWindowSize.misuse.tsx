import { onSettled } from "solid-js";
import { createWindowSize } from "@solid-primitives/resize-observer";
export default function App() {
  onSettled(() => { createWindowSize(); });
  return <p>candidate</p>;
}
