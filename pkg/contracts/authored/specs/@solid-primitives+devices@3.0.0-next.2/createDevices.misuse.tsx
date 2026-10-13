/** @jsxImportSource @solidjs/web */
import { createDevices } from "@solid-primitives/devices";
export default function App() {
  const devices = createDevices();
  const frozen = devices().length;
  return <p>{String(frozen)}</p>;
}
