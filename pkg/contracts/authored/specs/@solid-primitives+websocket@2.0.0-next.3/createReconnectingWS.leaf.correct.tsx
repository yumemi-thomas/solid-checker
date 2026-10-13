import { onSettled } from "solid-js";
import { createReconnectingWS } from "@solid-primitives/websocket";
export default function App() {
  createReconnectingWS("ws://127.0.0.1:9/batch3a");
  onSettled(() => {});
  return <p>ready</p>;
}
