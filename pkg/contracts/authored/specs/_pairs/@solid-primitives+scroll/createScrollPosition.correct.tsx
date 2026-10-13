import { createScrollPosition } from "@solid-primitives/scroll";
export default function App() {
  const position = createScrollPosition(document.body);
  return <p>{position.y}</p>;
}

