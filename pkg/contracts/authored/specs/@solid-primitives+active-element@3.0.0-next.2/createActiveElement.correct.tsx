import { createActiveElement } from "@solid-primitives/active-element";
export default function App() {
  const active = createActiveElement();
  return <p>{String(active()?.tagName)}</p>;
}
