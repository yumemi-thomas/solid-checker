/** @jsxImportSource @solidjs/web */
import { createTrigger } from "@solid-primitives/trigger";
export default function App() {
  const [track] = createTrigger();
  return <p>{String(track())}</p>;
}
