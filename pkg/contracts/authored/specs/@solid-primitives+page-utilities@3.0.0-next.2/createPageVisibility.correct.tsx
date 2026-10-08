import { createPageVisibility } from "@solid-primitives/page-utilities";
export default function App() {
  const visible = createPageVisibility();
  return <p>{String(visible())}</p>;
}
