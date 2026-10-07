import { resolveFirst } from "@solid-primitives/refs";
export default function App() {
  const first = resolveFirst(() => document.body);
  const initial = first();
  return <p>{initial?.tagName}</p>;
}

