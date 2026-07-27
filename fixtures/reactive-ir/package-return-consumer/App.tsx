import { createCount } from "reactive-package";

const count = createCount();

export function Good() {
  return <div>{count()}</div>;
}

export function Bad() {
  const value = count();
  return <div>{value}</div>;
}
