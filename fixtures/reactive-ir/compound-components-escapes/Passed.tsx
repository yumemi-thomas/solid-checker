import { createSignal } from "solid-js";

const [count] = createSignal(0);

function Panel() {
  const value = count();
  return <b>{value}</b>;
}

function Handle() {
  const value = count();
  return <i>{value}</i>;
}

export const Parts = { Panel, Handle };

// Uncertifiable: `Parts` is also passed to a function that may call a member
// plainly, so a tag is not the only way to obtain `Panel`.
function consume(parts: Record<string, () => unknown>) {
  return Object.keys(parts).length;
}

consume(Parts);

export function Screen() {
  return (
    <div>
      <Parts.Panel />
      <Parts.Handle />
    </div>
  );
}
