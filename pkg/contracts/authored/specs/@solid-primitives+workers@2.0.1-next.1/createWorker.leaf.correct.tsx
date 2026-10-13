/** @jsxImportSource @solidjs/web */
import { createWorker } from "@solid-primitives/workers";
export default function App() {
  const [, , stop] = createWorker({ add: (a: number, b: number) => a + b });
  stop();
  return <p>ready</p>;
}
