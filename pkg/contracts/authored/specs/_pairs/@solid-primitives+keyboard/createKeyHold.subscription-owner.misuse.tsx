import { onSettled } from "solid-js";
import { createKeyHold } from "@solid-primitives/keyboard";
export default function App() {
  onSettled(() => { createKeyHold("A"); });
  return <p>candidate</p>;
}
