import { createRAF } from "@solid-primitives/raf";
export default function App() {
  const [running, start, stop] = createRAF(() => {});
  start();
  return <p onClick={stop}>{String(running())}</p>;
}
