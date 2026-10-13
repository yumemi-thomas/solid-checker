import { onSettled } from "solid-js";
import { createScrollPosition } from "@solid-primitives/scroll";
export default function App() {
  onSettled(() => { createScrollPosition(window); });
  return <p>candidate</p>;
}
