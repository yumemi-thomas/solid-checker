import { createEventListenerMap } from "@solid-primitives/event-listener";
createEventListenerMap(window, { resize: () => {} });
export default function App() { return <p>ready</p>; }
