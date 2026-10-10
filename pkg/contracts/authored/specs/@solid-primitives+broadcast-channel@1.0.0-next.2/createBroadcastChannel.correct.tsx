/** @jsxImportSource @solidjs/web */
import { createBroadcastChannel } from "@solid-primitives/broadcast-channel";
export default function App() {
  const { message } = createBroadcastChannel<string>("research-batch-4a");
  return <p>{String(message())}</p>;
}
