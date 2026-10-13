import { createDateNow } from "@solid-primitives/date";
export default function App() {
  const [now] = createDateNow(0);
  const current = now().getTime(); // Expected STRICT_READ_UNTRACKED.
  return <p>{String(current)}</p>;
}
