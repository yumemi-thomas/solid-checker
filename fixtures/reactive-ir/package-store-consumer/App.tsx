import { createState } from "reactive-package/state";

const state = createState();

export function Good() {
  return <div>{state.value}</div>;
}

export function Bad() {
  const value = state.value;
  return <div>{value}</div>;
}
