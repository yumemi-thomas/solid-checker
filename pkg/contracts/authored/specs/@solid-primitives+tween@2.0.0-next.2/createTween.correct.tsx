/** @jsxImportSource @solidjs/web */
import { createTween } from "@solid-primitives/tween";
export default function App() {
  const tweened = createTween(() => 1, { duration: 100 });
  return <p>{String(tweened())}</p>;
}
