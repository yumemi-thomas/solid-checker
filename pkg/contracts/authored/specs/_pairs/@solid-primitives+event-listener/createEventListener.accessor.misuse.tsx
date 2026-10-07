import { onSettled } from "solid-js";
import { createEventListener } from "@solid-primitives/event-listener";
export default function App() {
  onSettled(() => { createEventListener(() => window, "keydown", () => {}); });
  return <p>keys</p>;
}
