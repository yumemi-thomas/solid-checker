import { createPageVisibility } from "@solid-primitives/page-utilities";
export default function App() {
  const visible = createPageVisibility();
  const current = visible();
  return <p>{String(current)}</p>;
}
