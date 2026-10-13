import { createElementCursor } from "@solid-primitives/cursor";
export default function App() {
  createElementCursor(document.body, "pointer");
  return <p>ready</p>;
}
