import { createConnectivitySignal } from "@solid-primitives/connectivity";
export default function App() {
  const online = createConnectivitySignal();
  return <p>{String(online())}</p>;
}
