/** @jsxImportSource @solidjs/web */
import { createAudio } from "@solid-primitives/audio";
export default function App() {
  const { currentTime } = createAudio("");
  return <p>{String(currentTime())}</p>;
}
