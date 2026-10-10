import { createBodyCursor } from "@solid-primitives/cursor";
export default function App() {
  createBodyCursor(() => "pointer");
  return <p>ready</p>;
}
