import { invoke, readCount } from "./source";

export function Good() {
  return <div>{invoke(readCount)}</div>;
}

export function Bad() {
  const value = invoke(readCount);
  return <div>{value}</div>;
}
