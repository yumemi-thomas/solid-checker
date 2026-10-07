import { createScrollPosition } from "@solid-primitives/scroll";
export default function App() {
  const position = createScrollPosition(document.body);
  const initial = position.y;
  return <p>{initial}</p>;
}

