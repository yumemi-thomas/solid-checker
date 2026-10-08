import { createReducedMotion } from "@solid-primitives/a11y";
export default function App() {
  const reduced = createReducedMotion();
  const current = reduced();
  return <p>{String(current)}</p>;
}
