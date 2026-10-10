import createRAF from "@solid-primitives/raf";
export default function App() {
  const [running] = createRAF(() => {});
  return <p>{String(running())}</p>;
}

