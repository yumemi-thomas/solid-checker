import { createEventListenerMap } from "@solid-primitives/event-listener";
export default function App() {
  createEventListenerMap(window, { click: () => {} });
  return <p>candidate</p>;
}
