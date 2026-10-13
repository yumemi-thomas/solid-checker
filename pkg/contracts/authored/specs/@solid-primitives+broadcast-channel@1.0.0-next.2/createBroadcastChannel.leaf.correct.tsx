/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createBroadcastChannel } from "@solid-primitives/broadcast-channel";
export default function App() {
  createBroadcastChannel<string>("research-batch-4a");
  onSettled(() => {});
  return <p>ready</p>;
}
