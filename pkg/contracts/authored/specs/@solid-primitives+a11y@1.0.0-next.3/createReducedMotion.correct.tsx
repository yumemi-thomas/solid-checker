import { createReducedMotion } from "@solid-primitives/a11y";
export default function App() {
  const reduced = createReducedMotion();
  return <p>{String(reduced())}</p>;
}
