import { createEventSignal } from "@solid-primitives/event-listener";
export default function App() {
  createEventSignal(window, "click");
  return <p>candidate</p>;
}
