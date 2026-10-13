import { targetFPS } from "@solid-primitives/raf";
export default function App() {
  targetFPS(() => {}, () => 60);
  return <p>candidate</p>;
}
