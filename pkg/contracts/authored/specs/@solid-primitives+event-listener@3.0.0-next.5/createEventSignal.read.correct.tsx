import { createEventSignal } from "@solid-primitives/event-listener";

export default function App() {
  const last = createEventSignal(window, "resize");
  return <p>{String(last())}</p>;
}
