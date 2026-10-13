import { createEventListenerMap } from "@solid-primitives/event-listener";
export default function App() {
  createEventListenerMap(window, { resize: () => {} });
  return <p>ready</p>;
}
