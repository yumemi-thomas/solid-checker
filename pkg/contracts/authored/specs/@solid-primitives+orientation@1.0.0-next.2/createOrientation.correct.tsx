/** @jsxImportSource @solidjs/web */
import { createOrientation } from "@solid-primitives/orientation";
export default function App() {
  const orientation = createOrientation();
  return <p>{String(orientation.angle())}</p>;
}
