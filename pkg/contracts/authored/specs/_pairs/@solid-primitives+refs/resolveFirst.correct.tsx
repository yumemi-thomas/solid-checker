import { resolveFirst } from "@solid-primitives/refs";
export default function App() {
  const first = resolveFirst(() => document.body);
  return <p>{first()?.tagName}</p>;
}

