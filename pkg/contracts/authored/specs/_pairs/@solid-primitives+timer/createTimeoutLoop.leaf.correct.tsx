/** @jsxImportSource @solidjs/web */
import { createTimeoutLoop } from "@solid-primitives/timer";
export default function App() {
  createTimeoutLoop(() => {}, 1000);
  return <p>ready</p>;
}
