// dashiboard-ui components/FilePicker.tsx:43-48,74 (used where the Load tab renders it): a function is the child Button's `onClick`
// (which only calls it from a click) and is also handed to a callback prop, `untrack(() => props.refreshRef)?.(ask)`.
import { createSignal, untrack } from "solid-js";

function Button(props: { onClick?: (event: MouseEvent) => void; children: string }) {
  return <button onClick={(event) => props.onClick?.(event)}>{props.children}</button>;
}

type PickerProps = { refreshRef?: (refresh: () => void) => void };

export function Picker(props: PickerProps) {
  const [asking, setAsking] = createSignal(false);
  function ask() {
    setAsking(true);
  }
  untrack(() => props.refreshRef)?.(ask);
  return <Button onClick={ask}>{asking() ? "…" : "↻"}</Button>;
}
