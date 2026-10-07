import { createEventListener } from "@solid-primitives/event-listener";
export default function App() {
  createEventListener(window, "keydown", () => {});
  return <p>keys</p>;
}

