/** @jsxImportSource @solidjs/web */
import { createMs } from "@solid-primitives/raf";
export default function App() {
  createMs(60);
  return <p>ready</p>;
}
