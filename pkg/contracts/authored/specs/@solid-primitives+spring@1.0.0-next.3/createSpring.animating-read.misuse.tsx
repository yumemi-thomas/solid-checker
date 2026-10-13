/** @jsxImportSource @solidjs/web */
import { createSpring } from "@solid-primitives/spring";
export default function App() {
  const [, , extras] = createSpring(0);
  const frozen = extras.isAnimating();
  return <p>{String(frozen)}</p>;
}
