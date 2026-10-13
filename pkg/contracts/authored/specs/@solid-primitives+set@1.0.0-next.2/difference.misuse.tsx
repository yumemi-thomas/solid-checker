import { difference } from "@solid-primitives/set";
export default function App() {
  const result = difference(new Set([1]), new Set([2]));
  const current = result().size; // Expected STRICT_READ_UNTRACKED.
  return <p>{String(current)}</p>;
}
