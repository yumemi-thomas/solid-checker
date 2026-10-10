import { createClipboard } from "@solid-primitives/clipboard";
export default function App() {
  const [items] = createClipboard();
  return <p>{String(items().length)}</p>;
}
