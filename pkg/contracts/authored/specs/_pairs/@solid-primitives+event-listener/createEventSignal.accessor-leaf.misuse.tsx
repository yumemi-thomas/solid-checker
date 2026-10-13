import { onSettled } from "solid-js";
import { createEventSignal } from "@solid-primitives/event-listener";
export default function App() {
  onSettled(() => { try { createEventSignal(() => window, "resize"); } catch { /* Keep the package diagnostic and allow mount. */ } });
  return <p>ready</p>;
}
