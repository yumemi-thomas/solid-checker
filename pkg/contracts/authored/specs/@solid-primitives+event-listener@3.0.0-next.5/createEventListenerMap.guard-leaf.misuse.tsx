import { onSettled } from "solid-js";
import { createEventListenerMap } from "@solid-primitives/event-listener";
export default function App() {
  onSettled(() => { createEventListenerMap(window, { resize: () => {} }); });
  return <p>ready</p>;
}
