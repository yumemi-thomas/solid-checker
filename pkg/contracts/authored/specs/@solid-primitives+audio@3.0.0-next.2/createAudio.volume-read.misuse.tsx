/** @jsxImportSource @solidjs/web */
import { createAudio } from "@solid-primitives/audio";
export default function App() {
  const { volume } = createAudio("");
  const current = volume();
  return <p>{String(current)}</p>;
}
