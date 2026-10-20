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

// Positive: both members run only as components, so a call written directly
// in their bodies runs during rendering and the helper's read is reported at
// each call. A member held in an object is not a retained callable here.
export const Split = { Panel, Handle };

export function Screen() {
  return (
    <div>
      <Split.Panel />
      <Split.Handle />
    </div>
  );
}
