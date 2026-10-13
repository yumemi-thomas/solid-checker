/** @jsxImportSource @solidjs/web */
import { createOrientation } from "@solid-primitives/orientation";
export default function App() {
  const orientation = createOrientation();
  const frozen = orientation.type();
  return <p>{String(frozen)}</p>;
}
