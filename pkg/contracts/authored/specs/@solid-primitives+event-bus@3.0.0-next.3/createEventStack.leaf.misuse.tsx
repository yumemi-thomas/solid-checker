import { onSettled } from "solid-js";
import { createEventStack } from "@solid-primitives/event-bus";
export default function App() {
  onSettled(() => {
    try { createEventStack<{ text: string }>(); } catch { /* Keep the emitted leaf diagnostic; allow the page to settle. */ }
  });
  return <p>ready</p>;
}
