import { intersection } from "@solid-primitives/set";
export default function App() {
  const result = intersection(new Set([1]), new Set([2]));
  const current = result().size; // Expected STRICT_READ_UNTRACKED.
  return <p>{String(current)}</p>;
}
