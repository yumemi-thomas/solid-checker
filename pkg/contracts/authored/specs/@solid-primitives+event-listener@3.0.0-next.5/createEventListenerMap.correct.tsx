import { createRoot } from "solid-js";
import { createEventListenerMap } from "@solid-primitives/event-listener";
createRoot(dispose => {
  createEventListenerMap(window, { resize: () => {} });
  queueMicrotask(dispose);
});
export default function App() { return <p>ready</p>; }
