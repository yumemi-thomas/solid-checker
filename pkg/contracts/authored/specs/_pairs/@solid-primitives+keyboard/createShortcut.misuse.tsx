import { onSettled } from "solid-js";
import { createShortcut } from "@solid-primitives/keyboard";
export default function App() {
  onSettled(() => { createShortcut(["Control", "K"], () => {}); });
  return <p>candidate</p>;
}
