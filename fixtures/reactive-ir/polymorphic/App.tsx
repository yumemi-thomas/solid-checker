import { overloaded, readGeneric } from "./source";

export function Good() {
  return <div>{readGeneric("safe")}{overloaded(1)}</div>;
}

export function Bad() {
  const first = readGeneric("lost");
  const second = overloaded("lost");
  return <div>{first + second}</div>;
}
