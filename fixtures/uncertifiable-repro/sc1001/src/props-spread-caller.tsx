// app-game solid-svg-editor svg-renderer.tsx:29: a private component is rendered with a spread of props, so which props reach it is not enumerable.
import { createSignal } from "solid-js";
import type { JSX } from "@solidjs/web";

function Item(props: { label: string; selected?: boolean }) {
  const [open, setOpen] = createSignal(props.selected ?? false);
  return <li onClick={() => setOpen(!open())}>{props.label}</li>;
}

export function List(props: { items: { label: string; selected?: boolean }[] }): JSX.Element {
  return <ul>{props.items.map((item) => <Item {...item} />)}</ul>;
}
