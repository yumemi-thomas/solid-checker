import { createDateNow } from "@solid-primitives/date";
export default function App() {
  const [now] = createDateNow(0);
  return <p>{String(now().getTime())}</p>;
}
