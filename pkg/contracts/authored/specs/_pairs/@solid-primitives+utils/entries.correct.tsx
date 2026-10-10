import { entries } from "@solid-primitives/utils";
// P2 OBSERVATION ONLY: both twins are clean; exotic getter/Proxy trap probes are outside this premise.
export default function App() {
  const object = { x: 1, y: 2 };
  const result = entries(object);
  return <p id="done">{JSON.stringify(result)}</p>;
}
