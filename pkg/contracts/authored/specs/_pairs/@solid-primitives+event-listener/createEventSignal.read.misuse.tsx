import { createEventSignal } from "@solid-primitives/event-listener";

export default function App() {
  const last = createEventSignal(window, "resize");
  const current = last();
  return <p>{String(current)}</p>;
}
