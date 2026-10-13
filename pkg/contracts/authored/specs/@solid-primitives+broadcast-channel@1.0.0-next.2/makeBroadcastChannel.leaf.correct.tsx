/** @jsxImportSource @solidjs/web */
import { onSettled } from "solid-js";
import { makeBroadcastChannel } from "@solid-primitives/broadcast-channel";
export default function App() {
  makeBroadcastChannel<string>("research-batch-4a");
  onSettled(() => {});
  return <p>ready</p>;
}
