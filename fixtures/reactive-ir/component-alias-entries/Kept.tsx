import { createEffect, createSignal } from "solid-js";

const [count, setCount] = createSignal(0);

// Violation kept, plus an uncertifiable body read: the untracked call in
// `BothHost` proves the read; `map` may also render `Both`.
function Both() {
  const value = count();
  return <b>{value}</b>;
}

export function BothHost() {
  const el = Both();
  return <p>{el}{[1].map(Both)}</p>;
}

// Violation kept: the handler's call proves the effect unowned.
function Effectful() {
  createEffect(() => count(), () => {});
  return <b />;
}

// Uncertifiable (SC2001): the handler's call writes legally; the entry
// through `map` is unfollowed.
function Writer() {
  setCount(1);
  return <b />;
}

export function Clicker() {
  return (
    <p>
      <button onClick={() => { Effectful(); Writer(); }} />
      {[1].map(Effectful)}
      {[1].map(Writer)}
    </p>
  );
}
