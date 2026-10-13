/** @jsxImportSource @solidjs/web */
import { createVibrate } from "@solid-primitives/vibrate";
export default function App() {
  const { vibrating } = createVibrate(200);
  return <p>{String(vibrating())}</p>;
}
