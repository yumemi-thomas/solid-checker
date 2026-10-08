/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { createBroadcastChannel } from "@solid-primitives/broadcast-channel";
export default function App() {
  onSettled(() => { try { createBroadcastChannel<string>("research-batch-4a"); } catch { /* Keep the emitted dev diagnostic. */ } });
  return <p>ready</p>;
}
