import { createSignal } from "solid-js";

const [count] = createSignal(0);

function Plain() {
  const value = count();
  return <b>{value}</b>;
}

const P2 = Plain;

export function AliasRendered() {
  return <p>{Plain()}<P2 /></p>;
}

function Twice() {
  const value = count();
  return <i>{value}</i>;
}

const T2 = Twice;

export function AliasCalled() {
  return <p>{Twice()}{T2()}</p>;
}
