import { count } from "./source";

export function Good() {
  return <div>{count()}</div>;
}

export function Bad() {
  const value = count();
  return <div>{value}</div>;
}

export function Events() {
  return <button onClick={() => count()}>Read</button>;
}
