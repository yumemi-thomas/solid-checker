import { createHydratableSignal } from "@solid-primitives/utils";

export default function App() {
  const [value] = createHydratableSignal(0, () => 1);
  return <p>{String(value())}</p>;
}
