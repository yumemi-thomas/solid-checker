/** @jsxImportSource @solidjs/web */
import { createSignal, untrack } from "solid-js";
import { createAudio } from "@solid-primitives/audio";
export default function App() {
  const [url] = createSignal("");
  createAudio(() => untrack(() => url()));
  return <p>ready</p>;
}
