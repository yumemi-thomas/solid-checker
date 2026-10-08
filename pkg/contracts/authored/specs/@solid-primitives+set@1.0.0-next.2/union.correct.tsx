import { union } from "@solid-primitives/set";
export default function App() {
  const result = union(new Set([1]), new Set([2]));
  return <p>{String(result().size)}</p>;
}
