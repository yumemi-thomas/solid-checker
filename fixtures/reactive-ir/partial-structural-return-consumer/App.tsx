import { createTicker, createPanel } from "reactive-package";

// The listed accessor of a tuple whose item list is open is still proven.
export function BadTupleRead() {
  const [running] = createTicker();
  const now = running();
  return <div>{String(now)}</div>;
}
export function GoodTupleRead() {
  const [running] = createTicker();
  return <div>{String(running())}</div>;
}
// The unknown member names no reactive leaf; calling it is an obligation (ADR 0234).
export function UnknownTupleMember() {
  const [, stop] = createTicker();
  stop();
  return <div />;
}
// A position the contract does not list names no leaf either.
export function UnlistedPosition() {
  const ticker = createTicker() as unknown as Array<() => boolean>;
  const value = ticker[2];
  return <div>{String(value?.())}</div>;
}
export function BadObjectRead() {
  const panel = createPanel();
  const now = panel.value();
  return <div>{String(now)}</div>;
}
export function GoodObjectRead() {
  const panel = createPanel();
  return <div>{String(panel.value())}</div>;
}
export function UnknownObjectMember() {
  const panel = createPanel();
  panel.stop();
  return <div />;
}
// Handed to a JSX event handler, the member runs later, outside the render.
export function UnknownMemberAsHandler() {
  const [, stop] = createTicker();
  return <button onClick={stop} />;
}
function keep(value: unknown) {
  return value;
}
// A returned value that escapes takes its unknown member with it.
export function EscapedTicker() {
  const ticker = createTicker();
  keep(ticker);
  return <div />;
}
