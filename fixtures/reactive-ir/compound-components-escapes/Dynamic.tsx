import { createSignal } from "solid-js";

const [count] = createSignal(0);
const base = {};

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

// Uncertifiable: a computed key, a spread and a getter each leave the member
// set or a member's value open, so the literal is not a closed census.
const Computed = { ["Panel"]: Panel };
const Spread = { ...base, Handle };
const Getter = {
  get Row() {
    return Row;
  },
};

export function Screen() {
  return (
    <div>
      <Computed.Panel />
      <Spread.Handle />
      <Getter.Row />
    </div>
  );
}
