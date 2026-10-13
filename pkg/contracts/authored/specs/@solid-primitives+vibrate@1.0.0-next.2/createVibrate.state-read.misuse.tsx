/** @jsxImportSource @solidjs/web */
import { createVibrate } from "@solid-primitives/vibrate";
export default function App() {
  const { vibrating } = createVibrate(200);
  const current = vibrating();
  return <p>{String(current)}</p>;
}
