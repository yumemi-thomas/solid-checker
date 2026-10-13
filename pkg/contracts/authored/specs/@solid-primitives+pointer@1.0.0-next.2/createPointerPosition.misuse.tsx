import { createPointerPosition } from "@solid-primitives/pointer";
export default function App() {
  const position = createPointerPosition();
  const current = position().x; // Expected STRICT_READ_UNTRACKED.
  return <p>{String(current)}</p>;
}
