/** @jsxImportSource @solidjs/web */
import { createBattery } from "@solid-primitives/sensors";
export default function App() {
  const battery = createBattery();
  return <p>{String(battery()?.level)}</p>;
}
