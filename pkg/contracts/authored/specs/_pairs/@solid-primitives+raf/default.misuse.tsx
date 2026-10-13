import createRAF from "@solid-primitives/raf";
export default function App() {
  const [running] = createRAF(() => {});
  const initial = running();
  return <p>{String(initial)}</p>;
}

