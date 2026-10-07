import { trueFn } from "@solid-primitives/utils";
export default function App() {
  const actual = trueFn();
  if (actual !== true) throw new Error("source observation contradicted");
  return <p>ordinary return observation</p>;
}
