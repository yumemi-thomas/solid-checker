import { createSignal } from "solid-js";

const [count] = createSignal(0);

// Positive: called in tracked JSX and rendered through an alias. `<P2 />`
// renders `Plain` as a component, so its body read is untracked there
// (runtime-probed on rc.17: STRICT_READ_UNTRACKED). The visible call proves
// nothing about the other entry.
function Plain() {
  const value = count();
  return <b>{value}</b>;
}

const P2 = Plain;

// Positive: a `let` nothing writes is an exact alias too, and a chain of
// aliases is followed.
function Chained() {
  const value = count();
  return <i>{value}</i>;
}

let C1 = Chained;
const C2 = C1;

export function App() {
  return (
    <p>
      {Plain()}
      <P2 />
      {Chained()}
      <C2 />
    </p>
  );
}
