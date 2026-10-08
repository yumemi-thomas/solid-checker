/** @jsxImportSource @solidjs/web */
import { createTrigger } from "@solid-primitives/trigger";
export default function App() {
  const [track] = createTrigger();
  const current = track();
  return <p>{String(current)}</p>;
}
