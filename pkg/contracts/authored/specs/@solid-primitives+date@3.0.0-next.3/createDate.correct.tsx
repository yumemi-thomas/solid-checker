import { createDate } from "@solid-primitives/date";
export default function App() {
  const [date] = createDate(0);
  return <p>{String(date().getTime())}</p>;
}
