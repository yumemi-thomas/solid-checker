import { createSignal } from "solid-js";

const [count] = createSignal(0);

function Root(props: { children?: unknown }) {
  return <div>{props.children}</div>;
}

// Positive: stored in an object literal and reached only as a member tag
// (`<Split.Panel>`), so `createComponent` is the only caller.
function Panel() {
  const value = count();
  return <b>{value}</b>;
}

function Handle() {
  return <i />;
}

export const Split = { Root, Panel, Handle };

export function Screen() {
  return (
    <Split.Root>
      <Split.Panel />
      <Split.Handle></Split.Handle>
    </Split.Root>
  );
}
