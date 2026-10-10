import { createConnectivitySignal } from "@solid-primitives/connectivity";
export default function App() {
  const online = createConnectivitySignal();
  const current = online();
  return <p>{String(current)}</p>;
}
