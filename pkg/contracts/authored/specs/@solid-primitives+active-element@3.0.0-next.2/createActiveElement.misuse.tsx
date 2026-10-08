import { createActiveElement } from "@solid-primitives/active-element";
export default function App() {
  const active = createActiveElement();
  const current = active()?.tagName;
  return <p>{String(current)}</p>;
}
