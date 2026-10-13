import { createPointerPosition } from "@solid-primitives/pointer";
export default function App() {
  const position = createPointerPosition();
  return <p>{String(position().x)}</p>;
}
