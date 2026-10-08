import { onSettled } from "solid-js";
import { createReconnectingWS } from "@solid-primitives/websocket";
export default function App() {
  onSettled(() => {
    try { createReconnectingWS("ws://127.0.0.1:9/batch3a"); } catch { /* Retain the emitted leaf diagnostic. */ }
  });
  return <p>ready</p>;
}
