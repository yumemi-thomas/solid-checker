import { onSettled } from "solid-js";
import { createEventStack } from "@solid-primitives/event-bus";
export default function App() {
  createEventStack<{ text: string }>();
  onSettled(() => {});
  return <p>ready</p>;
}
