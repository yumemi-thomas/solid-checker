/** @jsxImportSource @solidjs/web */
import { createNotification } from "@solid-primitives/notification";
export default function App() {
  const note = createNotification("hello");
  const frozen = note.notification() === null;
  return <p>{String(frozen)}</p>;
}
