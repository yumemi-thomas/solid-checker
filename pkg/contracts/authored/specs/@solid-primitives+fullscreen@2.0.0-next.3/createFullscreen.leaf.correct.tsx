import { onSettled } from "solid-js";
import { createFullscreen } from "@solid-primitives/fullscreen";
export default function App() {
  createFullscreen(document.body);
  onSettled(() => {});
  return <p>ready</p>;
}
