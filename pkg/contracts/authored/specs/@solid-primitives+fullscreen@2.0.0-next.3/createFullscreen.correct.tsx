import { createFullscreen } from "@solid-primitives/fullscreen";
export default function App() {
  const screen = createFullscreen(document.body);
  return <p>{String(screen.isActive())}</p>;
}
