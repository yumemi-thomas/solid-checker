import createRAF from "@solid-primitives/raf";
export default function App() {
  createRAF(() => {});
  return <p>candidate</p>;
}
