import { onSettled } from "solid-js";
import { createRAF } from "@solid-primitives/raf";
export default function App() {
  onSettled(() => { createRAF(() => {}); });
  return <p>candidate</p>;
}
