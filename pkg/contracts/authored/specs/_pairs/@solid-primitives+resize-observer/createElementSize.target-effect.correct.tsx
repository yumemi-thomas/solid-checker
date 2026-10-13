import { createElementSize } from "@solid-primitives/resize-observer";
export default function App() {
  createElementSize(() => document.body);
  return <p>candidate</p>;
}
