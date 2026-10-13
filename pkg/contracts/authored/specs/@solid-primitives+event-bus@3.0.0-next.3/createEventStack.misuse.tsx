import { createEventStack } from "@solid-primitives/event-bus";
export default function App() {
  const stack = createEventStack<{ text: string }>();
  const current = stack.value().length; // Expected STRICT_READ_UNTRACKED.
  return <p>{String(current)}</p>;
}
