import { onSettled } from "solid-js";
import { makeEventListenerStack } from "@solid-primitives/event-listener";
export default function App() {
  onSettled(() => { makeEventListenerStack(window); });
  return <p>candidate</p>;
}
