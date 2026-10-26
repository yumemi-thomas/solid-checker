// inferay RefContextMenu.tsx:42-132: callbacks that read props and a memo are given to a local `add` that pushes `{ label, run }` into
// an array the memo returns; the component passes the array as a prop to a menu that calls `run` from a row's handler.
import { createMemo } from "solid-js";

type Entry = { label: string; run: () => void };

function Menu(props: { entries: Entry[] }) {
  return <ul>{props.entries.map((entry) => <li onClick={() => entry.run()}>{entry.label}</li>)}</ul>;
}

export function RefMenu(props: { name: string; onPick?: (name: string) => void }) {
  const ref = createMemo(() => props.name);
  const entries = createMemo(() => {
    const list: Entry[] = [];
    const add = (label: string, run: () => void) => list.push({ label, run });
    add("Pick", () => props.onPick?.(ref()));
    return list;
  });
  return <Menu entries={entries()} />;
}
