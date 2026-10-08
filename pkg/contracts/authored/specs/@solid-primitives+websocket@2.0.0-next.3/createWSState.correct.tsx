import { createWSState } from "@solid-primitives/websocket";
export default function App() {
  const ws = new WebSocket("ws://127.0.0.1:9/batch3a"); const state = createWSState(ws);
  return <p>{String(state())}</p>;
}
