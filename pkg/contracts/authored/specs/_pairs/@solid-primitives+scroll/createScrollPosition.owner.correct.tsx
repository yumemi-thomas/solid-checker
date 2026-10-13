import { createScrollPosition } from "@solid-primitives/scroll";
export default function App() {
  createScrollPosition(window);
  return <p>candidate</p>;
}
