import { createHydratableSignal } from "@solid-primitives/utils";

export default function App() {
  const [value] = createHydratableSignal(0, () => 1);
  const current = value();
  return <p>{String(current)}</p>;
}
