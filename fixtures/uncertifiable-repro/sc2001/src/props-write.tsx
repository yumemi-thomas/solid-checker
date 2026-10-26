// app-game solid-virtual examples/shared/Item.tsx:68: an exported component writes through the rest props object of a split.
import { omit } from "solid-js";

export function Item(props: { label: string; ref?: (el: HTMLDivElement) => void }) {
  const others = omit(props, "label");
  others.ref = undefined;
  return <div>{props.label}</div>;
}
