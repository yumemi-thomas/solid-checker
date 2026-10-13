import { onSettled } from "solid-js";
import { makeEventListener } from "@solid-primitives/event-listener";
export default function App() {
  onSettled(() => { makeEventListener(window, "click", () => {}); });
  return <p>candidate</p>;
}
