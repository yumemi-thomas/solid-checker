/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { createEventStack } from "@solid-primitives/event-bus";
export default function App() {
  const [source] = createSignal(1);
  const { emit } = createEventStack<{ text: string }>({
    toValue: event => { source(); return event; }
  });
  emit({ text: "event" });
  return <p>ready</p>;
}
