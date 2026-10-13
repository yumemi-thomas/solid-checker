/** @jsxImportSource @solidjs/web */
import { createSignal } from "solid-js";
import { createTween } from "@solid-primitives/tween";
export default function App() {
  const [, setSink] = createSignal(0);
  let calls = 0;
  createTween(() => { if (++calls > 1) { try { setSink(1); } catch {} } return 1; }, { duration: 100 });
  return <p>ready</p>;
}
