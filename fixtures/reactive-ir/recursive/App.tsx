import { readA } from "./source";

export function Good() {
  return <div>{readA(2)}</div>;
}

export function Bad() {
  const value = readA(2);
  return <div>{value}</div>;
}
