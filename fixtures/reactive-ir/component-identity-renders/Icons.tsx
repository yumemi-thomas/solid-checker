import { createSignal } from "solid-js";
import { Dynamic } from "@solidjs/web";

const [count] = createSignal(0);

// Positive: a tag renders `Archive`, and its other reference stores it in a
// closed literal whose only use is a member read `Dynamic` renders
// (`createComponent(dynamic(() => props.component), …)`): no use can call it
// plainly, so its body is a component's and the read is a violation.
function Archive() {
  const value = count();
  return <b>{value}</b>;
}

// Positive: the same through a static member read and a shorthand value.
function Trash() {
  const value = count();
  return <i>{value}</i>;
}

const icons = { archive: Archive, Trash } as const;

export function IconMap(props: { name: "archive" | "Trash" }) {
  return (
    <div>
      <Archive />
      <Trash />
      <Dynamic component={icons[props.name]} />
      <Dynamic component={icons.Trash} />
    </div>
  );
}
