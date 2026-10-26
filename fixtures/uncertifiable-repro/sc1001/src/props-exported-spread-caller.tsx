// kui feature-schemas/src/CompatibilityCheck.tsx:76: an exported component reads a prop of its own, and one caller
// renders it with a spread, so which props reach it is not enumerable (the exported check alone would not certify it).
import { createSignal } from "solid-js";
import type { JSX } from "@solidjs/web";

export function Item(props: { label: string; selected?: boolean }) {
  const [open, setOpen] = createSignal(props.selected ?? false);
  return <li onClick={() => setOpen(!open())}>{props.label}</li>;
}

export function List(props: { items: { label: string; selected?: boolean }[] }): JSX.Element {
  return <ul>{props.items.map((item) => <Item {...item} />)}</ul>;
}
