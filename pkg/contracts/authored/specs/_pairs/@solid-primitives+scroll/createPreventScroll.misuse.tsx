import { onSettled } from "solid-js";
import { createPreventScroll } from "@solid-primitives/scroll";
export default function App() {
  onSettled(() => { createPreventScroll({ enabled: false }); });
  return <p>candidate</p>;
}
