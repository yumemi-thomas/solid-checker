import { onSettled } from "solid-js";
import { createEventSignal } from "@solid-primitives/event-listener";
export default function App() {
  onSettled(() => { createEventSignal(window, "click"); });
  return <p>candidate</p>;
}
