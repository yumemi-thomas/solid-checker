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

const Parts = { Panel, Handle };

// Uncertifiable: the member is invoked as a plain call
// (`Parts.Panel()`), so its body runs in the caller's tracked scope.
export function Screen() {
  return (
    <div>
      <Parts.Panel />
      <Parts.Handle />
      {Parts.Panel()}
    </div>
  );
}
