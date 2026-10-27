// Clean. Mirrors probus-hk src/routes/Notices.tsx:153: the item and the index are read in the JSX a
// For callback returns; the same reads in the callback body are SC1001 (read-in-for-map).
import { createSignal, For } from "solid-js";

export function Notices() {
  const [list] = createSignal([{ id: "a", text: "late" }]);
  return (
    <For each={list()}>
      {(notice, index) => (
        <p data-first={index() === 0}>{notice.text}</p>
      )}
    </For>
  );
}
