import { createEventSignal } from "@solid-primitives/event-listener";
export default function App() {
  const event = createEventSignal(window, "click");
  return <p>{String(event())}</p>;
}
