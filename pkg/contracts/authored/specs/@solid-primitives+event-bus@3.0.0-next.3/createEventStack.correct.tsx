import { createEventStack } from "@solid-primitives/event-bus";
export default function App() {
  const stack = createEventStack<{ text: string }>();
  return <p>{String(stack.value().length)}</p>;
}
