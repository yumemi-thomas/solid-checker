import { runWithOwner } from "solid-js";
import { createEventSignal } from "@solid-primitives/event-listener";
export default function App() {
  runWithOwner(null, () => { createEventSignal(window, "resize"); });
  return <p>ready</p>;
}
