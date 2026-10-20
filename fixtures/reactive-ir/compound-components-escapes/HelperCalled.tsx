import { createSignal } from "solid-js";

const [count] = createSignal(0);

// The strict read sits in a helper the members call while they render.
const useSplit = () => {
  const value = count();
  return value;
};

function Panel() {
  const split = useSplit();
  return <b>{split}</b>;
}

function Handle() {
  const split = useSplit();
  return <i>{split}</i>;
}

// Uncertifiable: a member is also invoked as a plain call, so the helper read
// in its body runs in the caller's tracked scope, not under `createComponent`.
const Split = { Panel, Handle };

export function Screen() {
  return (
    <div>
      <Split.Panel />
      <Split.Handle />
      {Split.Panel()}
    </div>
  );
}
