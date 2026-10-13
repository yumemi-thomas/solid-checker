/** @jsxImportSource @solidjs/web */
import { createSignal, untrack } from "solid-js";
import { createEventStack } from "@solid-primitives/event-bus";
export default function App() {
  const [source] = createSignal(1);
  const { emit } = createEventStack<{ text: string }>({
    toValue: event => { source(); return event; }
  });
  untrack(() => emit({ text: "event" }));
  return <p>ready</p>;
}
