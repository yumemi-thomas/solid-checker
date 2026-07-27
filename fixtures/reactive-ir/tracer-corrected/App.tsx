import { count } from "./source";

export function Good() {
  return <div>{count()}</div>;
}

export function Corrected() {
  return <div>{count()}</div>;
}

export function Events() {
  return <button onClick={() => count()}>Read</button>;
}
