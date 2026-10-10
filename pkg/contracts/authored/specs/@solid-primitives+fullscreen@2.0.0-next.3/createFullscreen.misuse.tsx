import { createFullscreen } from "@solid-primitives/fullscreen";
export default function App() {
  const screen = createFullscreen(document.body);
  const current = screen.isActive();
  return <p>{String(current)}</p>;
}
