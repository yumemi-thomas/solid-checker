import { difference } from "@solid-primitives/set";
export default function App() {
  const result = difference(new Set([1]), new Set([2]));
  return <p>{String(result().size)}</p>;
}
