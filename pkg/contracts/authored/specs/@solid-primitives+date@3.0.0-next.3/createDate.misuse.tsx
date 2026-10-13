import { createDate } from "@solid-primitives/date";
export default function App() {
  const [date] = createDate(0);
  const current = date().getTime(); // Expected STRICT_READ_UNTRACKED.
  return <p>{String(current)}</p>;
}
