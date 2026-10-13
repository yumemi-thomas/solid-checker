/** @jsxImportSource @solidjs/web */
import { createDevices } from "@solid-primitives/devices";
export default function App() {
  const devices = createDevices();
  return <p>{String(devices().length)}</p>;
}
