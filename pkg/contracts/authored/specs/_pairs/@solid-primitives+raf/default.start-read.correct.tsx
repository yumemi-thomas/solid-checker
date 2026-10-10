import createRAF from "@solid-primitives/raf";
export default function App() {
  const [running, start, stop] = createRAF(() => {});
  return <p onClick={start} onDblClick={stop}>{String(running())}</p>;
}
