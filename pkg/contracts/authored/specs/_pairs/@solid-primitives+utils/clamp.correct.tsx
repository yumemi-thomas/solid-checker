import { clamp } from "@solid-primitives/utils";
export default function App() {
  const actual = clamp(12, 0, 10);
  if (actual !== 10) throw new Error("source observation contradicted");
  return <p>ordinary return observation</p>;
}
