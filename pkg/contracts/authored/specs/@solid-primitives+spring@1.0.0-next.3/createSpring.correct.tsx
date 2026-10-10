/** @jsxImportSource @solidjs/web */
import { createSpring } from "@solid-primitives/spring";
export default function App() {
  const [value] = createSpring(0);
  return <p>{String(value())}</p>;
}
