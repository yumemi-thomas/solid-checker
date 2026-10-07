import { createRoot } from "solid-js";
import { createTimer } from "@solid-primitives/timer";
createRoot(dispose => {
  createTimer(() => {}, () => 30_000, setInterval);
  setTimeout(dispose, 100);
});
export default function App() { return <p>timer</p>; }

