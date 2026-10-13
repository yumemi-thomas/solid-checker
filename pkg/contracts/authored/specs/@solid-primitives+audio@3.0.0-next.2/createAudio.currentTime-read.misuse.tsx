/** @jsxImportSource @solidjs/web */
import { createAudio } from "@solid-primitives/audio";
export default function App() {
  const { currentTime } = createAudio("");
  const current = currentTime();
  return <p>{String(current)}</p>;
}
