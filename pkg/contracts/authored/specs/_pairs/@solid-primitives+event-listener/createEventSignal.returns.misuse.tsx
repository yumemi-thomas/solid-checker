import { createEventSignal } from "@solid-primitives/event-listener";
export default function App() {
  const event = createEventSignal(window, "click");
  const value = event();
  return <p>{String(value)}</p>;
}
