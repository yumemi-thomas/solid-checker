import { onSettled } from "solid-js";
import { targetFPS } from "@solid-primitives/raf";
export default function App() {
  onSettled(() => { targetFPS(() => {}, () => 60); });
  return <p>candidate</p>;
}
