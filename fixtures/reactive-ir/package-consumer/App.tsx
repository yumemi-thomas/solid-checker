import { readCount } from "reactive-package";

export function Good() {
  return <div>{readCount()}</div>;
}

export function Bad() {
  const value = readCount();
  return <div>{value}</div>;
}
