import { onSettled } from "solid-js";
import { createConnectivitySignal } from "@solid-primitives/connectivity";
export default function App() {
  onSettled(() => {
    try { createConnectivitySignal(); } catch { /* Retain the emitted leaf diagnostic. */ }
  });
  return <p>ready</p>;
}
