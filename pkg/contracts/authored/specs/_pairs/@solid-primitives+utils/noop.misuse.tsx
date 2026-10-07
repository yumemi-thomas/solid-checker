import { noop } from "@solid-primitives/utils";
export default function App() {
  const actual = noop();
  if (actual !== undefined) throw new Error("source observation contradicted");
  return <p>adversarial return observation</p>;
}
