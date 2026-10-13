import { onSettled } from "solid-js";
import { toEffect } from "@solid-primitives/event-bus";
export default function App() {
  onSettled(() => {
    try { toEffect<string>(() => {}); } catch { /* Keep the emitted leaf diagnostic; allow the page to settle. */ }
  });
  return <p>ready</p>;
}
