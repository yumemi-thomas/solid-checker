import { createWindowSize } from "@solid-primitives/resize-observer";
export default function App() {
  createWindowSize();
  return <p>candidate</p>;
}
