/** @jsxImportSource @solidjs/web */
import { createSpring } from "@solid-primitives/spring";
export default function App() {
  const [, , extras] = createSpring(0);
  return <p>{String(extras.isAnimating())}</p>;
}
