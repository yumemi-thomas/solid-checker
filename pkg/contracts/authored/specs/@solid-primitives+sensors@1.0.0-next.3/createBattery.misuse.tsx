/** @jsxImportSource @solidjs/web */
import { createBattery } from "@solid-primitives/sensors";
export default function App() {
  const battery = createBattery();
  const frozen = battery()?.level;
  return <p>{String(frozen)}</p>;
}
