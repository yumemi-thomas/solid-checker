/** @jsxImportSource @solidjs/web */
import { createAudio } from "@solid-primitives/audio";
export default function App() {
  const { volume } = createAudio("");
  return <p>{String(volume())}</p>;
}
