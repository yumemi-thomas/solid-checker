/** @jsxImportSource @solidjs/web */
import { createAudio } from "@solid-primitives/audio";
export default function App() {
  const { playing } = createAudio("");
  const current = playing();
  return <p>{String(current)}</p>;
}
