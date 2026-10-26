// inferay RowContextMenu.tsx:26-30: the same shape with prop reads (`_props.onCompareWithWip`),
// which the analysis reports as reads in code it cannot place.
import { createMemo, For } from "solid-js";

type Entry = { label: string; run: () => void };

export function RowMenu(props: { id: string; onCompare?: (id: string) => void }) {
  const entries = createMemo(() => {
    const items: Entry[] = [];
    if (props.onCompare) items.push({ label: "Compare", run: () => props.onCompare?.(props.id) });
    return items;
  });
  return (
    <ul>
      <For each={entries()}>{(entry) => <li onClick={() => entry.run()}>{entry.label}</li>}</For>
    </ul>
  );
}
