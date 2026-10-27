// ai-memory-ui screens/access.tsx:111 (`const close = () => setDialog(null)` passed as onClose; app-game dropdown-menu.tsx:194 has the same
// wrapper): a named callback passed to a component that forwards an `omit` rest spread into the component that invokes it.
import { createSignal, omit } from "solid-js";
import type { JSX } from "@solidjs/web";

type ItemProps = {
  readonly checked?: boolean;
  readonly onClick?: (event: MouseEvent) => void;
  readonly children?: JSX.Element;
};

function Item(props: ItemProps) {
  return <li onClick={(event) => props.onClick?.(event)}>{props.children}</li>;
}

function CheckboxItem(props: ItemProps) {
  const rest = omit(props, "checked");
  return <Item {...rest}>{props.checked ? "✓ " : ""}{props.children}</Item>;
}

export function Menu() {
  const [checked, setChecked] = createSignal(false);
  function toggle() {
    setChecked(!checked());
  }
  return <CheckboxItem checked={checked()} onClick={toggle}>Wrap</CheckboxItem>;
}
