import { createClipboard } from "@solid-primitives/clipboard";
export default function App() {
  const [items] = createClipboard();
  const current = items().length;
  return <p>{String(current)}</p>;
}
