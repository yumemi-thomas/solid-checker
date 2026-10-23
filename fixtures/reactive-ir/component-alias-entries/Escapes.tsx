import { createSignal } from "solid-js";

const [count] = createSignal(0);

// Uncertifiable: called in tracked JSX and handed to `map`, which the census
// does not follow; that entry may render it.
function Passed() {
  const value = count();
  return <b>{value}</b>;
}

// Uncertifiable: called in tracked JSX and stored in an object.
function Stored() {
  const value = count();
  return <b>{value}</b>;
}

const parts = { stored: Stored };

// Uncertifiable: called in tracked JSX; its alias escapes into `map`.
function AliasPassed() {
  const value = count();
  return <b>{value}</b>;
}

const A2 = AliasPassed;

// Uncertifiable: a written `let` is no exact alias, so neither its tag nor
// its other uses are followed.
function Reassigned() {
  const value = count();
  return <b>{value}</b>;
}

function Other() {
  return <i />;
}

let R2 = Reassigned;
R2 = Other;

export function Escapes() {
  return (
    <p>
      {Passed()}
      {[1].map(Passed)}
      {Stored()}
      {parts.stored()}
      {AliasPassed()}
      {[1].map(A2)}
      {Reassigned()}
      <R2 />
    </p>
  );
}
