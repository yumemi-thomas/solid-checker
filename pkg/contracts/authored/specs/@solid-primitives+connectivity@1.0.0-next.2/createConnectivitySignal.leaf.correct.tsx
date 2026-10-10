import { onSettled } from "solid-js";
import { createConnectivitySignal } from "@solid-primitives/connectivity";
export default function App() {
  createConnectivitySignal();
  onSettled(() => {});
  return <p>ready</p>;
}
