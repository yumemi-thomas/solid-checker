import { onSettled } from "solid-js";
import { createEventBus } from "@solid-primitives/event-bus";
export default function App() {
  onSettled(() => { createEventBus<void>(); });
  return <p>candidate</p>;
}
