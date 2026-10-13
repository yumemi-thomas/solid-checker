import { createRoot } from "solid-js";
import { createReconnectingWS } from "@solid-primitives/websocket";
createRoot(dispose => {
  createReconnectingWS("ws://127.0.0.1:9/batch3a");
  queueMicrotask(dispose);
});
export default function App() { return <p>ready</p>; }
