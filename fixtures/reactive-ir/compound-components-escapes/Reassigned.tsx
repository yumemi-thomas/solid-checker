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

function Other() {
  return <u />;
}

// Uncertifiable: a `let` holder can be rebound, and a member of a `const`
// holder can be overwritten or read back, so neither fixes what a tag renders.
let Mutable = { Panel };
Mutable = { Panel: Other };

const Written = { Handle };
Written.Handle = Other;

export function Screen() {
  return (
    <div>
      <Mutable.Panel />
      <Written.Handle />
    </div>
  );
}
