/** @jsxImportSource @solidjs/web */
import { createAudio } from "@solid-primitives/audio";
export default function App() {
  const { playing } = createAudio("");
  return <p>{String(playing())}</p>;
}
