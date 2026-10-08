import { createEventBus } from "@solid-primitives/event-bus";
export default function App() {
  createEventBus<void>();
  return <p>candidate</p>;
}
