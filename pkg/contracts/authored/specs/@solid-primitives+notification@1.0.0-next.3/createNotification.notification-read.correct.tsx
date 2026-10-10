/** @jsxImportSource @solidjs/web */
import { createNotification } from "@solid-primitives/notification";
export default function App() {
  const note = createNotification("hello");
  return <p>{String(note.notification() === null)}</p>;
}
