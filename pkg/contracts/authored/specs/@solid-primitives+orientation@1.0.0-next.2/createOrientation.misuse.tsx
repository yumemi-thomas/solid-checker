/** @jsxImportSource @solidjs/web */
import { createOrientation } from "@solid-primitives/orientation";
export default function App() {
  const orientation = createOrientation();
  const frozen = orientation.angle();
  return <p>{String(frozen)}</p>;
}
