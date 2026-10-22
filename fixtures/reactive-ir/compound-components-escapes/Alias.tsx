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

function Row() {
  const value = count();
  return <li>{value}</li>;
}

export const Parts = { Panel, Handle, Row };

// Uncertifiable: a member read as a value (`const P = Parts.Panel`, a
// destructuring, a plain call of the member) runs the function as a plain
// call inside tracked JSX, where `createComponent` is not involved.
const P = Parts.Panel;
const { Handle: H } = Parts;

export function Screen() {
  return (
    <div>
      <Parts.Panel />
      <Parts.Handle />
      <Parts.Row />
      {P()}
      {H()}
      {Parts.Row()}
    </div>
  );
}
