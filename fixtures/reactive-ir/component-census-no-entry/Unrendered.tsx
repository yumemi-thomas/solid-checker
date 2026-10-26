import { createSignal } from "solid-js";

const [n, setN] = createSignal(0);

// Uncertifiable: nothing renders these, so no entry places them as component
// bodies. A violation here would claim a write that never runs, or runs where
// no owner exists.
function Dead(x: number) {
  setN(x);
  return <li />;
}

function dead(x: number) {
  setN(x);
  return <li />;
}

export function renderRow(x: number) {
  setN(x);
  return <li />;
}

export const makeRow = (x: number) => {
  setN(x);
  return <li />;
};

// Module evaluation has no owner: a callback or an IIFE run there is no
// component, whatever it returns.
export const rows = [1, 2, 3].map((x) => {
  setN(x);
  return <li>{x}</li>;
});

export const once = (() => {
  setN(5);
  return <b>{n()}</b>;
})();

// Shadowed: the tag below names the local `Row`, never this one.
function Row() {
  setN(6);
  return <li />;
}

export function Shadows() {
  const Row = () => <i />;
  return <Row />;
}
