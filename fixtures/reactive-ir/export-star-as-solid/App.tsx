import { Local, Solid } from "./api";

export function Reads() {
  const [read] = Solid.createSignal(1);
  read();
  return <p />;
}

export function Tracked() {
  const [read] = Solid.createSignal(1);
  return <p>{read()}</p>;
}

export function Project() {
  const [read] = Local.createSignal(1);
  read();
  return <p />;
}
